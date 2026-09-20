use super::support::*;
use crate::Complex64;
use crate::acquisition::{DirectSamples, RawAxisKind};
use crate::axis::AxisRole;
use crate::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use crate::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use crate::processing::contracts::polarity::PolarityState;
use crate::processing::contracts::{error::*, operation::*, options::*, profile::*};
use crate::processing::engine::execution::*;
use crate::processing::prepare::plan::*;
use crate::processing::prepare::resources::*;
use crate::raw::{RawAxis, RawDatasetBuilder, RawMetadata};
use crate::raw::{RawDataset, SamplingSchedule, SourceKind};
use crate::resource::WorkLedger;

#[test]
fn auto_phase_rejects_all_budgets_before_history_or_representative_allocation() {
    let aggregate = crate::Dataset::from_processed(processed_point())
        .attach_read_context(
            crate::Format::Processed(crate::processed::Format::BrukerTopSpin),
            crate::DatasetIdentity::new(Some("subject".repeat(100)), None, None),
            "original-import".into(),
            vec![crate::ReadWarning::ExperimentalVendorSemantics {
                format: crate::Format::Processed(crate::processed::Format::BrukerTopSpin),
                details: Vec::new(),
            }],
        )
        .unwrap();
    let profile = NormalizedAcmeV1::new();
    let prepared = profile
        .preflight(
            &aggregate,
            crate::processing::methods::auto_phase::AutoPhaseRequest {
                axis: crate::AxisIndex::new(0),
                polarity: PolarityState::Ambiguous180,
                failure_policy: PhaseFailurePolicy::ContinueUnphasedReal,
            },
            ProcessingOptions::new(),
        )
        .unwrap();
    let estimate = prepared.resources();
    let maximum_work = prepared.estimated_work();
    let seeds = HISTORY_SEEDS.with(|count| count.get());
    let selections = REPRESENTATIVE_SELECTIONS.with(|count| count.get());
    for (options, resource) in [
        (
            ProcessingOptions::new().max_metadata_bytes(0),
            crate::resource::ResourceKind::MetadataBytes,
        ),
        (
            ProcessingOptions::new().max_output_bytes(estimate.output_bytes() - 1),
            crate::resource::ResourceKind::OutputBytes,
        ),
        (
            ProcessingOptions::new().max_working_bytes(estimate.working_bytes() - 1),
            crate::resource::ResourceKind::WorkingBytes,
        ),
    ] {
        let mut work = WorkLedger::processing_default();
        let error = profile
            .apply_with_context(
                &aggregate,
                crate::processing::methods::auto_phase::AutoPhaseRequest {
                    axis: crate::AxisIndex::new(0),
                    polarity: PolarityState::Ambiguous180,
                    failure_policy: PhaseFailurePolicy::ContinueUnphasedReal,
                },
                options,
                &mut crate::ExecutionContext::new(&mut work),
            )
            .unwrap_err();
        assert!(
            matches!(error.root_cause(), ProcessingError::LimitExceeded(value) if value.resource == resource)
        );
        assert_eq!(work.used(), 0);
        assert_eq!(HISTORY_SEEDS.with(|count| count.get()), seeds);
        assert_eq!(
            REPRESENTATIVE_SELECTIONS.with(|count| count.get()),
            selections
        );
    }
    let mut short_work = WorkLedger::new(maximum_work - 1);
    assert_eq!(
        prepared
            .execute_with_context(&mut crate::ExecutionContext::new(&mut short_work))
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::WorkLimit
    );
    assert_eq!(short_work.used(), 0);
    assert_eq!(HISTORY_SEEDS.with(|count| count.get()), seeds);
    assert_eq!(
        REPRESENTATIVE_SELECTIONS.with(|count| count.get()),
        selections
    );

    let limits = crate::resource::MemoryLimits::new()
        .max_output_bytes(estimate.output_bytes())
        .max_metadata_bytes(estimate.metadata_bytes())
        .max_working_bytes(estimate.working_bytes());
    let mut work = WorkLedger::new(maximum_work);
    let result = profile
        .apply_with_context(
            &aggregate,
            crate::processing::methods::auto_phase::AutoPhaseRequest {
                axis: crate::AxisIndex::new(0),
                polarity: PolarityState::Ambiguous180,
                failure_policy: PhaseFailurePolicy::ContinueUnphasedReal,
            },
            ProcessingOptions::new().memory(limits),
            &mut crate::ExecutionContext::new(&mut work),
        )
        .unwrap();
    assert_eq!(result.metadata(), aggregate.metadata());
    assert_eq!(
        result
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap()
            .records()
            .len(),
        2
    );
    assert_eq!(
        REPRESENTATIVE_SELECTIONS.with(|count| count.get()),
        selections + 1
    );
}

#[test]
fn retained_sources_history_and_context_are_checked_before_seed_cloning() {
    let base = processed_point();
    let source = crate::provenance::SourceFile::from_consumed_bytes(
        SourceKind::Parameters,
        &"p".repeat(8192),
        std::path::Path::new("synthetic"),
        b"synthetic",
    );
    let input = ProcessedDataset::new(
        base.descriptor().clone(),
        base.data().clone(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![source]).unwrap(),
    )
    .unwrap();
    let plan = ProcessingPlan::new(vec![phase()]).unwrap();
    let budget = prepared_retained_bytes(1, 1, true).unwrap()
        + state_storage_bytes(1, 0).unwrap()
        + crate::processing::prepare::memory::processed_apply(&input, 1).unwrap()
        - 1;
    let before = HISTORY_SEEDS.with(|count| count.get());
    assert!(matches!(
        plan.apply_processed_with_options(
            &input,
            ProcessingOptions::new().max_working_bytes(budget)
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(HISTORY_SEEDS.with(|count| count.get()), before);
    let first = ProcessingPlan::new(vec![phase(); 128])
        .unwrap()
        .apply_processed(&input)
        .unwrap();
    let required = prepared_retained_bytes(1, 1, true).unwrap()
        + state_storage_bytes(1, 0).unwrap()
        + crate::processing::prepare::memory::processed_apply(&first, 1).unwrap();
    let before = HISTORY_SEEDS.with(|count| count.get());
    assert!(matches!(
        plan.apply_processed_with_options(
            &first,
            ProcessingOptions::new().max_working_bytes(required - 1)
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(HISTORY_SEEDS.with(|count| count.get()), before);
    let result = plan.apply_processed(&first).unwrap();
    assert_eq!(result.provenance().history().unwrap().records().len(), 129);
    assert!((result.data().samples()[0] + 2.0).abs() < 1e-12);
    assert!((result.data().samples()[1] - 1.0).abs() < 1e-12);

    let aggregate = crate::Dataset::from_processed(processed_point())
        .attach_read_context(
            crate::Format::Processed(crate::processed::Format::BrukerTopSpin),
            crate::DatasetIdentity::new(Some("context".repeat(8192)), None, None),
            "synthetic".into(),
            Vec::new(),
        )
        .unwrap();
    let context = crate::processing::prepare::memory::aggregate(aggregate.metadata()).unwrap();
    let before = HISTORY_SEEDS.with(|count| count.get());
    assert!(matches!(
        plan.preflight(
            &aggregate,
            ProcessingOptions::new().max_working_bytes(context - 1)
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(HISTORY_SEEDS.with(|count| count.get()), before);
    let result = plan.apply(&aggregate).unwrap();
    assert_eq!(result.metadata(), aggregate.metadata());
}

#[test]
fn prepared_vectors_are_checked_before_reserve_and_added_to_sample_peak() {
    let input = crate::Dataset::from_processed(processed_point());
    let plan = ProcessingPlan::new(vec![phase(); 256]).unwrap();
    let retained = prepared_retained_bytes(1, 256, true).unwrap();
    let before = PREPARED_RESERVES.with(|count| count.get());
    assert!(matches!(
        plan.preflight(
            &input,
            ProcessingOptions::new().max_working_bytes(retained - 1)
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(PREPARED_RESERVES.with(|count| count.get()), before);
    // Current/output/gather are each a single complex point.
    let peak = retained
        + state_storage_bytes(1, 0).unwrap()
        + crate::processing::prepare::memory::processed_apply(input.as_processed().unwrap(), 256)
            .unwrap()
        + 3 * std::mem::size_of::<Complex64>();
    assert!(matches!(
        plan.preflight(&input, ProcessingOptions::new().max_working_bytes(peak - 1))
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let result = plan
        .preflight(&input, ProcessingOptions::new().max_working_bytes(peak))
        .unwrap()
        .execute()
        .unwrap();
    for (&actual, expected) in result
        .as_dense_processed()
        .unwrap()
        .samples()
        .iter()
        .zip([1.0, 2.0])
    {
        assert!((actual - expected).abs() < 1e-12);
    }
    assert!(matches!(
        prepared_retained_bytes(2, usize::MAX, true).map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::SizeOverflow)
    ));
}

#[test]
fn complete_sampling_map_is_budgeted_before_state_construction() {
    use crate::processing::contracts::state::STATE_CONSTRUCTIONS;
    use crate::raw::SamplingCoordinate;
    let points = 4096;
    let parameter = RawAxis::new(
        RawAxisKind::Parameter,
        AxisDomain::Parameter,
        None,
        points,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0,
        },
    )
    .unwrap();
    let direct = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        1,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0,
        },
    )
    .unwrap();
    let base = RawDatasetBuilder::new(vec![parameter, direct], RawMetadata::default())
        .unwrap()
        .dense(vec![Complex64::new(1.0, 0.0); points])
        .unwrap();
    let schedule = SamplingSchedule::new(
        vec![points],
        (0..points)
            .map(|point| SamplingCoordinate::new(vec![point]))
            .collect(),
    )
    .unwrap();
    let raw = RawDataset::from_reader(
        base.descriptor().clone(),
        base.data().clone(),
        base.provenance().clone(),
        Some(schedule),
    )
    .unwrap();
    let plan = ProcessingPlan::new(vec![ProcessingOperation::Window {
        axis: 1,
        window: Window::SineBell {
            offset: 0.5,
            end: 0.5,
            power: 1.0,
            first_point_scale: 0.5,
        },
    }])
    .unwrap();
    let budget = prepared_retained_bytes(2, 1, true).unwrap()
        + reserved_raw_axis_bytes(&raw, plan.operations()).unwrap()
        + state_storage_bytes(2, points).unwrap()
        - 1;
    let input = crate::Dataset::from_raw(raw);
    let before = STATE_CONSTRUCTIONS.with(|count| count.get());
    assert!(matches!(
        plan.preflight(&input, ProcessingOptions::new().max_working_bytes(budget))
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(STATE_CONSTRUCTIONS.with(|count| count.get()), before);
    let output = plan.apply(&input).unwrap();
    assert_eq!(
        output.as_dense_processed().unwrap().samples().len(),
        2 * points
    );
    for pair in output
        .as_dense_processed()
        .unwrap()
        .samples()
        .chunks_exact(2)
    {
        assert_eq!(pair, &[0.5, 0.0]);
    }
    assert!(matches!(
        state_storage_bytes(2, usize::MAX).map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::SizeOverflow)
    ));
}

#[test]
fn prepared_raw_execution_rechecks_larger_current_display_allocations() {
    let make = |label: String| {
        let axis = RawAxis::new(
            RawAxisKind::Direct(DirectSamples::Complex),
            AxisDomain::Time,
            Some(AxisUnit::Second),
            2,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.5,
            },
        )
        .unwrap()
        .with_label(Some(label));
        RawDatasetBuilder::new(vec![axis], RawMetadata::default())
            .unwrap()
            .dense(vec![Complex64::new(1.0, 0.0), Complex64::default()])
            .unwrap()
    };
    let original = make("short".into());
    let renamed = make("x".repeat(8192));
    assert_eq!(original.canonical_digests(), renamed.canonical_digests());
    let input = ProcessingInput::from_dataset(&original).unwrap();
    let plan = ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap();
    let resolved = plan
        .preflight_raw(&input, ProcessingOptions::new())
        .unwrap();
    let step = &resolved.steps[0];
    let budget = resolved.options.prepared_bytes
        + resolved.options.axis_bytes
        + resolved.options.state_bytes
        + crate::processing::prepare::memory::raw_apply(&original, 1).unwrap()
        + numerical_working_bytes(&step.before, &step.after, step.axis, &step.resolved).unwrap();
    let prepared = plan
        .preflight_raw(&input, ProcessingOptions::new().max_working_bytes(budget))
        .unwrap();
    let before = crate::processed::model::AXIS_ALLOCATIONS.with(|count| count.get());
    assert!(matches!(
        prepared
            .apply(&renamed)
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(
        crate::processed::model::AXIS_ALLOCATIONS.with(|count| count.get()),
        before
    );
    assert_eq!(
        prepared.apply(&original).unwrap().data().samples(),
        &[1.0, 0.0, 1.0, 0.0]
    );
}

#[test]
fn magnitude_gather_budget_is_added_to_prepared_vectors() {
    let input = processed_point();
    let plan = ProcessingPlan::new(vec![ProcessingOperation::Projection {
        projection: Projection::Magnitude,
        polarity: PolarityState::Ambiguous180,
    }])
    .unwrap();
    let peak = prepared_retained_bytes(1, 1, true).unwrap()
        + reserved_processed_axis_bytes(&input, plan.operations()).unwrap()
        + state_storage_bytes(1, 0).unwrap()
        + crate::processing::prepare::memory::processed_apply(&input, 1).unwrap()
        + 5 * std::mem::size_of::<f64>();
    assert!(matches!(
        plan.apply_processed_with_options(
            &input,
            ProcessingOptions::new().max_working_bytes(peak - 1)
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let output = plan
        .apply_processed_with_options(&input, ProcessingOptions::new().max_working_bytes(peak))
        .unwrap();
    assert!((output.data().samples()[0] - 5.0_f64.sqrt()).abs() < 1e-12);
}

#[test]
fn component_gather_budget_is_added_to_prepared_vectors() {
    use crate::acquisition::{
        LinearComponentTransform, PeriodicLaneModulation, ResolvedComponentTransform,
    };
    let transform = ResolvedComponentTransform::user_constructed(
        LinearComponentTransform::try_new(
            2,
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::default(),
                Complex64::default(),
                Complex64::new(1.0, 0.0),
            ],
            PeriodicLaneModulation::identity(2).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let axes = [
        (
            AxisRole::IndirectAcquisition,
            ComponentBasis::Encoded(transform),
        ),
        (AxisRole::DirectAcquisition, ComponentBasis::Cartesian),
    ]
    .into_iter()
    .map(|(role, basis)| {
        ProcessedAxis::new(
            role,
            AxisDomain::Time,
            Some(AxisUnit::Second),
            1,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 1.0,
            },
            basis,
        )
        .unwrap()
    })
    .collect();
    let input = ProcessedDataset::new(
        ProcessedDescriptor::new(axes).unwrap(),
        ProcessedData::new(vec![1, 1], vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let plan =
        ProcessingPlan::new(vec![ProcessingOperation::ComponentTransform { axis: 0 }]).unwrap();
    let peak = prepared_retained_bytes(2, 1, true).unwrap()
        + reserved_processed_axis_bytes(&input, plan.operations()).unwrap()
        + state_storage_bytes(2, 0).unwrap()
        + crate::processing::prepare::memory::processed_apply(&input, 1).unwrap()
        + 6 * std::mem::size_of::<Complex64>();
    assert!(matches!(
        plan.apply_processed_with_options(
            &input,
            ProcessingOptions::new().max_working_bytes(peak - 1)
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let output = plan
        .apply_processed_with_options(&input, ProcessingOptions::new().max_working_bytes(peak))
        .unwrap();
    assert_eq!(output.data().samples(), &[1.0, 2.0, 3.0, 4.0]);
}
