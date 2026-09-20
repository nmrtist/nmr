use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::execution_report::{self, ComparisonOptions, ExternalAlgorithmDeclaration, ReportError};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    FourierTransform, PhaseCorrection, PolarityState, ProcessingOperation, ProcessingOptions,
    ProcessingPlan, Projection, Window,
};

fn spectrum(values: &[f64], start: f64, step: f64) -> ProcessedDataset {
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        values.len(),
        AxisCoordinates::Uniform { start, step },
        ComponentBasis::Scalar,
    )
    .unwrap();
    ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![values.len()], vec![1], values.to_vec()).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap()
}
fn processed_fixture() -> ProcessedDataset {
    let parameters = "##TITLE= report fixture\n##$TD= 8\n##$PARMODE= 0\n##$AQ_mod= 3\n##$BYTORDA= 0\n##$DTYPA= 0\n##$GO_block_size= <continuous>\n##$SW_h= 1000\n##$SFO1= 400\n##$GRPDLY= 0\n##END=\n";
    let bytes: Vec<_> = [1_i32, 0, 2, 1, 3, 0, 4, -1]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let raw =
        nmr::formats::bruker::read_parts(nmr::formats::bruker::Parts::new(&bytes, &[parameters]))
            .unwrap();
    let plan = ProcessingPlan::new(vec![
        ProcessingOperation::Window {
            axis: 0,
            window: Window::exponential(1.25).unwrap(),
        },
        ProcessingOperation::StandardZeroFill { axis: 0 },
        ProcessingOperation::FourierTransform {
            axis: 0,
            transform: FourierTransform::default(),
        },
        ProcessingOperation::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(12.5, -23.0, 0.25).unwrap(),
        },
        ProcessingOperation::Projection {
            projection: Projection::Real,
            polarity: PolarityState::Ambiguous180,
        },
    ])
    .unwrap();
    plan.apply_raw(&raw).unwrap()
}

#[test]
fn report_carries_stable_parameters_identity_and_separate_external_claims() {
    let dataset = processed_fixture();
    let declarations = [ExternalAlgorithmDeclaration::new(
        "external tool",
        "2.0",
        "caller says: \"phase\"\\\n温度\u{0001}",
    )
    .unwrap()];
    let mut bytes = Vec::new();
    execution_report::write_json(&dataset, &declarations, &mut bytes, 1_000_000).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.starts_with("{\"schema\":\"nmr.execution-report.v1\","));
    for expected in [
        "\"algorithm_id\":\"centered-fft.v1\"",
        "\"target_points\":8",
        "\"p0_degrees\":12.5",
        "\"p1_degrees\":-23",
        "\"phase_grid\":\"k/N\"",
        "\"lb_hz\":1.25",
        "\"raw_origin_snapshot\":{",
        "\"kind\":\"format-rule\",\"id\":\"bruker.direct.v1\"",
        "\"algorithm_id\":\"source-sample-normalization.v1\"",
        "\"external_algorithm_declarations\":[{\"authority\":\"caller-declaration-unverified\"",
        "\\u0001",
        "\"automatic_json_replay_supported\":false",
        "\"resolved_transitive_dependency_identity\":null",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    let environment = dataset.provenance().history().unwrap().segments()[0].environment();
    let build = environment.build_identifier().unwrap();
    assert_eq!(build.len(), 64);
    assert!(build.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(text.contains(&format!("\"build_sha256\":\"{build}\"")));
    let mut repeated = Vec::new();
    execution_report::write_json(&dataset, &declarations, &mut repeated, bytes.len()).unwrap();
    assert_eq!(bytes, repeated);
    let mut untouched = vec![42];
    assert!(matches!(
        execution_report::write_json(&dataset, &declarations, &mut untouched, bytes.len() - 1),
        Err(ReportError::LimitExceeded { .. })
    ));
    assert_eq!(untouched, [42]);
}

#[test]
fn comparator_exposes_amplitude_bias_and_never_fits_global_scale() {
    let reference = spectrum(&[0.0, 1.0, 2.0, 1.0, 0.0], 0.0, 1.0);
    let actual = spectrum(&[0.0, 0.75, 1.5, 0.75, 0.0], 0.0, 1.0);
    let options = ComparisonOptions::new(1e-12, 1e-12).unwrap();
    let comparison =
        execution_report::compare_positive_scalar_spectra(&reference, &actual, 1..4, options)
            .unwrap();
    assert!(!comparison.samples().within_tolerance());
    assert!((comparison.samples().relative_l2().unwrap() - 0.25).abs() < 1e-15);
    assert_eq!(comparison.peak_area_ratio(), 0.75);
    assert_eq!(comparison.peak_shift(), 0.0);
    assert_eq!(comparison.outside_roi_error_relative_l2(), 0.0);
    assert_eq!(comparison.samples().differences()[2].absolute(), 0.5);
    let shifted = spectrum(&[0.0, 0.0, 1.0, 2.0, 1.0], 0.0, 1.0);
    let shifted =
        execution_report::compare_positive_scalar_spectra(&reference, &shifted, 1..4, options)
            .unwrap();
    assert_eq!(shifted.peak_shift(), 1.0);
    assert!(shifted.outside_roi_error_relative_l2() > 0.0);
}

#[test]
fn comparison_rejects_invalid_inputs_and_budgets_before_output_allocation() {
    let options = ComparisonOptions::new(0.0, 0.0).unwrap();
    assert!(matches!(
        execution_report::compare_samples(&[1.0], &[f64::NAN], options),
        Err(ReportError::NonFinite)
    ));
    assert!(matches!(
        execution_report::compare_samples(&[f64::INFINITY], &[1.0], options),
        Err(ReportError::NonFinite)
    ));
    assert!(matches!(
        execution_report::compare_samples(&[f64::MAX], &[-f64::MAX], options),
        Err(ReportError::NonFinite)
    ));
    assert!(matches!(
        execution_report::compare_samples(&[1.0], &[1.0, 2.0], options),
        Err(ReportError::IncompatibleInputs)
    ));
    assert!(matches!(
        execution_report::compare_samples(&[1.0], &[1.0], options.max_difference_bytes(0)),
        Err(ReportError::LimitExceeded { .. })
    ));
    assert!(ComparisonOptions::new(f64::NAN, 0.0).is_err());
    let zero = execution_report::compare_samples(&[0.0], &[1.0], options).unwrap();
    assert_eq!(zero.relative_l2(), None);
    assert_eq!(zero.differences()[0].relative(), None);
    assert!(!zero.within_tolerance());
    let a = spectrum(&[0.0, 1.0, 0.0], 0.0, 1.0);
    let b = spectrum(&[0.0, 1.0, 0.0], 10.0, 1.0);
    assert!(matches!(
        execution_report::compare_processed(&a, &b, options),
        Err(ReportError::IncompatibleInputs)
    ));
    let signed = spectrum(&[-1.0, 1.0, 0.0], 0.0, 1.0);
    assert!(matches!(
        execution_report::compare_positive_scalar_spectra(&signed, &signed, 0..3, options),
        Err(ReportError::Invalid(_))
    ));
}

#[test]
fn tolerance_comparison_is_independent_of_runtime_replay_identity() {
    let dataset = processed_fixture();
    let history = dataset.provenance().history().unwrap();
    assert!(
        history
            .replay_processed(&dataset, ProcessingOptions::new())
            .is_err()
    );
    assert!(
        execution_report::compare_processed(
            &dataset,
            &dataset,
            ComparisonOptions::new(0.0, 0.0).unwrap()
        )
        .unwrap()
        .within_tolerance()
    );
}

#[test]
fn writer_errors_are_reported() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test writer"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(matches!(
        execution_report::write_json(&processed_fixture(), &[], &mut Broken, 1_000_000),
        Err(ReportError::Io(_))
    ));
}
