use nmr::processed::ProcessedDataset;
use nmr::processing::{
    PhaseCorrection, ProcessingError, ProcessingOperation, ProcessingOptions, ProcessingPlan,
};
use nmr::provenance::SourceDigest;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn fixture(path: &Path, real: &[i32], imaginary: &[i32], nc: i32, offset: i32, title: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(path.join("procs"), format!("##TITLE= synthetic\n##$SI= 3\n##$DTYPP= 0\n##$BYTORDP= 0\n##$NC_proc= {nc}\n$$ {title}\n##$SW_p= 1200\n##$SF= 400\n##$OFFSET= {offset}\n##$AXNUC= <1H>\n")).unwrap();
    for (name, values) in [("1r", real), ("1i", imaginary)] {
        fs::write(
            path.join(name),
            values
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    }
}

fn read(path: &Path) -> ProcessedDataset {
    nmr::read(path).unwrap().into_processed_data().unwrap()
}

fn phase(angle: f64) -> ProcessingOperation {
    ProcessingOperation::PhaseCorrection {
        axis: 0,
        correction: PhaseCorrection::new(angle, 0.0, 0.5).unwrap(),
    }
}

fn compare(a: &ProcessedDataset, b: &ProcessedDataset) {
    assert_eq!(a.descriptor(), b.descriptor());
    assert_eq!(a.data().shape(), b.data().shape());
    assert_eq!(a.data().component_counts(), b.data().component_counts());
    assert_eq!(a.data().samples().len(), b.data().samples().len());
    for (a, b) in a.data().samples().iter().zip(b.data().samples()) {
        assert!((a - b).abs() < 1e-12);
    }
}

#[test]
fn segmented_replay_charges_retained_intermediate_samples() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path(), &[1, -2, 3], &[4, 5, -6], 0, 10, "budget");
    let imported = read(temp.path());
    let plan = ProcessingPlan::new(vec![phase(12.0)]).unwrap();
    let first = plan.apply_processed(&imported).unwrap();
    let result = plan.apply_processed(&first).unwrap();
    let sample_bytes = 6 * std::mem::size_of::<f64>();
    // Three complex points: current + next + one gathered trace. The original
    // input is borrowed; a replay-created intermediate result is an extra copy.
    first
        .provenance()
        .history()
        .unwrap()
        .replay_processed(&imported, ProcessingOptions::new())
        .unwrap();
    let history = result.provenance().history().unwrap();
    assert!(matches!(
        history
            .replay_processed(
                &imported,
                ProcessingOptions::new().max_working_bytes(4 * sample_bytes - 1),
            )
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            nmr::resource::LimitExceeded {
                resource: nmr::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let replayed = history
        .replay_processed(&imported, ProcessingOptions::new())
        .unwrap();
    compare(&result, &replayed);
}

#[test]
fn decoded_bytes_and_two_stage_replay_bind_the_original_import() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("original");
    fixture(&path, &[1, -2, 3], &[4, 5, -6], 1, 10, "original");
    let imported = read(&path);
    assert_eq!(
        imported.data().samples(),
        &[2.0, 8.0, -4.0, 10.0, 6.0, -12.0]
    );
    for (ordinal, source) in imported.provenance().sources().iter().enumerate() {
        assert_eq!(source.id().unwrap().ordinal(), ordinal);
        assert_eq!(
            source.digest(),
            SourceDigest::Sha256(Sha256::digest(fs::read(source.path()).unwrap()).into())
        );
    }
    let record = imported.provenance().read_record().unwrap();
    assert_eq!(record.output_digests(), imported.canonical_digests());
    assert_eq!(
        record.transform(),
        &nmr::provenance::ProcessedReadTransform::Bruker {
            nc_proc: 1,
            float64: false,
            big_endian: false
        }
    );
    assert_eq!(
        record
            .components()
            .iter()
            .map(|id| id.ordinal())
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let first = ProcessingPlan::new(vec![phase(12.0)])
        .unwrap()
        .apply_processed(&imported)
        .unwrap();
    let result = ProcessingPlan::new(vec![phase(-2.0)])
        .unwrap()
        .apply_processed(&first)
        .unwrap();
    drop(first);
    let once = ProcessingPlan::new(vec![phase(12.0), phase(-2.0)])
        .unwrap()
        .apply_processed(&imported)
        .unwrap();
    compare(&result, &once);
    let angle = 10f64.to_radians();
    for (input, output) in imported
        .data()
        .samples()
        .chunks_exact(2)
        .zip(result.data().samples().chunks_exact(2))
    {
        assert!((output[0] - (input[0] * angle.cos() - input[1] * angle.sin())).abs() < 1e-12);
        assert!((output[1] - (input[0] * angle.sin() + input[1] * angle.cos())).abs() < 1e-12);
    }
    let history = result.provenance().history().unwrap().clone();
    assert_eq!(history.records().len(), 2);
    assert_eq!(history.segments().len(), 2);
    assert_eq!(history.segments()[0].end_record(), 1);
    assert_eq!(history.segments()[1].end_record(), 2);
    assert_eq!(
        history.segments()[1].output_digests(),
        result.canonical_digests()
    );
    let replayed = history
        .replay_processed(&imported, ProcessingOptions::new())
        .unwrap();
    compare(&result, &replayed);
    assert_eq!(
        replayed.provenance().history().unwrap().segments(),
        history.segments()
    );
    assert!(matches!(
        ProcessedDataset::new(
            imported.descriptor().clone(),
            imported.data().clone(),
            imported.provenance().clone()
        ),
        Err(nmr::processed::ProcessedValidationError::LibraryDerivedProvenance)
    ));
    let moved = temp.path().join("moved");
    fs::rename(&path, &moved).unwrap();
    let relocated = read(&moved);
    assert_eq!(imported.canonical_digests(), relocated.canonical_digests());
    compare(
        &result,
        &history
            .replay_processed(&relocated, ProcessingOptions::new())
            .unwrap(),
    );
    fixture(&moved, &[99, -2, 3], &[4, 5, -6], 1, 10, "original");
    // Changing the path after import cannot change the already consumed identity.
    compare(
        &result,
        &history
            .replay_processed(&imported, ProcessingOptions::new())
            .unwrap(),
    );
    assert!(matches!(
        history
            .replay_processed(&read(&moved), ProcessingOptions::new())
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::InputIdentityMismatch)
    ));
}

#[test]
fn strict_sources_and_canonical_content_have_distinct_equivalence_rules() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path();
    fixture(path, &[1, -2, 3], &[4, 5, -6], 1, 10, "original");
    let original = read(path);
    let result = ProcessingPlan::new(vec![phase(12.0)])
        .unwrap()
        .apply_processed(&original)
        .unwrap();
    let history = result.provenance().history().unwrap();
    for (real, imaginary, nc, offset, title, same_samples, same_semantics) in [
        ([1, -2, 3], [4, 5, -6], 1, 10, "comment changed", true, true),
        ([1, -2, 3], [4, 5, -6], 1, 11, "original", true, false),
        ([2, -4, 6], [8, 10, -12], 0, 10, "original", true, true),
        ([1, -2, 3], [4, 5, -6], 2, 10, "original", false, true),
        ([99, -2, 3], [4, 5, -6], 1, 10, "original", false, true),
        ([4, 5, -6], [1, -2, 3], 1, 10, "original", false, true),
    ] {
        fixture(path, &real, &imaginary, nc, offset, title);
        let changed = read(path);
        assert_eq!(
            original.canonical_digests().samples() == changed.canonical_digests().samples(),
            same_samples
        );
        assert_eq!(
            original.canonical_digests().descriptor() == changed.canonical_digests().descriptor(),
            same_semantics
        );
        assert!(matches!(
            history
                .replay_processed(&changed, ProcessingOptions::new())
                .map_err(ProcessingError::into_root_cause),
            Err(ProcessingError::InputIdentityMismatch)
        ));
    }
}

#[test]
fn binary_encoding_matrix_preserves_content_but_binds_decoding() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path();
    fixture(path, &[1, -2, 3], &[4, 5, -6], -1, 10, "encoding");
    let baseline = read(path);
    for float64 in [false, true] {
        for big_endian in [false, true] {
            let text = fs::read_to_string(path.join("procs"))
                .unwrap()
                .replace("##$DTYPP= 0", "##$DTYPP= 2")
                .replace("##$BYTORDP= 0", "##$BYTORDP= 1");
            let text = if float64 {
                text
            } else {
                text.replace("##$DTYPP= 2", "##$DTYPP= 0")
            };
            let text = if big_endian {
                text
            } else {
                text.replace("##$BYTORDP= 1", "##$BYTORDP= 0")
            };
            fs::write(path.join("procs"), text).unwrap();
            for (name, values) in [("1r", [1i32, -2, 3]), ("1i", [4, 5, -6])] {
                let bytes: Vec<u8> = values
                    .into_iter()
                    .flat_map(|value| {
                        if float64 {
                            if big_endian {
                                (value as f64).to_be_bytes().to_vec()
                            } else {
                                (value as f64).to_le_bytes().to_vec()
                            }
                        } else if big_endian {
                            value.to_be_bytes().to_vec()
                        } else {
                            value.to_le_bytes().to_vec()
                        }
                    })
                    .collect();
                fs::write(path.join(name), bytes).unwrap();
            }
            let imported = read(path);
            assert_eq!(baseline.canonical_digests(), imported.canonical_digests());
            let record = imported.provenance().read_record().unwrap();
            assert_eq!(
                record.transform(),
                &nmr::provenance::ProcessedReadTransform::Bruker {
                    nc_proc: -1,
                    float64,
                    big_endian
                }
            );
            let output = ProcessingPlan::new(vec![phase(12.0)])
                .unwrap()
                .apply_processed(&imported)
                .unwrap();
            compare(
                &output,
                &output
                    .provenance()
                    .history()
                    .unwrap()
                    .replay_processed(&imported, ProcessingOptions::new())
                    .unwrap(),
            );
        }
    }
}

#[test]
fn memory_inputs_are_distinct_from_reader_authority_and_processing_state() {
    use nmr::processed::{ProcessedOrigin, ProcessedProvenance};
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path(), &[1, -2, 3], &[4, 5, -6], 0, 10, "state");
    let imported = read(temp.path());
    let memory = ProcessedDataset::new(
        imported.descriptor().clone(),
        imported.data().clone(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    assert_eq!(memory.canonical_digests(), imported.canonical_digests());
    let output = ProcessingPlan::new(vec![phase(12.0)])
        .unwrap()
        .apply_processed(&memory)
        .unwrap();
    compare(
        &output,
        &output
            .provenance()
            .history()
            .unwrap()
            .replay_processed(&memory, ProcessingOptions::new())
            .unwrap(),
    );
    let relabeled = ProcessedDataset::new(
        nmr::processed::ProcessedDescriptor::new(vec![
            memory.descriptor().axes()[0]
                .clone()
                .with_label(Some("display only".into())),
        ])
        .unwrap(),
        memory.data().clone(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    assert_eq!(relabeled.canonical_digests(), memory.canonical_digests());
    compare(
        &output,
        &output
            .provenance()
            .history()
            .unwrap()
            .replay_processed(&relabeled, ProcessingOptions::new())
            .unwrap(),
    );
    let reset = ProcessedDataset::new(
        output.descriptor().clone(),
        output.data().clone(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        reset.canonical_digests().samples(),
        output.canonical_digests().samples()
    );
    assert_ne!(
        reset.canonical_digests().descriptor(),
        output.canonical_digests().descriptor()
    );
    let unverified = ProcessedDataset::new(
        memory.descriptor().clone(),
        memory.data().clone(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap();
    let result = ProcessingPlan::new(vec![phase(12.0)])
        .unwrap()
        .apply_processed(&unverified)
        .unwrap();
    assert!(matches!(
        result
            .provenance()
            .history()
            .unwrap()
            .replay_processed(&unverified, ProcessingOptions::new())
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::MissingCapability {
            capability: "complete processed reading record",
            ..
        })
    ));
}

#[test]
fn bruker_quartet_retains_complete_parameters_and_replays_both_component_axes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let direct = "##TITLE= synthetic\r\n##$SI= 2\r\n##$XDIM= 2\r\n##$DTYPP= 0\r\n##$BYTORDP= 0\r\n##$NC_proc= 1\r\n##OWNER= first\r\nmultiline global text\r\n##OWNER= second\r\n$$ retained comment\r\n##$SW_p= 1200\r\n##$SF= 400\r\n##$OFFSET= 10\r\n##$AXNUC= <1H>\r\n##$UNKNOWN= <first\r\nsecond>\r\n";
    let indirect = direct.replace("<1H>", "<13C>").replace("1200", "800");
    fs::write(root.join("procs"), direct).unwrap();
    fs::write(root.join("proc2s"), &indirect).unwrap();
    for (name, factor) in [("2rr", 1i32), ("2ri", 10), ("2ir", 100), ("2ii", 1000)] {
        fs::write(
            root.join(name),
            (1..=4)
                .flat_map(|value| (value * factor).to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    }
    let budget = direct.len() + indirect.len();
    let imported = nmr::ReadOptions::new()
        .limits(nmr::ReadLimits::new().max_metadata_bytes(budget))
        .read(root)
        .unwrap()
        .into_processed()
        .unwrap();
    let error = nmr::ReadOptions::new()
        .limits(nmr::ReadLimits::new().max_metadata_bytes(budget - 1))
        .read(root)
        .unwrap_err();
    assert_eq!(error.kind(), nmr::ReadErrorKind::LimitExceeded);
    assert!(matches!(
        error.reason(),
        nmr::ReadErrorReason::LimitExceeded {
            resource: nmr::ReadResource::MetadataBytes,
            ..
        }
    ));
    let documents = imported
        .processed()
        .provenance()
        .source_metadata()
        .bruker_topspin()
        .unwrap()
        .documents();
    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].text(), direct);
    assert_eq!(documents[1].text(), indirect);
    assert_eq!(documents[0].source().ordinal(), 4);
    assert_eq!(documents[1].source().ordinal(), 5);
    for source in imported.processed().provenance().sources() {
        assert_eq!(
            source.digest(),
            SourceDigest::Sha256(Sha256::digest(fs::read(source.path()).unwrap()).into())
        );
    }
    let record = imported.processed().provenance().read_record().unwrap();
    assert_eq!(record.algorithm_version(), "bruker.processed-2d.v1");
    assert_eq!(
        record.component_indices(),
        &[vec![0, 0], vec![1, 0], vec![0, 1], vec![1, 1]]
    );
    let first = ProcessingPlan::new(vec![phase(90.0)])
        .unwrap()
        .apply_processed(imported.processed())
        .unwrap();
    let result = ProcessingPlan::new(vec![ProcessingOperation::PhaseCorrection {
        axis: 1,
        correction: PhaseCorrection::new(90.0, 0.0, 0.5).unwrap(),
    }])
    .unwrap()
    .apply_processed(&first)
    .unwrap();
    drop(first);
    for row in 0..2 {
        for column in 0..2 {
            let factor = (row * 2 + column + 1) as f64;
            for (component, expected) in [
                ([0, 0], 2000.0),
                ([1, 0], -200.0),
                ([0, 1], -20.0),
                ([1, 1], 2.0),
            ] {
                assert!(
                    (result.data().get(&[row, column], &component).unwrap() - expected * factor)
                        .abs()
                        < 1e-10
                );
            }
        }
    }
    compare(
        &result,
        &result
            .provenance()
            .history()
            .unwrap()
            .replay_processed(imported.processed(), ProcessingOptions::new())
            .unwrap(),
    );
    assert_eq!(
        result
            .provenance()
            .source_metadata()
            .bruker_topspin()
            .unwrap()
            .documents(),
        documents
    );
}
