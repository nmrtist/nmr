use crate::processed::ComponentBasis;
use crate::processing::contracts::operation::*;

use super::{PlanState, StateError, transition_projection};

mod components;
mod delay;
mod spectral;
mod time;

enum AxisTransition {
    Identity(ResolvedOperation),
    Applied(ResolvedOperation),
}

/// Pending lane decoding can depend on acquisition coordinates that the
/// processed descriptor alone cannot recover after a permutation or transform.
fn validate_pending_modulation_coordinates(
    state: &PlanState,
    operation: &ProcessingOperation,
    target: Option<usize>,
) -> Result<(), StateError> {
    for (encoded_axis, pending) in state.axes.iter().enumerate() {
        let ComponentBasis::Encoded(transform) = pending.axis.component_basis() else {
            continue;
        };
        let modulation = transform.transform().modulation();
        let referenced = match modulation.domain() {
            crate::acquisition::ModulationIndexDomain::AbsoluteGridCoordinate(axis) => axis.index(),
            crate::acquisition::ModulationIndexDomain::ObservationOrdinal => encoded_axis,
        };
        let referenced_axis = state.axes.get(referenced).ok_or(StateError::InvalidState {
            axis: encoded_axis,
            operation: operation.name(),
            reason: "component modulation axis is outside the descriptor",
        })?;
        if let crate::acquisition::ModulationIndexDomain::AbsoluteGridCoordinate(_) =
            modulation.domain()
        {
            state.absolute_origin[referenced]
                .checked_add(referenced_axis.axis.points() - 1)
                .and_then(|last| i64::try_from(last).ok())
                .ok_or(StateError::SizeOverflow)?;
        }
        if modulation.period() == 1 || Some(referenced) != target {
            continue;
        }
        let changes_coordinates = match operation {
            ProcessingOperation::ReverseAxis { .. } => referenced_axis.axis.points() > 1,
            ProcessingOperation::FourierTransform { .. }
            | ProcessingOperation::DigitalFilterCorrection {
                correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 { .. },
                ..
            } => true,
            _ => false,
        };
        if changes_coordinates {
            return Err(StateError::InvalidState {
                axis: referenced,
                operation: operation.name(),
                reason: "decode dependent component modulation before changing acquisition coordinates",
            });
        }
    }
    Ok(())
}
pub(crate) fn transition(
    state: &mut PlanState,
    operation: &ProcessingOperation,
) -> Result<ResolvedOperation, StateError> {
    validate_pending_modulation_coordinates(state, operation, operation.axis())?;
    if let ProcessingOperation::Projection {
        projection,
        polarity,
        ..
    } = operation
    {
        return transition_projection(state, *projection, *polarity);
    }
    if let ProcessingOperation::Spectrum { axis, operation } = operation {
        return crate::processing::contracts::spectrum::transition(state, *axis, operation);
    }
    let axis_index = operation
        .axis()
        .expect("non-projection operation has an axis");
    let rank = state.axes.len();
    let observation_ordinals = state.observation_ordinals.clone();
    let direct_cartesian = state
        .axes
        .last()
        .is_some_and(|state| matches!(state.axis.component_basis(), ComponentBasis::Cartesian));
    let axis_state = state
        .axes
        .get_mut(axis_index)
        .ok_or(StateError::InvalidState {
            axis: axis_index,
            operation: operation.name(),
            reason: "axis index is out of bounds",
        })?;
    let invalid = |reason| StateError::InvalidState {
        axis: axis_index,
        operation: operation.name(),
        reason,
    };
    let outcome = match operation {
        ProcessingOperation::DigitalFilterCorrection {
            correction: DigitalFilterCorrection::AcknowledgeZeroDelayV1,
            ..
        } => delay::acknowledge_zero_delay(axis_state, axis_index, &invalid)?,
        ProcessingOperation::Window { window, .. } => {
            time::window(axis_state, axis_index, window, &invalid)?
        }
        ProcessingOperation::ZeroFill { zero_fill, .. } => {
            time::zero_fill(axis_state, zero_fill, &invalid)?
        }
        ProcessingOperation::StandardZeroFill { .. } => {
            time::standard_zero_fill(axis_state, &invalid)?
        }
        ProcessingOperation::FourierTransform { transform, .. } => {
            time::fourier_transform(axis_state, transform, &invalid)?
        }
        ProcessingOperation::DigitalFilterCorrection {
            correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(source),
            ..
        } => delay::frequency_delay(axis_state, axis_index, source, &invalid)?,
        ProcessingOperation::DigitalFilterCorrection {
            correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 { source, policy },
            ..
        } => delay::time_delay(axis_state, axis_index, rank, source, policy, &invalid)?,
        ProcessingOperation::PhaseCorrection { correction, .. } => {
            spectral::phase(axis_state, correction, &invalid)?
        }
        ProcessingOperation::BaselineCorrection { profile, .. } => {
            spectral::baseline(axis_state, profile, &invalid)?
        }
        ProcessingOperation::ComponentTransform { .. } => components::component_transform(
            axis_state,
            axis_index,
            rank,
            direct_cartesian,
            observation_ordinals,
            &state.absolute_origin,
            &invalid,
        )?,
        ProcessingOperation::ResolveFrequencyFrame { frame, .. } => {
            spectral::frequency_frame(axis_state, axis_index, frame, &invalid)?
        }
        ProcessingOperation::ReverseAxis { .. } => spectral::reverse(axis_state, &invalid)?,
        ProcessingOperation::Projection { .. } | ProcessingOperation::Spectrum { .. } => {
            unreachable!("projection is handled before borrowing one axis")
        }
    };
    let resolved = match outcome {
        AxisTransition::Identity(resolved) => return Ok(resolved),
        AxisTransition::Applied(resolved) => resolved,
    };
    axis_state.operation_count = axis_state
        .operation_count
        .checked_add(1)
        .ok_or(StateError::SizeOverflow)?;
    Ok(resolved)
}
