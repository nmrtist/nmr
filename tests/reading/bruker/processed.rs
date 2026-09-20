use nmr::axis::{AxisCoordinates, AxisUnit};
use nmr::plot::{PlotData, PlotError};
use nmr::processed::ComponentBasis;
use nmr::processing::{PolarityState, ProcessingOperation, ProcessingPlan, Projection};
use nmr::provenance::SourceKind;
use nmr::{DatasetKind, ReadErrorKind, ReadLimits, ReadOptions, ReadResource};
use std::fs;

use crate::support::*;

#[test]
fn bruker_processed_scalar_preserves_identity_sources_metadata_and_plot_contract() {
    let temporary = tempfile::tempdir().unwrap();
    let procno = temporary
        .path()
        .join("subject")
        .join("7")
        .join("pdata")
        .join("3");
    fs::create_dir_all(&procno).unwrap();
    fs::write(procno.join("procs"), processed_parameters(3)).unwrap();
    write_i32(&procno.join("1r"), &[1, -2, 3]);

    let loaded = nmr::read(&procno).unwrap();
    assert_eq!(loaded.kind(), DatasetKind::Processed);
    assert_eq!(loaded.selected_path(), Some(procno.as_path()));
    assert_eq!(loaded.identity().subject(), Some("subject"));
    assert_eq!(loaded.identity().source_label(), Some("7"));
    assert_eq!(loaded.identity().acquisition(), Some("exp-name"));
    assert_eq!(loaded.sources().len(), 2);
    assert_eq!(loaded.sources()[0].kind(), SourceKind::Data);

    let dataset = loaded.as_processed().unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.component_basis(), &ComponentBasis::Scalar);
    assert_eq!(axis.unit(), Some(AxisUnit::Ppm));
    assert_eq!(axis.nucleus(), Some("1H"));
    assert_eq!(axis.spectral_width_hz(), Some(1200.0));
    assert_eq!(axis.coordinate_span(), Some(2.0));
    assert_eq!(
        axis.coordinates(),
        &AxisCoordinates::Uniform {
            start: 10.0,
            step: -1.0
        }
    );
    assert_eq!(dataset.data().samples(), &[2.0, -4.0, 6.0]);
    let metadata = dataset
        .provenance()
        .source_metadata()
        .bruker_topspin()
        .unwrap();
    assert!(
        metadata
            .parameters()
            .contains(&("AXNUC".to_owned(), "<1H>".to_owned()))
    );
    assert!(PlotData::from_processed(dataset).is_ok());
}

#[test]
fn bruker_processed_cartesian_requires_explicit_projection_for_plotting() {
    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("procs"), processed_parameters(2)).unwrap();
    write_i32(&temporary.path().join("1r"), &[1, 2]);
    write_i32(&temporary.path().join("1i"), &[-3, -4]);

    let loaded = nmr::read(temporary.path()).unwrap();
    let dataset = loaded.as_processed().unwrap();
    assert_eq!(
        dataset.descriptor().axes()[0].component_basis(),
        &ComponentBasis::Cartesian
    );
    assert_eq!(dataset.data().samples(), &[2.0, -6.0, 4.0, -8.0]);
    assert_eq!(
        PlotData::from_processed(dataset).unwrap_err(),
        PlotError::NonScalarData
    );
    let projected = ProcessingPlan::new(vec![ProcessingOperation::Projection {
        projection: Projection::Magnitude,
        polarity: PolarityState::Ambiguous180,
    }])
    .unwrap()
    .apply_processed(dataset)
    .unwrap();
    assert!(PlotData::from_processed(&projected).is_ok());
}

#[test]
fn bruker_processed_limits_are_independent_and_preflighted() {
    let temporary = tempfile::tempdir().unwrap();
    let parameters = processed_parameters(3);
    fs::write(temporary.path().join("procs"), &parameters).unwrap();
    write_i32(&temporary.path().join("1r"), &[1, 2, 3]);

    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_metadata_bytes(parameters.len() - 1))
            .read(temporary.path())
            .unwrap_err(),
        ReadResource::MetadataBytes,
    );
    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_source_bytes(11))
            .read(temporary.path())
            .unwrap_err(),
        ReadResource::SourceBytes,
    );
    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_materialized_bytes(23))
            .read(temporary.path())
            .unwrap_err(),
        ReadResource::MaterializedBytes,
    );
    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_working_bytes(11))
            .read(temporary.path())
            .unwrap_err(),
        ReadResource::WorkingBytes,
    );
}

#[test]
fn bruker_processed_numeric_and_endian_matrix_is_verified() {
    for (dtypp, bytordp, values) in [
        (0, 0, vec![1.0, -2.0]),
        (0, 1, vec![1.0, -2.0]),
        (2, 0, vec![1.5, -2.5]),
        (2, 1, vec![1.5, -2.5]),
    ] {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(
            temporary.path().join("procs"),
            processed_parameters_with_storage(2, dtypp, bytordp, 0),
        )
        .unwrap();
        fs::write(
            temporary.path().join("1r"),
            encoded_processed(&values, dtypp, bytordp),
        )
        .unwrap();
        let loaded = nmr::read(temporary.path()).unwrap();
        assert_eq!(
            loaded.as_processed().unwrap().data().samples(),
            values,
            "DTYPP={dtypp}, BYTORDP={bytordp}"
        );
    }
}

#[test]
fn bruker_processed_rejects_length_scaling_and_numeric_corruption() {
    let assert_corrupt = |parameters: String, bytes: Vec<u8>| {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(temporary.path().join("procs"), parameters).unwrap();
        fs::write(temporary.path().join("1r"), bytes).unwrap();
        assert_eq!(
            nmr::read(temporary.path()).unwrap_err().kind(),
            ReadErrorKind::Corrupt
        );
    };

    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("procs"), processed_parameters(2)).unwrap();
    write_i32(&temporary.path().join("1r"), &[1]);
    assert_eq!(
        nmr::read(temporary.path()).unwrap_err().kind(),
        ReadErrorKind::Truncated
    );

    assert_corrupt(
        processed_parameters(1),
        encoded_processed(&[1.0, 2.0], 0, 0),
    );
    assert_corrupt(
        processed_parameters_with_storage(1, 0, 0, -1075),
        encoded_processed(&[1.0], 0, 0),
    );
    assert_corrupt(
        processed_parameters_with_storage(1, 0, 0, 1024),
        encoded_processed(&[1.0], 0, 0),
    );
    assert_corrupt(
        processed_parameters_with_storage(1, 2, 0, 0),
        f64::NAN.to_le_bytes().to_vec(),
    );
}

#[test]
fn bruker_processed_missing_calibration_warns_and_cannot_plot() {
    let temporary = tempfile::tempdir().unwrap();
    fs::write(
        temporary.path().join("procs"),
        "##$SI= 2\n##$DTYPP= 0\n##$BYTORDP= 0\n##$NC_proc= 0\n",
    )
    .unwrap();
    write_i32(&temporary.path().join("1r"), &[1, 2]);
    let loaded = nmr::read(temporary.path()).unwrap();
    assert!(loaded.warnings().len() >= 5);
    assert_eq!(
        PlotData::from_processed(loaded.as_processed().unwrap()).unwrap_err(),
        PlotError::MissingPhysicalCoordinates
    );
}

#[test]
fn bruker_processed_contiguous_2d_is_direct_fastest() {
    let temporary = tempfile::tempdir().unwrap();
    let mut direct = processed_parameters(2);
    direct.push_str("##$XDIM= 2\n");
    let indirect = "##$SI= 2\n\
                    ##$XDIM= 2\n\
                    ##$DTYPP= 0\n\
                    ##$BYTORDP= 0\n\
                    ##$NC_proc= 1\n\
                    ##$SW_p= 800\n\
                    ##$SF= 100\n\
                    ##$OFFSET= 20\n\
                    ##$AXNUC= <13C>\n";
    fs::write(temporary.path().join("procs"), direct).unwrap();
    fs::write(temporary.path().join("proc2s"), indirect).unwrap();
    write_i32(&temporary.path().join("2rr"), &[1, 2, 3, 4]);

    let loaded = nmr::read(temporary.path()).unwrap();
    let dataset = loaded.as_processed().unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), [2, 2]);
    assert_eq!(dataset.data().samples(), &[2.0, 4.0, 6.0, 8.0]);
    let record = dataset.provenance().read_record().unwrap();
    assert_eq!(record.algorithm_version(), "bruker.processed-2d.v1");
    assert_eq!(record.component_indices(), &[vec![0, 0]]);
    assert!(
        dataset
            .provenance()
            .sources()
            .iter()
            .all(|source| source.id().is_some()
                && matches!(source.digest(), nmr::provenance::SourceDigest::Sha256(_)))
    );
    assert_eq!(dataset.descriptor().axes()[0].nucleus(), Some("13C"));
    assert_eq!(dataset.descriptor().axes()[1].nucleus(), Some("1H"));
    let indirect = &dataset.descriptor().axes()[0];
    assert_eq!(indirect.spectral_width_hz(), Some(800.0));
    assert_eq!(indirect.coordinate_span(), Some(4.0));
    assert_eq!(indirect.coordinate_span().unwrap() * 100.0, 400.0);
    let direct = &dataset.descriptor().axes()[1];
    assert_eq!(direct.spectral_width_hz(), Some(1200.0));
    assert_eq!(direct.coordinate_span(), Some(1.5));
    assert_eq!(direct.coordinate_span().unwrap() * 400.0, 600.0);
}

#[test]
fn bruker_tiles_and_rectangular_component_sets_preserve_edge_padding() {
    for names in [
        vec!["2rr"],
        vec!["2rr", "2ri"],
        vec!["2rr", "2ir"],
        vec!["2rr", "2ri", "2ir", "2ii"],
    ] {
        for big in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let p = processed_parameters_with_storage(3, 0, i32::from(big), 1) + "##$XDIM= 2\n";
            fs::write(root.join("procs"), &p).unwrap();
            fs::write(root.join("proc2s"), &p).unwrap();
            for (plane, name) in names.iter().enumerate() {
                let mut bytes = vec![];
                for tr in 0..2 {
                    for tc in 0..2 {
                        for r in 0..2 {
                            for c in 0..2 {
                                let (row, col) = (tr * 2 + r, tc * 2 + c);
                                let value = if row < 3 && col < 3 {
                                    (100 * plane + row * 10 + col) as i32
                                } else {
                                    -999
                                };
                                bytes.extend_from_slice(&if big {
                                    value.to_be_bytes()
                                } else {
                                    value.to_le_bytes()
                                });
                            }
                        }
                    }
                }
                fs::write(root.join(name), bytes).unwrap();
            }
            let input = nmr::read(root).unwrap();
            let p = input.as_processed().unwrap();
            for (plane, name) in names.iter().enumerate() {
                let component = match *name {
                    "2rr" => [0, 0],
                    "2ri" => [1, 0],
                    "2ir" => [0, 1],
                    _ => [1, 1],
                };
                for row in 0..3 {
                    for col in 0..3 {
                        assert_eq!(
                            p.data().get(&[row, col], &component).unwrap(),
                            2.0 * (100 * plane + row * 10 + col) as f64
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn bruker_processed_2d_quartet_maps_planes_to_cartesian_axes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let mut direct = processed_parameters(2);
    direct.push_str("##$XDIM= 2\n");
    let mut indirect = processed_parameters(2);
    indirect.push_str("##$XDIM= 2\n");
    fs::write(root.join("procs"), direct).unwrap();
    fs::write(root.join("proc2s"), indirect).unwrap();
    for (name, values) in [
        ("2rr", [1, 2, 3, 4]),
        ("2ri", [10, 20, 30, 40]),
        ("2ir", [100, 200, 300, 400]),
        ("2ii", [1000, 2000, 3000, 4000]),
    ] {
        write_i32(&root.join(name), &values);
    }

    for selected in ["2rr", "2ri", "2ir", "2ii"] {
        let read = nmr::read(root.join(selected)).unwrap();
        let dataset = read.as_processed().unwrap();
        assert_eq!(dataset.data().shape(), &[2, 2]);
        assert_eq!(dataset.data().component_counts(), &[2, 2]);
        assert!(
            dataset
                .descriptor()
                .axes()
                .iter()
                .all(|axis| axis.component_basis() == &ComponentBasis::Cartesian)
        );
        assert_eq!(dataset.data().get(&[0, 0], &[0, 0]).unwrap(), 2.0);
        assert_eq!(dataset.data().get(&[0, 0], &[1, 0]).unwrap(), 20.0);
        assert_eq!(dataset.data().get(&[0, 0], &[0, 1]).unwrap(), 200.0);
        assert_eq!(dataset.data().get(&[0, 0], &[1, 1]).unwrap(), 2000.0);
        assert_eq!(dataset.data().get(&[1, 1], &[0, 0]).unwrap(), 8.0);
        assert_eq!(dataset.data().get(&[1, 1], &[1, 0]).unwrap(), 80.0);
        assert_eq!(dataset.data().get(&[1, 1], &[0, 1]).unwrap(), 800.0);
        assert_eq!(dataset.data().get(&[1, 1], &[1, 1]).unwrap(), 8000.0);
        let roles: Vec<_> = dataset
            .provenance()
            .sources()
            .iter()
            .map(|source| source.role())
            .collect();
        assert_eq!(roles, ["2rr", "2ri", "2ir", "2ii", "procs", "proc2s"]);
    }
}

#[test]
fn bruker_processed_2d_quartet_never_falls_back_to_scalar() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let mut direct = processed_parameters(2);
    direct.push_str("##$XDIM= 2\n");
    let mut indirect = processed_parameters(2);
    indirect.push_str("##$XDIM= 2\n");
    fs::write(root.join("procs"), direct).unwrap();
    fs::write(root.join("proc2s"), indirect).unwrap();
    write_i32(&root.join("2rr"), &[1, 2, 3, 4]);
    write_i32(&root.join("2ii"), &[10, 20, 30, 40]);

    assert_eq!(
        nmr::read(root).unwrap_err().kind(),
        ReadErrorKind::Incomplete
    );
    write_i32(&root.join("2ri"), &[10, 20, 30, 40]);
    write_i32(&root.join("2ir"), &[100, 200, 300, 400]);
    write_i32(&root.join("2ii"), &[1000, 2000, 3000]);
    assert_eq!(
        nmr::read(root).unwrap_err().kind(),
        ReadErrorKind::Truncated
    );
}

#[test]
fn bruker_processed_2d_quartet_limits_use_all_four_planes() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let mut direct = processed_parameters(2);
    direct.push_str("##$XDIM= 2\n");
    let mut indirect = processed_parameters(2);
    indirect.push_str("##$XDIM= 2\n");
    fs::write(root.join("procs"), direct).unwrap();
    fs::write(root.join("proc2s"), indirect).unwrap();
    for name in ["2rr", "2ri", "2ir", "2ii"] {
        write_i32(&root.join(name), &[1, 2, 3, 4]);
    }

    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_source_bytes(63))
            .read(root)
            .unwrap_err(),
        ReadResource::SourceBytes,
    );
    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_materialized_bytes(127))
            .read(root)
            .unwrap_err(),
        ReadResource::MaterializedBytes,
    );
    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_working_bytes(63))
            .read(root)
            .unwrap_err(),
        ReadResource::WorkingBytes,
    );
}

#[test]
fn bruker_quartet_working_budget_covers_live_decoded_planes_and_source() {
    for (dtypp, width) in [(0, 4), (2, 8)] {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let parameters = processed_parameters_with_storage(2, dtypp, 0, 0) + "##$XDIM= 2\n";
        fs::write(root.join("procs"), &parameters).unwrap();
        fs::write(root.join("proc2s"), &parameters).unwrap();
        for (plane, name) in ["2rr", "2ri", "2ir", "2ii"].iter().enumerate() {
            let bytes: Vec<u8> = (0..4)
                .flat_map(|point| {
                    let value = (plane * 10 + point + 1) as i32;
                    if dtypp == 0 {
                        value.to_le_bytes().to_vec()
                    } else {
                        (value as f64).to_le_bytes().to_vec()
                    }
                })
                .collect();
            fs::write(root.join(name), bytes).unwrap();
        }
        // Final interleaved output coexists with one decoded tile plane and
        // that plane's encoded source bytes. Includes output in the peak.
        let required = 4 * 4 * 8 + 4 * 8 + 4 * width;
        assert_limit(
            ReadOptions::new()
                .limits(ReadLimits::new().max_working_bytes(required - 1))
                .read(root)
                .unwrap_err(),
            ReadResource::WorkingBytes,
        );
        let loaded = ReadOptions::new()
            .limits(
                ReadLimits::new()
                    .max_working_bytes(required)
                    .max_materialized_bytes(128),
            )
            .read(root)
            .unwrap();
        let data = loaded.as_processed().unwrap().data();
        assert_eq!(data.shape(), [2, 2]);
        assert_eq!(data.samples().len(), 16);
        for row in 0..2 {
            for column in 0..2 {
                for (plane, components) in [[0, 0], [1, 0], [0, 1], [1, 1]].iter().enumerate() {
                    assert_eq!(
                        data.get(&[row, column], components).unwrap(),
                        (plane * 10 + row * 2 + column + 1) as f64
                    );
                }
            }
        }
        // The budget rejection occurs before numeric decoding, even for an
        // invalid payload of the already-validated file length.
        if dtypp == 2 {
            fs::write(root.join("2rr"), f64::NAN.to_le_bytes().repeat(4)).unwrap();
            assert_limit(
                ReadOptions::new()
                    .limits(ReadLimits::new().max_working_bytes(required - 1))
                    .read(root)
                    .unwrap_err(),
                ReadResource::WorkingBytes,
            );
            assert_eq!(
                ReadOptions::new()
                    .limits(ReadLimits::new().max_working_bytes(required))
                    .read(root)
                    .unwrap_err()
                    .kind(),
                ReadErrorKind::Corrupt
            );
        }
    }
}
