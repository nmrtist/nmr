use nmr::Complex64;
use nmr::axis::AxisCoordinates;
use nmr::raw::{ParameterErrorKind, RawFormat as Format, ReadErrorKind, ReadErrorReason};

use super::support::*;

#[test]
fn reads_little_endian_int32_fid() {
    let temp = tempfile::tempdir().unwrap();
    let values = [10_i32, -20, 30, -40];
    let bytes: Vec<u8> = values.into_iter().flat_map(i32::to_le_bytes).collect();
    write_dataset(temp.path(), &parameter_text(0, 0, values.len()), &bytes);

    assert_eq!(nmr::raw::detect(temp.path()).unwrap(), Format::BrukerRaw);
    let dataset = nmr::raw::read(temp.path().join("fid")).unwrap();
    assert_eq!(
        dataset.data().dense_samples().unwrap(),
        vec![Complex64::new(10.0, -20.0), Complex64::new(30.0, -40.0)]
    );
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.spectral_width_hz(), Some(8000.0));
    assert_eq!(axis.nucleus(), Some("1H"));
    assert_eq!(pending_delay(axis), Some(44.75));
    let reference = axis.frequency_evidence().unwrap();
    assert_eq!(reference.observe_frequency_mhz(), Some(400.13));
    assert_eq!(reference.transmitter_offset_hz(), Some(1880.0));
    let chemical = axis.chemical_shift_reference().unwrap();
    assert_eq!(chemical.reference_frequency_mhz(), 400.0);
    assert_eq!(dataset.descriptor().acquisition().solvent(), Some("CDCl3"));
    assert_eq!(
        dataset.descriptor().acquisition().title(),
        Some("synthetic")
    );
    assert_eq!(dataset.descriptor().acquisition().scans(), Some(16));
    assert_eq!(
        dataset
            .provenance()
            .source_metadata()
            .as_bruker()
            .unwrap()
            .direct()
            .get("NUC1"),
        Some("<1H>")
    );
    assert_eq!(
        dataset.descriptor().acquisition().pulse_program(),
        Some("zg30")
    );
    assert_eq!(dataset.provenance().sources().len(), 2);
}

#[test]
fn reads_big_endian_float64_fid_and_ignores_padding() {
    let values = [1.25_f64, -2.5, 3.75, -4.0];
    let mut bytes: Vec<u8> = values.into_iter().flat_map(f64::to_be_bytes).collect();
    bytes.extend_from_slice(&[0; 32]);

    let acqus = parameter_text(1, 2, values.len());
    let dataset = read_from_parts(&bytes, &[&acqus], None).unwrap();
    assert_eq!(
        dataset.data().dense_samples().unwrap(),
        vec![Complex64::new(1.25, -2.5), Complex64::new(3.75, -4.0)]
    );
    use sha2::{Digest, Sha256};
    let sources = dataset.provenance().sources();
    assert_eq!(sources.len(), 2);
    assert!(sources.iter().all(|source| source.locator().is_none()));
    assert_eq!(
        sources[0].digest(),
        nmr::provenance::SourceDigest::Sha256(Sha256::digest(&bytes).into())
    );
    assert_eq!(
        sources[1].digest(),
        nmr::provenance::SourceDigest::Sha256(Sha256::digest(acqus.as_bytes()).into())
    );
}

#[test]
fn raw_parts_replay_checks_source_comments_and_preserves_original_parameters() {
    use nmr::processing::{
        FourierTransform, ProcessingError, ProcessingOperation, ProcessingOptions, ProcessingPlan,
    };
    let parameters = format!(
        "{}\n##UNKNOWN=first\nsecond line\n$$ original comment\n",
        parameter_text(0, 0, 4)
    );
    let bytes: Vec<_> = [1i32, 2, 3, 4]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    let original = read_from_parts(&bytes, &[&parameters], None).unwrap();
    assert_eq!(
        original
            .provenance()
            .source_metadata()
            .as_bruker()
            .unwrap()
            .direct()
            .raw_text(),
        parameters
    );
    let result = ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply_raw(&original)
    .unwrap();
    let replayed = result
        .provenance()
        .history()
        .unwrap()
        .replay_raw(&original, ProcessingOptions::new())
        .unwrap();
    assert_eq!(replayed.descriptor(), result.descriptor());
    assert_eq!(replayed.data().shape(), &[2]);
    assert_eq!(replayed.data().component_counts(), &[2]);
    for (&actual, expected) in replayed.data().samples().iter().zip([-2.0, -2.0, 4.0, 6.0]) {
        assert!((actual - expected).abs() < 1e-12);
    }
    for changed_text in [
        parameters.replace("original comment", "different comment"),
        parameters.replace("##TITLE= synthetic", "##TITLE= renamed"),
    ] {
        let changed = read_from_parts(&bytes, &[&changed_text], None).unwrap();
        assert_eq!(original.canonical_digests(), changed.canonical_digests());
        assert!(matches!(
            result
                .provenance()
                .history()
                .unwrap()
                .replay_raw(&changed, ProcessingOptions::new())
                .map_err(ProcessingError::into_root_cause),
            Err(ProcessingError::InputIdentityMismatch)
        ));
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("acqus"), &parameters).unwrap();
    std::fs::write(root.path().join("fid"), &bytes).unwrap();
    let from_path = nmr::read(root.path()).unwrap().into_raw().unwrap();
    assert_eq!(
        original.canonical_digests(),
        from_path.raw().canonical_digests()
    );
    let replayed = result
        .provenance()
        .history()
        .unwrap()
        .replay_raw(from_path.raw(), ProcessingOptions::new())
        .unwrap();
    assert_eq!(replayed.data(), result.data());
}

#[test]
fn reports_truncated_binary_payload() {
    let acqus = parameter_text(0, 0, 4);
    let error = read_from_parts(&[0; 8], &[&acqus], None).unwrap_err();
    assert_eq!(error.format(), Some(nmr::Format::Raw(Format::BrukerRaw)));
    assert_eq!(error.kind(), ReadErrorKind::Truncated);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::Truncated {
            expected: 16,
            actual: 8,
            ..
        }
    ));
}

#[test]
fn rejects_odd_td_instead_of_discarding_a_value() {
    let acqus = parameter_text(0, 0, 3);
    let error = read_from_parts(&[0; 12], &[&acqus], None).unwrap_err();
    assert_eq!(error.format(), Some(nmr::Format::Raw(Format::BrukerRaw)));
    assert_eq!(error.kind(), ReadErrorKind::InvalidMetadata);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::InvalidMetadata { parameter: Some(parameter), .. }
            if parameter.parameter() == Some("TD")
                && parameter.kind() == ParameterErrorKind::Invalid
    ));
}

#[test]
fn complex_point_interval_accounts_for_two_stored_real_values() {
    let acqus = parameter_text(0, 0, 4);
    let dataset = read_from_parts(&[0; 16], &[&acqus], None).unwrap();
    assert_eq!(
        dataset.descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0 / 8_000.0,
        }
    );
}

#[test]
fn rejects_nonzero_bytes_after_declared_payload() {
    let mut bytes = vec![0; 16];
    bytes.push(1);
    let acqus = parameter_text(0, 0, 4);
    let error = read_from_parts(&bytes, &[&acqus], None).unwrap_err();
    assert_eq!(error.format(), Some(nmr::Format::Raw(Format::BrukerRaw)));
    assert_eq!(error.kind(), ReadErrorKind::Corrupt);
    assert!(matches!(error.reason(), ReadErrorReason::Corrupt { .. }));
}
