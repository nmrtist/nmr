use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    PhaseCorrection, ProcessingError, ProcessingOperation, ProcessingOptions, ProcessingPlan,
};
use nmr::resource::{LimitExceeded, MemoryLimits, ResourceKind};

fn input() -> nmr::Dataset {
    nmr::Dataset::from_processed(
        ProcessedDataset::new(
            ProcessedDescriptor::new(vec![
                ProcessedAxis::new(
                    AxisRole::Signal,
                    AxisDomain::Frequency,
                    Some(AxisUnit::Hertz),
                    1,
                    AxisCoordinates::Uniform {
                        start: 0.0,
                        step: 1.0,
                    },
                    ComponentBasis::Cartesian,
                )
                .unwrap(),
            ])
            .unwrap(),
            ProcessedData::new(vec![1], vec![2], vec![3.0, 4.0]).unwrap(),
            ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
        )
        .unwrap(),
    )
}

fn phase() -> ProcessingOperation {
    ProcessingOperation::PhaseCorrection {
        axis: 0,
        correction: PhaseCorrection::new(90.0, 0.0, 0.0).unwrap(),
    }
}

#[test]
fn prepared_resource_estimate_covers_metadata_and_reports_structured_limits() {
    let input = input();
    let plan = ProcessingPlan::new(vec![phase(); 129]).unwrap();
    let estimate = plan
        .preflight(&input, ProcessingOptions::new())
        .unwrap()
        .resources();
    assert_eq!(estimate.output_bytes(), 16);
    assert!(estimate.metadata_bytes() > 16);
    assert_eq!(estimate.working_bytes(), estimate.metadata_bytes() + 48);
    let limits = MemoryLimits::new()
        .max_output_bytes(estimate.output_bytes())
        .max_metadata_bytes(estimate.metadata_bytes())
        .max_working_bytes(estimate.working_bytes());
    let output = plan
        .preflight(&input, ProcessingOptions::new().memory(limits))
        .unwrap()
        .execute()
        .unwrap();
    let actual = output.as_dense_processed().unwrap().samples();
    assert!((actual[0] + 4.0).abs() < 1e-12 && (actual[1] - 3.0).abs() < 1e-12);
    for (limits, resource, limit, required) in [
        (
            limits.max_output_bytes(15),
            ResourceKind::OutputBytes,
            15,
            16,
        ),
        (
            limits.max_metadata_bytes(estimate.metadata_bytes() - 1),
            ResourceKind::MetadataBytes,
            estimate.metadata_bytes() - 1,
            estimate.metadata_bytes(),
        ),
        (
            limits.max_working_bytes(estimate.working_bytes() - 1),
            ResourceKind::WorkingBytes,
            estimate.working_bytes() - 1,
            estimate.working_bytes(),
        ),
    ] {
        assert_eq!(
            plan.preflight(&input, ProcessingOptions::new().memory(limits))
                .unwrap_err()
                .into_root_cause(),
            ProcessingError::LimitExceeded(LimitExceeded {
                resource,
                limit,
                required
            })
        );
    }
}

#[test]
fn replay_uses_metadata_sublimit_and_memory_builders_remain_consistent() {
    let input = input();
    let plan = ProcessingPlan::new(vec![phase()]).unwrap();
    let first = plan.apply(&input).unwrap();
    let second = plan.apply(&first).unwrap();
    let history = second
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap();
    let options = ProcessingOptions::new().memory(MemoryLimits::new().max_metadata_bytes(1));
    assert_eq!(options.metadata_bytes(), 1);
    assert!(
        matches!(history.replay(&[&input], options).map_err(ProcessingError::into_root_cause), Err(ProcessingError::LimitExceeded(LimitExceeded { resource: ResourceKind::MetadataBytes, limit: 1, required })) if required > 1)
    );
    let replayed = history.replay(&[&input], ProcessingOptions::new()).unwrap();
    assert_eq!(
        replayed.as_dense_processed().unwrap().samples(),
        second.as_dense_processed().unwrap().samples()
    );
    let defaults = MemoryLimits::default();
    assert_eq!(defaults.output_bytes(), 512 * 1024 * 1024);
    assert_eq!(defaults.metadata_bytes(), defaults.output_bytes());
    assert_eq!(defaults.working_bytes(), defaults.output_bytes());
}

#[test]
fn borrowed_raw_fft_estimate_includes_output_metadata_and_backend() {
    use nmr::processing::FourierTransform;
    use nmr::raw::{DirectSamples, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata};
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        5,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        },
    )
    .unwrap();
    let mut samples = vec![nmr::Complex64::default(); 5];
    samples[0] = nmr::Complex64::new(1.0, 0.0);
    let input = nmr::Dataset::from_raw(
        RawDatasetBuilder::new(vec![axis], RawMetadata::default())
            .unwrap()
            .dense(samples)
            .unwrap(),
    );
    let plan = ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap();
    let estimate = plan
        .preflight(&input, ProcessingOptions::new())
        .unwrap()
        .resources();
    assert_eq!(estimate.output_bytes(), 80);
    assert!(estimate.working_bytes() > estimate.metadata_bytes() + 3 * 80);
    let limits = MemoryLimits::new()
        .max_output_bytes(80)
        .max_metadata_bytes(estimate.metadata_bytes())
        .max_working_bytes(estimate.working_bytes());
    assert!(matches!(
        plan.preflight(
            &input,
            ProcessingOptions::new().memory(limits.max_working_bytes(estimate.working_bytes() - 1))
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(LimitExceeded {
            resource: ResourceKind::WorkingBytes,
            ..
        }))
    ));
    let output = plan
        .preflight(&input, ProcessingOptions::new().memory(limits))
        .unwrap()
        .execute()
        .unwrap();
    for pair in output
        .as_dense_processed()
        .unwrap()
        .samples()
        .chunks_exact(2)
    {
        assert!((pair[0] - 1.0).abs() < 1e-12 && pair[1].abs() < 1e-12);
    }
}
