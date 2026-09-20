use crate::axis::AxisDomain;
use crate::processed::ComponentBasis;
use crate::processing::contracts::operation::*;
use crate::processing::contracts::polarity::PolarityState;

use super::calibration::rebuild_axis;
use super::{AxisUpdate, PlanState, StateError};

pub(crate) fn transition_projection(
    state: &mut PlanState,
    projection: Projection,
    polarity: PolarityState,
) -> Result<ResolvedOperation, StateError> {
    if !state.axes.iter().any(|axis| axis.axis.role().is_signal()) {
        return Err(StateError::InvalidDatasetState {
            operation: "scalar projection",
            reason: "dataset has no signal axis",
        });
    }
    if projection == Projection::Real && polarity != PolarityState::Ambiguous180 {
        return Err(StateError::InvalidDatasetState {
            operation: "scalar projection",
            reason: "neutral real extraction requires unspecified polarity (Ambiguous180)",
        });
    }
    for (axis_index, axis_state) in state.axes.iter().enumerate() {
        if !axis_state.axis.role().is_signal() {
            continue;
        }
        if axis_state.axis.domain() != AxisDomain::Frequency
            || !matches!(
                axis_state.axis.component_basis(),
                ComponentBasis::Cartesian | ComponentBasis::SharedComplex { .. }
            )
        {
            return Err(StateError::InvalidState {
                axis: axis_index,
                operation: "scalar projection",
                reason: "all signal axes must be Cartesian frequency axes",
            });
        }
        match projection {
            Projection::RealAbsorptive | Projection::RealSigned if !axis_state.phase_applied => {
                return Err(StateError::InvalidState {
                    axis: axis_index,
                    operation: "scalar projection",
                    reason: "all signal axes must be phased",
                });
            }
            Projection::UnphasedReal if !axis_state.phase_attempt_failed => {
                return Err(StateError::InvalidState {
                    axis: axis_index,
                    operation: "scalar projection",
                    reason: "UnphasedReal requires a recorded phase failure",
                });
            }
            _ => {}
        }
    }
    if projection == Projection::RealAbsorptive && !polarity.is_established() {
        return Err(StateError::InvalidDatasetState {
            operation: "scalar projection",
            reason: "RealAbsorptive requires established polarity",
        });
    }
    for axis_state in &mut state.axes {
        if axis_state.axis.role().is_signal() {
            axis_state.axis = rebuild_axis(
                &axis_state.axis,
                AxisUpdate {
                    component_basis: Some(ComponentBasis::Scalar),
                    ..AxisUpdate::default()
                },
            )?;
            axis_state.operation_count = axis_state
                .operation_count
                .checked_add(1)
                .ok_or(StateError::SizeOverflow)?;
        }
    }
    Ok(ResolvedOperation::Projection {
        projection,
        polarity,
    })
}
pub(crate) fn validate_auto_phase_axis(state: &PlanState, axis: usize) -> Result<(), StateError> {
    let selected = state.axes.get(axis).ok_or(StateError::InvalidState {
        axis,
        operation: "automatic phase correction",
        reason: "axis index is out of bounds",
    })?;
    if selected.axis.domain() != AxisDomain::Frequency
        || !matches!(selected.axis.component_basis(), ComponentBasis::Cartesian)
        || !selected.axis.role().is_signal()
    {
        return Err(StateError::InvalidState {
            axis,
            operation: "automatic phase correction",
            reason: "requires a Cartesian frequency-domain signal axis",
        });
    }
    Ok(())
}

pub(crate) fn transition_auto_phase(state: &mut PlanState, axis: usize) -> Result<(), StateError> {
    validate_auto_phase_axis(state, axis)?;
    state.axes[axis].phase_applied = true;
    state.axes[axis].operation_count = state.axes[axis]
        .operation_count
        .checked_add(1)
        .ok_or(StateError::SizeOverflow)?;
    Ok(())
}
