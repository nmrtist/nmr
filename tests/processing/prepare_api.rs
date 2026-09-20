//! Downstream task acceptance for the recommended prepare / inspect / execute API.

use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processing::{
    DelaySource, DigitalFilterCorrection, FourierTransform, OperationTarget, PhaseCorrection,
    ProcessingError, ProcessingInput, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan,
    ResolvedOperation, Window,
};
use nmr::raw::{
    DirectSamples, GroupDelayState, PendingGroupDelay, RawAxis, RawAxisKind, RawDatasetBuilder,
    RawMetadata,
};
use nmr::{Complex64, Dataset};

fn raw(delay: GroupDelayState) -> Dataset {
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        4,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        },
    )
    .unwrap()
    .with_group_delay(delay)
    .unwrap();
    Dataset::from_raw(
        RawDatasetBuilder::new(vec![axis], RawMetadata::default())
            .unwrap()
            .dense(vec![
                Complex64::new(1.0, 0.0),
                Complex64::default(),
                Complex64::default(),
                Complex64::default(),
            ])
            .unwrap(),
    )
}

#[test]
fn preview_exposes_resolved_zero_fill_delay_and_rules_used_by_every_entry() {
    let input = raw(GroupDelayState::Pending(
        PendingGroupDelay::user_constructed(0.75).unwrap(),
    ));
    let plan = ProcessingPlan::new(vec![
        Op::StandardZeroFill { axis: 0 },
        Op::FourierTransform {
            axis: 0,
            transform: FourierTransform::default(),
        },
        Op::DigitalFilterCorrection {
            axis: 0,
            correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(
                DelaySource::AxisEvidence,
            ),
        },
    ])
    .unwrap();
    let prepared = plan.preflight(&input, ProcessingOptions::new()).unwrap();
    let steps: Vec<_> = prepared.steps().collect();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].target(), OperationTarget::Axis(0));
    assert!(matches!(
        steps[0].resolved(),
        ResolvedOperation::ZeroFill {
            target_points: 8,
            ..
        }
    ));
    assert_eq!(steps[0].input_descriptor().logical_shape(), [4]);
    assert_eq!(steps[0].output_descriptor().logical_shape(), [8]);
    assert!(matches!(
        steps[2].resolved(),
        ResolvedOperation::FrequencyDomainPhaseRampV1 { delay: 0.75, .. }
    ));
    let preview: Vec<_> = steps
        .iter()
        .map(|step| {
            (
                step.requested().clone(),
                step.resolved().clone(),
                step.algorithm_version(),
            )
        })
        .collect();
    drop(steps);
    let output = prepared.execute().unwrap();
    let records = output
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap()
        .records();
    for ((request, resolved, version), record) in preview.iter().zip(records) {
        assert_eq!(record.requested().explicit(), Some(request));
        assert_eq!(record.resolved(), Some(resolved));
        assert_eq!(record.algorithm_version(), Some(*version));
    }
    let raw = input.as_raw().unwrap();
    let low = plan
        .preflight_raw(
            &ProcessingInput::from_dataset(raw).unwrap(),
            ProcessingOptions::new(),
        )
        .unwrap();
    assert_eq!(low.steps().len(), 3);
    assert_eq!(
        low.apply(raw).unwrap().canonical_digests(),
        output.canonical_digests()
    );
    assert_eq!(
        plan.apply_raw(raw).unwrap().canonical_digests(),
        output.canonical_digests()
    );
}

#[test]
fn request_values_and_checked_named_parameters_have_distinct_contracts() {
    let input = raw(GroupDelayState::NotApplicable);
    assert!(Window::exponential(f64::NAN).is_err());
    let plan = ProcessingPlan::new(vec![Op::Window {
        axis: 0,
        window: Window::Exponential { lb_hz: f64::NAN },
    }])
    .unwrap();
    assert_eq!(
        plan.preflight(&input, ProcessingOptions::new())
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::InvalidParameter("exponential line broadening")
    );
    let correction = PhaseCorrection::zero_order_degrees(37.0)
        .unwrap()
        .with_first_order_degrees(-60.0)
        .unwrap()
        .with_pivot_fraction(1.0)
        .unwrap();
    assert_eq!(correction, PhaseCorrection::new(37.0, -60.0, 1.0).unwrap());
    assert!(correction.with_pivot_fraction(2.0).is_err());
}
