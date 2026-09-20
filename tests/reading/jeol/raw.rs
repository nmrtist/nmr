use super::support::*;
use crate::raw_support::*;
use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain as Domain, AxisQuantity, AxisRole, AxisUnit};
use nmr::raw::{
    DirectSamples, IndirectComponents, RawAxisKind, RawFormat as Format, ReadErrorKind,
    ReadErrorReason, ReadLimits,
};
use std::fs;

#[test]
fn embedded_parameter_lists_and_ramps_override_header_end_to_end() {
    for (text, n, unit, expected) in [
        (
            "tau_interval => y_acq {1[ms], 1.7644[ms], 3.11312[ms], 5[s]}",
            4,
            AxisUnit::Second,
            vec![0.001, 0.0017644, 0.00311312, 5.0],
        ),
        (
            "g => y_acq 20[mT/m]..0.28[T/m] : 17.33333[mT/m], help \"gradient\";",
            16,
            AxisUnit::TeslaPerMeter,
            (0..16).map(|i| 0.02 + 0.01733333 * i as f64).collect(),
        ),
        (
            r#"g => y_acq 15[mT/m]..0.185[T/m] : 1[G/cm], help "gradient";"#,
            18,
            AxisUnit::TeslaPerMeter,
            (0..18).map(|i| 0.015 + 0.01 * i as f64).collect(),
        ),
        (
            r#"total_echo => y_acq 0.1[ms]..2.5[ms] : 160[us], 0.3[us]->50[ms] : 20[ns], help "echo";"#,
            16,
            AxisUnit::Second,
            (0..16).map(|i| 0.0001 + 0.0024 * i as f64 / 15.0).collect(),
        ),
    ] {
        let mut bytes = jeol_fixture_small_2d();
        let stored = (n as usize).next_multiple_of(4);
        bytes.resize(1360 + 8 * stored * 2 * 8, 0);
        put_be_u32(&mut bytes, 180, stored as u32);
        put_be_u32(&mut bytes, 244, (n - 1) as u32);
        // Contradictory linear header would lose the actual list.
        bytes[34] = 1;
        bytes[35] = 28;
        bytes[280..288].copy_from_slice(&1.0_f64.to_be_bytes());
        bytes[344..352].copy_from_slice(&2.0_f64.to_be_bytes());
        bytes.extend_from_slice(text.as_bytes());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("embedded.jdf");
        fs::write(&path, &bytes).unwrap();
        let read = nmr::ReadOptions::new()
            .allow_experimental_vendor_semantics(true)
            .read(&path)
            .unwrap();
        assert!(
            super::warnings::details(&read)
                .iter()
                .any(|detail| detail.starts_with("Embedded"))
        );
        let raw = read.as_raw().unwrap();
        let axis = &raw.descriptor().axes()[0];
        assert_eq!(axis.unit(), Some(unit));
        let AxisCoordinates::Explicit(actual) = axis.coordinates() else {
            panic!("list was lost")
        };
        for (a, b) in actual.iter().zip(&expected) {
            assert!((a - b).abs() < 1e-14);
        }
        let evidence = &raw
            .provenance()
            .source_metadata()
            .as_jeol()
            .unwrap()
            .embedded_axes()[0];
        assert_eq!(evidence.values(), actual);
        assert_eq!(evidence.disk_axis(), 1);
        let mut saved = Vec::new();
        nmr::snapshot::write_snapshot(&read, &mut saved, Default::default()).unwrap();
        fs::remove_file(&path).unwrap();
        let restored = nmr::snapshot::read_snapshot(&mut saved.as_slice(), Default::default())
            .unwrap()
            .restore(nmr::snapshot::AcceptRecordedHistory);
        assert_eq!(restored.canonical_digests(), read.canonical_digests());
        let bad = String::from_utf8_lossy(text.as_bytes())
            .replace("[ms]", "[unknown]")
            .replace("[mT/m]", "[unknown]");
        bytes.truncate(bytes.len() - text.len());
        bytes.extend_from_slice(bad.as_bytes());
        assert!(read_from_parts(DatasetParts::Jeol { jdf: &bytes }).is_err());
    }
}

#[test]
fn experimental_jeol_processed_2d_preserves_all_sections_and_parameter_axes() {
    for parameter in [false, true] {
        let mut bytes = if parameter {
            jeol_fixture_small_2d()
        } else {
            jeol_fixture_hypercomplex_2d()
        };
        bytes[32] = 1;
        bytes[33] = 26;
        bytes[272..280].copy_from_slice(&10.0_f64.to_be_bytes());
        bytes[336..344].copy_from_slice(&1.0_f64.to_be_bytes());
        if !parameter {
            bytes[34] = 1;
            bytes[35] = 13;
            bytes[280..288].copy_from_slice(&0.0_f64.to_be_bytes());
            bytes[344..352].copy_from_slice(&30.0_f64.to_be_bytes());
        }
        let parts =
            nmr::formats::jeol::Parts::new(&bytes).allow_experimental_vendor_semantics(true);
        let p = nmr::formats::jeol::read_processed_parts(parts).unwrap();
        assert_eq!(
            p.descriptor().component_counts(),
            if parameter { vec![1, 2] } else { vec![2, 2] }
        );
        assert_eq!(
            p.data().get(&[0, 0], &[0, 1]).unwrap(),
            if parameter { -1000.0 } else { -100.0 }
        );
        if !parameter {
            assert_eq!(p.data().get(&[0, 0], &[1, 0]).unwrap(), -200.0);
            assert_eq!(p.data().get(&[0, 0], &[1, 1]).unwrap(), 300.0);
        }
        let input: nmr::Dataset = p.into();
        let mut saved = Vec::new();
        nmr::snapshot::write_snapshot(&input, &mut saved, Default::default()).unwrap();
        let restored = nmr::snapshot::read_snapshot(&mut saved.as_slice(), Default::default())
            .unwrap()
            .restore(nmr::snapshot::AcceptRecordedHistory);
        assert_eq!(restored.canonical_digests(), input.canonical_digests());
    }
}

#[test]
fn jeol_requires_opt_in_on_every_raw_entry_point() {
    let bytes = jeol_fixture_f64();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.jdf");
    fs::write(&path, &bytes).unwrap();
    let errors = [
        nmr::raw::open(&path).unwrap_err(),
        nmr::raw::read(&path).unwrap_err(),
        nmr::read(&path).unwrap_err(),
        nmr::formats::jeol::read_parts(nmr::formats::jeol::Parts::new(&bytes)).unwrap_err(),
        nmr::formats::jeol::read_parts_with_limits(
            nmr::formats::jeol::Parts::new(&bytes),
            ReadLimits::new(),
        )
        .unwrap_err(),
    ];
    for error in errors {
        assert!(
            matches!(error.reason(), ReadErrorReason::UnsupportedFeature { code, .. }
            if *code == nmr::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS)
        );
    }
    assert_eq!(nmr::raw::detect(&path).unwrap(), Format::JeolDelta);
    let loaded = nmr::ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(&path)
        .unwrap();
    assert!(loaded.warnings().iter().any(|warning| matches!(
        warning,
        nmr::ReadWarning::ExperimentalVendorSemantics {
            format: nmr::Format::Raw(Format::JeolDelta),
            ..
        }
    )));
}

#[test]
fn jeol_only_explicit_false_marks_the_digital_filter_not_applicable() {
    use nmr::formats::jeol::ParameterValue;
    use nmr::raw::GroupDelayState;
    let mut original = jeol_fixture_f64();
    original[32] = 1;
    original[33] = 28;
    for value in [
        None,
        Some(ParameterValue::Integer(0)),
        Some(ParameterValue::String("not-a-boolean".into())),
        Some(ParameterValue::String("TRUE".into())),
        Some(ParameterValue::String("FALSE".into())),
        Some(ParameterValue::String("false".into())),
    ] {
        let records: Vec<_> = value
            .clone()
            .into_iter()
            .map(|value| jeol_parameter_record("digital_filter", value, 0))
            .collect();
        let bytes = jeol_with_parameter_records(&original, &records);
        let dataset = read_from_parts(DatasetParts::Jeol { jdf: &bytes }).unwrap();
        let explicitly_disabled = matches!(&value, Some(ParameterValue::String(text)) if text.eq_ignore_ascii_case("false"));
        assert_eq!(
            dataset.descriptor().axes()[0].group_delay(),
            if explicitly_disabled {
                &GroupDelayState::NotApplicable
            } else {
                &GroupDelayState::Unknown
            },
            "{value:?}"
        );
        let recipe = nmr::processing::DensePipeline::new(
            vec![None]
                .into_iter()
                .zip(vec![Some(nmr::processing::FrequencyFrame::Hertz)])
                .enumerate()
                .map(
                    |(index, (phase, frequency_frame))| nmr::processing::DenseAxisConfig {
                        axis: nmr::AxisIndex::new(index),
                        phase,
                        frequency_frame,
                    },
                )
                .collect(),
            nmr::processing::DensePipelineOptions {
                direct_delay: nmr::processing::DirectDelayMode::Automatic,
                projection: nmr::processing::Projection::Magnitude,
                expected_polarity: nmr::processing::ExpectedPolarity::Signed,
                descending_ppm: false,
            },
        )
        .unwrap()
        .plan(&dataset);
        if explicitly_disabled {
            assert!(recipe.is_ok());
        } else {
            assert!(matches!(
                recipe.unwrap_err(),
                nmr::processing::ProcessingError::MissingCapability {
                    capability: "direct group-delay evidence",
                    ..
                }
            ));
        }
    }
}

#[test]
fn jeol_snapshot_rejects_changed_header_even_when_file_identity_is_restored() {
    use std::io::{Seek, SeekFrom, Write};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.jdf");
    fs::write(&path, jeol_fixture_f64()).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let reader = nmr::raw::OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .open(&path)
        .unwrap();
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::Start(272)).unwrap();
    file.write_all(&2.0_f64.to_be_bytes()).unwrap();
    file.set_modified(modified).unwrap();
    drop(file);
    assert_eq!(
        reader.into_dataset().unwrap_err().kind(),
        ReadErrorKind::SourceChanged
    );
}

#[test]
fn jeol_real_1d_remains_a_single_real_lane() {
    let mut fixture = jeol_fixture_f64();
    fixture[24] = 1;
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();

    assert_eq!(
        dataset.descriptor().axes()[0].kind(),
        &RawAxisKind::Direct(DirectSamples::Real)
    );
    assert_eq!(dataset.descriptor().component_lanes(), vec![1]);
    assert_eq!(
        dataset.data().dense_samples().unwrap(),
        &[
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0),
            Complex64::new(4.0, 0.0),
        ]
    );
}

#[test]
fn jeol_sections_become_canonical_complex_samples() {
    let dataset = read_from_parts(DatasetParts::Jeol {
        jdf: &jeol_fixture_f64(),
    })
    .unwrap();
    assert_eq!(dataset.provenance().format(), Some(Format::JeolDelta));
    assert_eq!(
        dataset.descriptor().axes()[0].kind(),
        &RawAxisKind::Direct(DirectSamples::Complex)
    );
    assert_eq!(dataset.data().shape(), &[4]);
    assert_eq!(
        dataset.data().dense_samples().unwrap(),
        &[
            Complex64::new(1.0, -10.0),
            Complex64::new(2.0, -20.0),
            Complex64::new(3.0, -30.0),
            Complex64::new(4.0, -40.0),
        ]
    );
    let normalization = dataset.provenance().sample_normalization();
    assert_eq!(normalization.source_block_scale_factors(), &[1.0]);
    assert_eq!(normalization.stored_imaginary_multiplier(), -1);
}

#[test]
fn jeol_projects_declared_digital_filter_delay_onto_the_direct_axis() {
    let mut original = jeol_fixture_f64();
    original[32] = 1;
    original[33] = 28;
    let fixture = jeol_with_parameter_records(
        &original,
        &[
            jeol_parameter_record(
                "digital_filter",
                nmr::formats::jeol::ParameterValue::String("TRUE".into()),
                0,
            ),
            jeol_parameter_record(
                "orders",
                nmr::formats::jeol::ParameterValue::String("2 28 74".into()),
                0,
            ),
            jeol_parameter_record(
                "factors",
                nmr::formats::jeol::ParameterValue::String("4 2".into()),
                0,
            ),
        ],
    );
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    assert!(matches!(
        dataset.descriptor().axes()[0].group_delay(),
        nmr::raw::GroupDelayState::Pending(delay) if delay.delay_points() == 19.9375
    ));
}

#[test]
fn jeol_does_not_project_malformed_digital_filter_metadata() {
    let mut original = jeol_fixture_f64();
    original[32] = 1;
    original[33] = 28;
    let fixture = jeol_with_parameter_records(
        &original,
        &[
            jeol_parameter_record(
                "digital_filter",
                nmr::formats::jeol::ParameterValue::String("TRUE".into()),
                0,
            ),
            jeol_parameter_record(
                "orders",
                nmr::formats::jeol::ParameterValue::String("3 28 74".into()),
                0,
            ),
            jeol_parameter_record(
                "factors",
                nmr::formats::jeol::ParameterValue::String("4 2".into()),
                0,
            ),
        ],
    );
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    assert!(matches!(
        dataset.descriptor().axes()[0].group_delay(),
        nmr::raw::GroupDelayState::Unknown
    ));
}

#[test]
fn jeol_small_2d_reorders_tiles_and_crops_valid_window() {
    let dataset = read_from_parts(DatasetParts::Jeol {
        jdf: &jeol_fixture_small_2d(),
    })
    .unwrap();
    assert_eq!(dataset.data().shape(), &[3, 6]);
    assert_eq!(
        dataset.descriptor().axes()[0].role(),
        AxisRole::ArrayParameter
    );
    assert_eq!(dataset.descriptor().axes()[0].domain(), Domain::Parameter);
    let samples = dataset.data().dense_samples().unwrap();
    assert_eq!(samples[0], Complex64::new(0.0, -1000.0));
    assert_eq!(samples[5], Complex64::new(5.0, -1005.0));
    assert_eq!(samples[6], Complex64::new(10.0, -1010.0));
    assert_eq!(samples[17], Complex64::new(25.0, -1025.0));
}

#[test]
fn jeol_array_parameter_uses_referenced_compound_gradient_unit() {
    let fixture = jeol_gradient_array_fixture();
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.role(), AxisRole::ArrayParameter);
    assert_eq!(axis.domain(), Domain::Parameter);
    assert_eq!(axis.unit(), Some(AxisUnit::TeslaPerMeter));
    assert_eq!(
        axis.quantity(),
        Some(AxisQuantity::MagneticFieldGradientStrength)
    );
    assert_eq!(axis.label(), Some("g"));
    assert_eq!(
        axis.coordinates(),
        &AxisCoordinates::Uniform {
            start: 0.02,
            step: 0.02,
        }
    );
    assert_eq!(dataset.descriptor().axes()[1].nucleus(), Some("19F"));
    let diffusion = dataset.descriptor().acquisition().diffusion().unwrap();
    assert_eq!(diffusion.gradient_axis(), 0);
    assert_eq!(diffusion.gradient_parameter(), "g");
    assert_eq!(diffusion.gradient_pulse_duration_seconds(), 0.005);
    assert_eq!(diffusion.gradient_pulse_duration_parameter(), "delta");
    assert_eq!(diffusion.diffusion_time_seconds(), 0.1);
    assert_eq!(diffusion.diffusion_time_parameter(), "delta_large");
    assert_eq!(diffusion.recovery_delay_seconds(), Some(0.002));
    assert_eq!(diffusion.recovery_delay_parameter(), Some("tau"));
    assert_eq!(diffusion.gradient_shape(), Some("SQUARE"));
    assert_eq!(diffusion.gradient_shape_parameter(), Some("grad_shape"));

    let parameters = dataset.provenance().source_metadata().as_jeol().unwrap();
    assert_eq!(
        parameters.get("g").unwrap().raw_units(),
        &[0x11, 31, 0x0f, 19, 0, 0, 0, 0, 0, 0]
    );
    assert!(matches!(
        parameters.get("x_domain").unwrap().value(),
        nmr::formats::jeol::ParameterValue::String(value) if value == "Fluorine19"
    ));
}

#[test]
fn jeol_array_parameter_uses_referenced_time_unit() {
    let fixture = jeol_time_array_fixture();
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.role(), AxisRole::ArrayParameter);
    assert_eq!(axis.domain(), Domain::Parameter);
    assert_eq!(axis.unit(), Some(AxisUnit::Second));
    assert_eq!(axis.quantity(), Some(AxisQuantity::TimeDelay));
    assert_eq!(axis.label(), Some("total_echo"));
    assert_eq!(
        axis.coordinates(),
        &AxisCoordinates::Uniform {
            start: 0.02,
            step: 0.02,
        }
    );
    assert_eq!(
        dataset.descriptor().acquisition().pulse_program(),
        Some("spin_echo_t2.jxp")
    );
}

#[test]
fn jeol_array_parameter_does_not_truncate_an_unknown_compound_unit() {
    let fixture = jeol_array_parameter_fixture([0x11, 31, 0x0f, 28, 0, 0, 0, 0, 0, 0]);
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.role(), AxisRole::ArrayParameter);
    assert_eq!(axis.domain(), Domain::Parameter);
    assert_eq!(axis.unit(), None);
    assert_eq!(axis.quantity(), None);
    assert_eq!(axis.label(), Some("g"));
    assert_eq!(axis.coordinates(), &AxisCoordinates::Unknown);
    assert!(dataset.descriptor().acquisition().diffusion().is_none());
}

#[test]
fn jeol_hypercomplex_2d_uses_delta_converter_section_order_and_sign() {
    let dataset = read_from_parts(DatasetParts::Jeol {
        jdf: &jeol_fixture_hypercomplex_2d(),
    })
    .unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), vec![2, 3]);
    assert_eq!(dataset.descriptor().component_lanes(), vec![2, 1]);
    assert!(matches!(
        dataset.descriptor().axes()[0].kind(),
        RawAxisKind::Indirect(IndirectComponents::Cartesian(_))
    ));
    assert_eq!(
        dataset.data().read_trace(&[0]).unwrap().samples(),
        &[
            Complex64::new(0.0, -100.0),
            Complex64::new(1.0, -101.0),
            Complex64::new(2.0, -102.0),
            Complex64::new(-200.0, 300.0),
            Complex64::new(-201.0, 301.0),
            Complex64::new(-202.0, 302.0),
        ]
    );
}

#[test]
fn jeol_real_complex_2d_shares_the_direct_complex_lane_with_opposite_orientation() {
    let mut fixture = jeol_fixture_small_2d();
    fixture[24] = 4;
    fixture[25] = 4;
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    assert_eq!(dataset.descriptor().component_lanes(), vec![1, 1]);
    assert!(matches!(
        dataset.descriptor().axes()[0].kind(),
        RawAxisKind::Indirect(IndirectComponents::SharedComplex {
            conjugated: true,
            ..
        })
    ));
    assert_eq!(
        dataset.descriptor().axes()[1].kind(),
        &RawAxisKind::Direct(DirectSamples::Complex)
    );
    assert_eq!(
        dataset.data().read_trace(&[0]).unwrap().samples()[0],
        Complex64::new(0.0, -1000.0)
    );
}

#[test]
fn jeol_hypercomplex_crop_preserves_both_component_lanes() {
    let mut fixture = jeol_fixture_hypercomplex_2d();
    put_be_u32(&mut fixture, 212, 1); // indirect valid start
    put_be_u32(&mut fixture, 244, 2); // indirect valid stop
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), vec![2, 3]);
    assert_eq!(dataset.descriptor().component_lanes(), vec![2, 1]);
}

#[test]
fn jeol_hypercomplex_nus_preserves_original_grid_schedule_and_signs() {
    let fixture = jeol_fixture_hypercomplex_nus();
    let expected = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    assert_eq!(expected.descriptor().logical_shape(), vec![8, 3]);
    assert_eq!(expected.descriptor().component_lanes(), vec![2, 1]);
    assert_eq!(
        expected
            .sampling_schedule()
            .unwrap()
            .coordinates()
            .iter()
            .map(|coordinate| coordinate.as_slice()[0])
            .collect::<Vec<_>>(),
        vec![0, 1, 3, 7]
    );
    assert_eq!(
        expected.data().read_trace(&[0]).unwrap().samples()[3],
        Complex64::new(-200.0, 300.0)
    );

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nus.jdf");
    fs::write(&path, fixture).unwrap();
    let reader = nmr::raw::OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .open(&path)
        .unwrap();
    let loaded = nmr::ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(&path)
        .unwrap();
    assert!(
        loaded
            .warnings()
            .iter()
            .any(|w| matches!(w, nmr::ReadWarning::ExperimentalVendorSemantics { .. }))
    );
    assert_eq!(reader.descriptor().axes(), expected.descriptor().axes());
    assert_eq!(
        reader.descriptor().acquisition(),
        expected.descriptor().acquisition()
    );
    assert_eq!(reader.sampling_schedule(), expected.sampling_schedule());
    assert_eq!(reader.into_dataset().unwrap().data(), expected.data());
}

#[test]
fn jeol_lazy_trace_and_materialization_match_in_memory_decode() {
    for (index, fixture) in [jeol_fixture_small_2d(), jeol_fixture_hypercomplex_2d(), {
        let mut v = jeol_fixture_small_2d();
        v[24] = 4;
        v[25] = 4;
        v
    }]
    .into_iter()
    .enumerate()
    {
        let expected = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(format!("lazy-{index}.jdf"));
        fs::write(&path, fixture).unwrap();
        let reader = nmr::raw::OpenOptions::new()
            .allow_experimental_vendor_semantics(true)
            .open(&path)
            .unwrap();
        assert_eq!(reader.descriptor().axes(), expected.descriptor().axes());
        let coordinate = [expected.data().shape()[0] - 1];
        assert_eq!(
            reader.read_trace(&coordinate).unwrap().samples(),
            expected.data().read_trace(&coordinate).unwrap().samples()
        );
        let materialized = reader.into_dataset().unwrap();
        assert_eq!(materialized.data(), expected.data());
        assert_eq!(
            materialized.provenance().sources()[0].digest(),
            expected.provenance().sources()[0].digest()
        );
        assert_eq!(
            materialized.provenance().sources()[0].id(),
            expected.provenance().sources()[0].id()
        );
        assert!(expected.provenance().sources()[0].locator().is_none());
    }
}

#[test]
fn jeol_frequency_domain_is_not_misrepresented_as_raw_acquisition() {
    let mut fixture = jeol_fixture_f64();
    fixture[32] = 1; // unit power one
    fixture[33] = 26; // ppm axis
    put_be_u32(&mut fixture, 240, 2); // processing window is shorter than storage
    let error = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap_err();
    assert_vendor_error(&error, Format::JeolDelta, ReadErrorKind::UnsupportedFeature);
}

#[test]
fn jeol_applies_axis_unit_prefix_and_retains_raw_descriptor() {
    let mut fixture = jeol_fixture_f64();
    fixture[32] = 0x11; // milli, first power
    fixture[33] = 28; // seconds
    fixture[272..280].copy_from_slice(&1.0_f64.to_be_bytes());
    fixture[336..344].copy_from_slice(&4.0_f64.to_be_bytes());
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.unit(), Some(AxisUnit::Second));
    assert_eq!(axis.domain(), Domain::Time);
    assert_eq!(
        axis.coordinates(),
        &AxisCoordinates::Uniform {
            start: 0.001,
            step: 0.001,
        }
    );
    let parameters = dataset.provenance().source_metadata().as_jeol().unwrap();
    assert_eq!(parameters.axis_units()[0].prefix_exponent(), -3);
    assert_eq!(parameters.axis_units()[0].power(), 1);
    assert_eq!(parameters.axis_units()[0].base_code(), 28);
    assert_eq!(parameters.raw_header().len(), 1360);
    assert_eq!(
        parameters.sample_transform().direct_imaginary_multiplier(),
        -1
    );
}

#[test]
fn jeol_does_not_expose_numeric_coordinates_with_an_unknown_unit() {
    let mut fixture = jeol_fixture_f64();
    fixture[32] = 0x11; // unit power one, but unknown base code
    fixture[33] = 255;
    fixture[272..280].copy_from_slice(&1.0_f64.to_be_bytes());
    fixture[336..344].copy_from_slice(&4.0_f64.to_be_bytes());
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.unit(), None);
    assert_eq!(axis.domain(), Domain::Unknown);
    assert_eq!(axis.coordinates(), &AxisCoordinates::Unknown);
}

#[test]
fn jeol_parses_typed_parameter_records_and_projects_acquisition_metadata() {
    let fixture = jeol_fixture_with_parameters();
    let dataset = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.nucleus(), Some("1H"));
    assert_eq!(axis.spectral_width_hz(), Some(4000.0));
    let frequency = axis.frequency_evidence().unwrap();
    assert_eq!(frequency.observe_frequency_mhz(), Some(400.0));
    assert_eq!(frequency.transmitter_offset_hz(), Some(2000.0));
    assert_eq!(
        dataset.descriptor().acquisition().title(),
        Some("Synthetic FID")
    );
    assert_eq!(dataset.descriptor().acquisition().solvent(), Some("D2O"));
    assert_eq!(dataset.descriptor().acquisition().scans(), Some(16));
    assert_eq!(
        dataset.descriptor().acquisition().pulse_program(),
        Some("proton.jxp")
    );
    let parameters = dataset.provenance().source_metadata().as_jeol().unwrap();
    assert!(matches!(
        parameters.get("x_domain").unwrap().value(),
        nmr::formats::jeol::ParameterValue::String(value) if value == "Proton"
    ));
    assert_eq!(parameters.values().len(), 7);
    assert_eq!(
        parameters.get("x_sweep").unwrap().scaled_f64(),
        Some(4000.0)
    );
    assert_eq!(
        parameters
            .get("x_sweep")
            .unwrap()
            .primary_unit()
            .base_code(),
        13
    );
    assert_eq!(parameters.raw_pre_data_records().len(), 464);
}

#[test]
fn jeol_rejects_declared_data_section_truncation() {
    let mut fixture = jeol_fixture_f64();
    put_be_u64(&mut fixture, 1288, 8);
    let error = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap_err();
    assert_vendor_error(&error, Format::JeolDelta, ReadErrorKind::Truncated);
}

#[test]
fn jeol_rejects_a_parameter_table_overlapping_sample_data() {
    let mut fixture = jeol_fixture_with_parameters();
    put_be_u32(&mut fixture, 1212, 1632);
    fixture.resize(2096, 0);
    let error = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap_err();
    assert_vendor_error(&error, Format::JeolDelta, ReadErrorKind::Corrupt);
}

#[test]
fn jeol_rejects_format_dimension_mismatch() {
    let mut fixture = jeol_fixture_f64();
    fixture[12] = 2;
    let error = read_from_parts(DatasetParts::Jeol { jdf: &fixture }).unwrap_err();
    assert_vendor_error(&error, Format::JeolDelta, ReadErrorKind::Corrupt);
}

#[test]
fn jeol_rejects_short_headers() {
    let error = read_from_parts(DatasetParts::Jeol { jdf: b"JEOL.NMR" }).unwrap_err();
    assert_vendor_error(&error, Format::JeolDelta, ReadErrorKind::Truncated);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::Truncated { expected: 1360, .. }
    ));
}

#[test]
fn binary_and_text_axis_lists_agree_after_units_and_roundoff_but_conflicts_fail() {
    for cropped in [false, true] {
        for conflict in [false, true] {
            let mut bytes = jeol_fixture_small_2d();
            bytes[34] = 0x11; // binary milliseconds
            bytes[35] = 28;
            put_be_u32(&mut bytes, 212, if cropped { 1 } else { 0 });
            put_be_u32(&mut bytes, 244, 3);
            let start = bytes.len();
            put_be_u32(&mut bytes, 1224, start as u32);
            put_be_u32(&mut bytes, 1256, 32);
            for value in [
                1.0_f64,
                5.492799999999988,
                if conflict { 900.0 } else { 910.28 },
                5000.0,
            ] {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
            bytes.push(0);
            bytes.extend_from_slice(
                b"tau => y_acq {1[ms], 5.4928[ms], 0.91028[s], 5[s]}, help \"delay\";",
            );
            let result = read_from_parts(DatasetParts::Jeol { jdf: &bytes });
            if conflict {
                assert!(result.is_err());
            } else {
                let raw = result.unwrap();
                let AxisCoordinates::Explicit(values) = raw.descriptor().axes()[0].coordinates()
                else {
                    panic!()
                };
                assert_eq!(values.len(), if cropped { 3 } else { 4 });
                assert_eq!(values[values.len() - 1], 5.0);
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("list.jdf");
                fs::write(&path, &bytes).unwrap();
                let lazy = nmr::raw::OpenOptions::new()
                    .allow_experimental_vendor_semantics(true)
                    .open(path)
                    .unwrap();
                assert_eq!(lazy.descriptor().axes(), raw.descriptor().axes());
            }
        }
    }
}

#[test]
fn malformed_ramp_increments_are_not_hidden_by_suffix_parsing() {
    for increment in ["1[bogus]", "1[ms]junk", "NaN[ms]", "0[ms]", "1[ms] + 2[ms]"] {
        let mut bytes = jeol_fixture_small_2d();
        bytes.extend_from_slice(
            format!("tau => y_acq 1[ms]..4[ms] : {increment}, help \"delay\";").as_bytes(),
        );
        assert!(read_from_parts(DatasetParts::Jeol { jdf: &bytes }).is_err());
    }
}
