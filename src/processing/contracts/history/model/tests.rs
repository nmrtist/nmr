use crate::processed::model::ProcessedDescriptor;
use crate::processed::model::ProcessedOrigin;
use crate::processing::contracts::error::ProcessingError;
use crate::processing::contracts::operation::ProcessingOperation;
use crate::processing::contracts::operation::ProcessingRequest;
use crate::processing::contracts::operation::ResolvedOperation;
use crate::processing::contracts::state::PlanState;
use crate::processing::contracts::state::StateError;
use crate::processing::contracts::state::transition;
use crate::processing::contracts::state::transition_auto_phase;

use super::super::replay_record;
use super::*;
use crate::processing::ProcessingOptions;

#[test]
fn unknown_component_baseline_and_projection_rules_are_rejected() {
    use crate::Complex64;
    use crate::acquisition::{
        LinearComponentTransform, PeriodicLaneModulation, ResolvedComponentTransform,
    };
    use crate::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
    use crate::processed::{ComponentBasis, ProcessedAxis};
    use crate::processing::{PolarityState, PositivePeaksV1, Projection};

    let axis = |role, domain, unit, basis| {
        ProcessedAxis::new(
            role,
            domain,
            Some(unit),
            3,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 1.0,
            },
            basis,
        )
        .unwrap()
    };
    let one = Complex64::new(1.0, 0.0);
    let zero = Complex64::new(0.0, 0.0);
    let transform = ResolvedComponentTransform::user_constructed(
        LinearComponentTransform::try_new(
            2,
            vec![one, zero, zero, one],
            PeriodicLaneModulation::identity(2).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let cases = [
        (
            ProcessedDescriptor::new(vec![
                axis(
                    AxisRole::IndirectAcquisition,
                    AxisDomain::Time,
                    AxisUnit::Second,
                    ComponentBasis::Encoded(transform),
                ),
                axis(
                    AxisRole::Signal,
                    AxisDomain::Frequency,
                    AxisUnit::Hertz,
                    ComponentBasis::Cartesian,
                ),
            ])
            .unwrap(),
            ProcessingOperation::ComponentTransform { axis: 0 },
            "unknown-component-transform",
            "linear-component-transform.v1",
        ),
        (
            ProcessedDescriptor::new(vec![axis(
                AxisRole::Signal,
                AxisDomain::Frequency,
                AxisUnit::Hertz,
                ComponentBasis::Scalar,
            )])
            .unwrap(),
            ProcessingOperation::BaselineCorrection {
                axis: 0,
                profile: PositivePeaksV1.into(),
            },
            "unknown-baseline",
            "asls.v1",
        ),
        (
            ProcessedDescriptor::new(vec![axis(
                AxisRole::Signal,
                AxisDomain::Frequency,
                AxisUnit::Hertz,
                ComponentBasis::Cartesian,
            )])
            .unwrap(),
            ProcessingOperation::Projection {
                projection: Projection::Magnitude,
                polarity: PolarityState::Ambiguous180,
            },
            "unknown-projection",
            "projection.v1",
        ),
    ];
    for (descriptor, request, unknown_version, current_version) in cases {
        let initial = PlanState::from_descriptor(&descriptor);
        let mut expected = initial.clone();
        let resolved = transition(&mut expected, &request)
            .map_err(ProcessingError::from)
            .unwrap();
        let mut record = ProcessingRecord::applied(
            request,
            resolved,
            descriptor,
            expected
                .descriptor()
                .map_err(ProcessingError::from)
                .unwrap(),
            vec![],
        );
        assert_eq!(record.algorithm_version(), Some(current_version));
        let mut replayed = initial.clone();
        assert!(replay_record(&mut replayed, &record).is_ok());
        assert_eq!(replayed, expected);

        let ProcessingRecord::Applied {
            algorithm_version, ..
        } = &mut record
        else {
            unreachable!()
        };
        *algorithm_version = unknown_version;
        let mut rejected = initial.clone();
        assert!(matches!(
            replay_record(&mut rejected, &record),
            Err(StateError::Mapping(
                "processing history input contract disagrees"
            ))
        ));
        assert_eq!(
            rejected, initial,
            "unknown rules must fail before mutating state"
        );
    }
}

#[test]
fn unguarded_automatic_phase_history_is_rejected_before_state_mutation() {
    use crate::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
    use crate::processed::{ComponentBasis, ProcessedAxis};
    use crate::processing::{NormalizedAcmeV1, PhaseCorrection, PhaseFailurePolicy, PolarityState};
    let descriptor = ProcessedDescriptor::new(vec![
        ProcessedAxis::new(
            AxisRole::Signal,
            AxisDomain::Frequency,
            Some(AxisUnit::Hertz),
            8,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 1.0,
            },
            ComponentBasis::Cartesian,
        )
        .unwrap(),
    ])
    .unwrap();
    let initial = PlanState::from_descriptor(&descriptor);
    let mut after = initial.clone();
    transition_auto_phase(&mut after, 0)
        .map_err(ProcessingError::from)
        .unwrap();
    let mut record = ProcessingRecord::applied(
        ProcessingRequest::AutoPhase {
            axis: 0,
            profile: NormalizedAcmeV1::new(),
            polarity: PolarityState::UserAssertedPositive,
            failure_policy: PhaseFailurePolicy::Fail,
        },
        ResolvedOperation::PhaseCorrection(PhaseCorrection::new(0.0, 0.0, 1.0).unwrap()),
        descriptor,
        after.descriptor().map_err(ProcessingError::from).unwrap(),
        vec![],
    );
    assert_eq!(
        record.algorithm_version(),
        Some("normalized-acme.lorentzian-guard.v1")
    );
    assert!(replay_record(&mut initial.clone(), &record).is_ok());
    let ProcessingRecord::Applied {
        algorithm_version, ..
    } = &mut record
    else {
        unreachable!()
    };
    *algorithm_version = "phase-correction.v1";
    let mut rejected = initial.clone();
    assert!(replay_record(&mut rejected, &record).is_err());
    assert_eq!(rejected, initial);
}

#[test]
fn unsupported_history_inputs_and_axis_slots_are_rejected_before_execution() {
    use crate::axis::{AxisCoordinates, AxisDomain, AxisRole};
    use crate::processed::{
        ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedProvenance,
    };
    let descriptor = ProcessedDescriptor::new(vec![
        ProcessedAxis::new(
            AxisRole::Signal,
            AxisDomain::Unknown,
            None,
            1,
            AxisCoordinates::Unknown,
            ComponentBasis::Scalar,
        )
        .unwrap(),
    ])
    .unwrap();
    let dataset = ProcessedDataset::new(
        descriptor.clone(),
        ProcessedData::new(vec![1], vec![1], vec![1.0]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let binding = HistoryInput::Processed {
        digests: dataset.canonical_digests(),
        sources: vec![],
        read_record: None,
    };
    let mut history = ProcessingHistory::new(descriptor, binding.clone(), vec![]);
    history.inputs.push(binding);
    let input = crate::Dataset::from_processed(dataset);
    assert_eq!(
        history
            .replay(&[&input, &input], ProcessingOptions::default())
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::MissingCapability {
            capability: "single external replay input",
            axis: None
        }
    );
    history.inputs.pop();
    history.axis_lineage[0] =
        crate::provenance::InputAxisRef::new(crate::provenance::InputSlot::new(1), 0);
    assert_eq!(
        history
            .replay(&[&input], ProcessingOptions::default())
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::Mapping("history input-axis references are invalid")
    );
}
