use nmr::axis::AxisCoordinates;
use nmr::processed::{ComponentBasis, Format as ProcessedFormat};
use nmr::processing::{ProcessingOperation, ProcessingPlan};
use nmr::{Format, ReadErrorKind, ReadErrorReason, ReadLimits, ReadOptions, ReadResource};
use std::fs;

use crate::support::*;

#[test]
fn jeol_frequency_domain_uses_processed_cartesian_sign_and_valid_window() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("spectrum.jdf");
    fs::write(
        &path,
        processed_jeol_with_axis_evidence(3, &[1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]),
    )
    .unwrap();

    assert_eq!(
        nmr::detect(&path).unwrap(),
        Format::Processed(ProcessedFormat::JeolDelta)
    );
    assert_eq!(
        nmr::raw::detect(&path).unwrap_err().kind(),
        ReadErrorKind::Unrecognized
    );
    assert!(matches!(nmr::read(&path).unwrap_err().reason(),
        ReadErrorReason::UnsupportedFeature { code, .. }
        if *code == nmr::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS));
    let loaded = ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(&path)
        .unwrap();
    let dataset = loaded.as_processed().unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), [2]);
    assert_eq!(dataset.data().samples(), &[2.0, -20.0, 3.0, -30.0]);
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.component_basis(), &ComponentBasis::Cartesian);
    assert_eq!(axis.nucleus(), Some("1H"));
    assert_eq!(axis.spectral_width_hz(), Some(4000.0));
    assert_eq!(axis.coordinate_span(), Some(1.0));
    assert_eq!(
        axis.coordinates(),
        &AxisCoordinates::Uniform {
            start: 9.0,
            step: -1.0,
        }
    );
    assert_eq!(
        axis.frequency_evidence().unwrap().observe_frequency_mhz(),
        Some(400.0)
    );
    assert!(
        dataset
            .provenance()
            .source_metadata()
            .jeol_delta()
            .is_some()
    );

    assert_limit(
        ReadOptions::new()
            .allow_experimental_vendor_semantics(true)
            .limits(
                ReadLimits::new()
                    .max_working_bytes(fs::metadata(&path).unwrap().len() as usize - 1),
            )
            .read(&path)
            .unwrap_err(),
        ReadResource::WorkingBytes,
    );
}

#[test]
fn jeol_processed_parts_retain_units_raw_areas_and_exact_reading_identity() {
    use nmr::formats::jeol::{
        ParameterValue, Parts, read_processed_parts, read_processed_parts_with_limits,
    };
    use nmr::processing::{PhaseCorrection, ProcessingOptions};
    use nmr::provenance::{ProcessedReadTransform, SourceDigest};
    use sha2::{Digest, Sha256};
    let mut bytes =
        processed_jeol_with_axis_evidence(3, &[1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]);
    let unknown = 1376 + 64;
    bytes[unknown..unknown + 4].copy_from_slice(&[1, 2, 3, 4]);
    bytes[unknown + 4..unknown + 6].copy_from_slice(&(-2i16).to_le_bytes());
    bytes[unknown + 7] = 200;
    bytes[unknown + 36..unknown + 64].fill(0);
    bytes[unknown + 36..unknown + 43].copy_from_slice(b"mystery");
    bytes[32] = 0xf1; // JEOL kilo prefix (10^3), with unit power one.
    let payload_end = bytes.len();
    bytes.extend([0xa1, 0xb2, 0xc3]);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("synthetic.jdf");
    fs::write(&path, &bytes).unwrap();
    let disk = ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(&path)
        .unwrap()
        .into_processed()
        .unwrap();
    assert!(
        matches!(read_processed_parts(Parts::new(&bytes)).unwrap_err().reason(),
        ReadErrorReason::UnsupportedFeature { code, .. }
        if *code == nmr::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS)
    );
    assert!(
        matches!(read_processed_parts_with_limits(Parts::new(&bytes), ReadLimits::new()).unwrap_err().reason(),
        ReadErrorReason::UnsupportedFeature { code, .. }
        if *code == nmr::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS)
    );
    let memory =
        read_processed_parts(Parts::new(&bytes).allow_experimental_vendor_semantics(true)).unwrap();
    assert_eq!(
        disk.processed().canonical_digests(),
        memory.canonical_digests()
    );
    assert_eq!(memory.data().samples(), &[2.0, -20.0, 3.0, -30.0]);
    assert_eq!(
        memory.descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Uniform {
            start: 9000.0,
            step: -1000.0
        }
    );
    let source = &memory.provenance().sources()[0];
    assert!(source.locator().is_none());
    assert_eq!(
        source.digest(),
        SourceDigest::Sha256(Sha256::digest(&bytes).into())
    );
    let metadata = memory.provenance().source_metadata().jeol_delta().unwrap();
    let parameters = metadata.parameters();
    assert_eq!(metadata.source(), source.id().unwrap());
    assert_eq!(parameters.raw_header(), &bytes[..1360]);
    assert_eq!(
        parameters.raw_pre_data_records(),
        &bytes[1360..payload_end - 64]
    );
    assert_eq!(parameters.raw_trailing_records(), &bytes[payload_end..]);
    assert_eq!(metadata.trailing_byte_offset(), payload_end);
    let parameter = parameters.get("mystery").unwrap();
    assert_eq!(parameter.class(), [1, 2, 3, 4]);
    assert_eq!(parameter.unit_scaler(), -2);
    assert_eq!(parameter.raw_units()[1], 200);
    assert_eq!(parameter.value(), &ParameterValue::Float(4000.0));
    assert_eq!(
        memory.provenance().read_record().unwrap().transform(),
        &ProcessedReadTransform::JeolDelta {
            float64: true,
            big_endian: false,
            disk_points: 4,
            crop_start: 1,
            crop_points: 2,
            imaginary_multiplier: -1,
            submatrix_edge: 8,
            coordinate_scale: 1000.0,
            explicit_coordinates: false,
        }
    );
    assert_eq!(
        memory.provenance().read_record(),
        disk.processed().provenance().read_record()
    );
    let required = bytes.len() - 64;
    let limits = ReadLimits::new().max_metadata_bytes(required);
    assert!(
        read_processed_parts_with_limits(
            Parts::new(&bytes).allow_experimental_vendor_semantics(true),
            limits
        )
        .is_ok()
    );
    assert_limit(
        read_processed_parts_with_limits(
            Parts::new(&bytes).allow_experimental_vendor_semantics(true),
            limits.max_metadata_bytes(required - 1),
        )
        .unwrap_err(),
        ReadResource::MetadataBytes,
    );
    let output = ProcessingPlan::new(vec![ProcessingOperation::PhaseCorrection {
        axis: 0,
        correction: PhaseCorrection::new(90.0, 0.0, 0.5).unwrap(),
    }])
    .unwrap()
    .apply_processed(disk.processed())
    .unwrap();
    let replayed = output
        .provenance()
        .history()
        .unwrap()
        .replay_processed(&memory, ProcessingOptions::new())
        .unwrap();
    assert_eq!(replayed.descriptor(), output.descriptor());
    assert_eq!(replayed.data().shape(), &[2]);
    assert_eq!(replayed.data().component_counts(), &[2]);
    for (&actual, expected) in replayed.data().samples().iter().zip([20.0, 2.0, 30.0, 3.0]) {
        assert!((actual - expected).abs() < 1e-12);
    }
    bytes[unknown + 7] = 201;
    let changed =
        read_processed_parts(Parts::new(&bytes).allow_experimental_vendor_semantics(true)).unwrap();
    assert_eq!(changed.canonical_digests(), memory.canonical_digests());
    assert_eq!(
        changed
            .provenance()
            .source_metadata()
            .jeol_delta()
            .unwrap()
            .parameters()
            .get("mystery")
            .unwrap()
            .raw_units()[1],
        201
    );
    assert!(matches!(
        output
            .provenance()
            .history()
            .unwrap()
            .replay_processed(&changed, ProcessingOptions::new()),
        Err(nmr::processing::ProcessingError::InputIdentityMismatch)
    ));
}

#[test]
fn jeol_frequency_domain_scalar_layout_preserves_valid_window() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("scalar.jdf");
    fs::write(&path, processed_jeol(1, &[1.0, 2.0, 3.0, 4.0])).unwrap();
    let loaded = ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(path)
        .unwrap();
    let dataset = loaded.as_processed().unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), [2]);
    assert_eq!(dataset.data().samples(), &[2.0, 3.0]);
    assert_eq!(
        dataset.descriptor().axes()[0].component_basis(),
        &ComponentBasis::Scalar
    );
}

#[test]
fn jeol_processed_working_budget_covers_source_and_intermediate_planes() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("budget.jdf");
    for axis_type in [1, 3] {
        let values = if axis_type == 1 {
            vec![1.0, 2.0, 3.0, 4.0]
        } else {
            vec![1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]
        };
        let bytes = processed_jeol(axis_type, &values);
        fs::write(&path, &bytes).unwrap();
        // Source + original/reordered f64 sections + original/cropped
        // Complex64 storage. The existing estimate conservatively uses the
        // uncropped size for both complex buffers.
        let required = bytes.len() + 2 * values.len() * 8 + 2 * 4 * 16;
        assert_limit(
            ReadOptions::new()
                .allow_experimental_vendor_semantics(true)
                .limits(ReadLimits::new().max_working_bytes(required - 1))
                .read(&path)
                .unwrap_err(),
            ReadResource::WorkingBytes,
        );
        let loaded = ReadOptions::new()
            .allow_experimental_vendor_semantics(true)
            .limits(ReadLimits::new().max_working_bytes(required))
            .read(&path)
            .unwrap();
        let data = loaded.as_processed().unwrap().data();
        assert_eq!(data.shape(), [2]);
        assert_eq!(
            data.samples(),
            if axis_type == 1 {
                vec![2.0, 3.0]
            } else {
                vec![2.0, -20.0, 3.0, -30.0]
            }
        );
    }
}

#[test]
fn jeol_processed_encoding_records_cover_scalar_and_cartesian_parts() {
    use nmr::formats::jeol::{Parts, read_processed_parts};
    use nmr::provenance::ProcessedReadTransform;
    for axis_type in [1, 3] {
        let values = if axis_type == 1 {
            vec![1.0, 2.0, 3.0, 4.0]
        } else {
            vec![1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]
        };
        let mut identity = None;
        for float64 in [false, true] {
            for big_endian in [false, true] {
                let mut bytes = processed_jeol(axis_type, &[]);
                bytes[8] = u8::from(!big_endian);
                bytes[14] = if float64 { 1 } else { 65 };
                for &value in &values {
                    bytes.extend(if float64 {
                        if big_endian {
                            f64::to_be_bytes(value).to_vec()
                        } else {
                            f64::to_le_bytes(value).to_vec()
                        }
                    } else if big_endian {
                        (value as f32).to_be_bytes().to_vec()
                    } else {
                        (value as f32).to_le_bytes().to_vec()
                    });
                }
                let dataset = read_processed_parts(
                    Parts::new(&bytes).allow_experimental_vendor_semantics(true),
                )
                .unwrap();
                assert_eq!(dataset.data().shape(), &[2]);
                assert_eq!(
                    dataset.data().samples(),
                    if axis_type == 1 {
                        &[2.0, 3.0][..]
                    } else {
                        &[2.0, -20.0, 3.0, -30.0][..]
                    }
                );
                if let Some(identity) = identity {
                    assert_eq!(dataset.canonical_digests(), identity);
                } else {
                    identity = Some(dataset.canonical_digests());
                }
                assert!(
                    matches!(dataset.provenance().read_record().unwrap().transform(), ProcessedReadTransform::JeolDelta { float64: actual_float64, big_endian: actual_big_endian, .. } if *actual_float64 == float64 && *actual_big_endian == big_endian)
                );
            }
        }
    }
}
