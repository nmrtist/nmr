use crate::axis::AxisDomain;
use crate::axis::AxisRole;
use crate::processed::ComponentBasis;
use crate::processing::contracts::operation::*;

use super::super::calibration::{
    checked_floor_to_usize, delay_matches, positive_uniform_time, rebuild_axis,
};
use super::super::{AxisState, AxisUpdate, ProcessingDelayState, StateError};
use super::AxisTransition;

pub(super) fn acknowledge_zero_delay(
    axis_state: &mut AxisState,
    axis_index: usize,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    Ok(AxisTransition::Applied({
        let evidence = match &axis_state.group_delay {
            ProcessingDelayState::Pending(value) if value.delay_points() == 0.0 => {
                value.evidence().clone()
            }
            ProcessingDelayState::Pending(_) => return Err(StateError::DelayEvidenceMismatch),
            ProcessingDelayState::Corrected { .. } => {
                return Err(invalid("group delay has already been corrected"));
            }
            _ => {
                return Err(StateError::MissingCapability {
                    capability: "zero group-delay evidence",
                    axis: Some(axis_index),
                });
            }
        };
        axis_state.group_delay = ProcessingDelayState::Corrected {
            delay: 0.0,
            evidence: Some(evidence.clone()),
        };
        ResolvedOperation::AcknowledgeZeroDelayV1 { evidence }
    }))
}

pub(super) fn frequency_delay(
    axis_state: &mut AxisState,
    axis_index: usize,
    source: &DelaySource,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        if axis.domain() != AxisDomain::Frequency
            || !matches!(axis.component_basis(), ComponentBasis::Cartesian)
            || !axis.role().is_signal()
        {
            return Err(invalid("requires a Cartesian frequency-domain signal axis"));
        }
        let Some(sign) = axis_state.latest_fft else {
            return Err(invalid(
                "requires the canonical centered-bin layout of a library FFT",
            ));
        };
        let evidence = match &axis_state.group_delay {
            ProcessingDelayState::Pending(value) => Some(value.delay_points()),
            ProcessingDelayState::Unknown | ProcessingDelayState::NotApplicable => None,
            ProcessingDelayState::Corrected { .. } => {
                return Err(invalid("group delay has already been corrected"));
            }
        };
        let delay = match source {
            DelaySource::AxisEvidence => evidence.ok_or(StateError::MissingCapability {
                capability: "group-delay evidence",
                axis: Some(axis_index),
            })?,
            DelaySource::Explicit(value) => {
                if !value.is_finite() || *value < 0.0 {
                    return Err(StateError::InvalidParameter("explicit delay"));
                }
                if evidence.is_some_and(|known| !delay_matches(known, *value)) {
                    return Err(StateError::DelayEvidenceMismatch);
                }
                *value
            }
        };
        if delay == 0.0 {
            return Err(invalid("operation is an identity on this axis"));
        }
        let evidence = match &axis_state.group_delay {
            ProcessingDelayState::Pending(value) => Some(value.evidence().clone()),
            _ => None,
        };
        axis_state.group_delay = ProcessingDelayState::Corrected { delay, evidence };
        ResolvedOperation::FrequencyDomainPhaseRampV1 { delay, sign }
    }))
}

pub(super) fn time_delay(
    axis_state: &mut AxisState,
    axis_index: usize,
    rank: usize,
    source: &DelaySource,
    policy: &TimeDomainResidualPolicy,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        if axis_index + 1 != rank
            || axis.role() != AxisRole::DirectAcquisition
            || axis.domain() != AxisDomain::Time
            || !matches!(axis.component_basis(), ComponentBasis::Cartesian)
        {
            return Err(invalid(
                "requires the direct complex acquisition axis in the time domain",
            ));
        }
        if axis_state.operation_count != 0 {
            return Err(invalid("must be the first operation on the direct axis"));
        }
        positive_uniform_time(axis, &invalid)?;
        let pending = match &axis_state.group_delay {
            ProcessingDelayState::Pending(value) => Some(value),
            ProcessingDelayState::Unknown | ProcessingDelayState::NotApplicable => None,
            ProcessingDelayState::Corrected { .. } => {
                return Err(invalid("group delay has already been corrected"));
            }
        };
        let delay =
            match source {
                DelaySource::AxisEvidence => pending.map(|value| value.delay_points()).ok_or(
                    StateError::MissingCapability {
                        capability: "group-delay evidence",
                        axis: Some(axis_index),
                    },
                )?,
                DelaySource::Explicit(value) => {
                    if !value.is_finite() || *value < 0.0 {
                        return Err(StateError::InvalidParameter("explicit delay"));
                    }
                    if pending.is_some_and(|known| !delay_matches(known.delay_points(), *value)) {
                        return Err(StateError::DelayEvidenceMismatch);
                    }
                    *value
                }
            };
        let applied_delay = match policy {
            TimeDomainResidualPolicy::CorrectFully => delay,
            TimeDomainResidualPolicy::IntegerOnlyRetainResidual => delay.floor(),
        };
        let skip = checked_floor_to_usize(applied_delay + 2.0)?;
        if axis.points() <= skip {
            return Err(invalid("corrected trace would contain no points"));
        }
        let fold = skip.saturating_sub(6);
        let residual = delay - applied_delay;
        let corrected = ProcessingDelayState::Corrected {
            delay,
            evidence: pending.map(|value| value.evidence().clone()),
        };
        let remaining = match policy {
            TimeDomainResidualPolicy::CorrectFully => corrected,
            TimeDomainResidualPolicy::IntegerOnlyRetainResidual => {
                if residual != 0.0 {
                    ProcessingDelayState::Pending(
                        crate::acquisition::PendingGroupDelay::resolved(
                            residual,
                            pending
                                .ok_or(StateError::MissingCapability {
                                    capability: "group-delay evidence",
                                    axis: Some(axis_index),
                                })?
                                .evidence()
                                .clone(),
                        )
                        .map_err(|_| StateError::InvalidParameter("group delay"))?,
                    )
                } else {
                    corrected
                }
            }
        };
        axis_state.axis = rebuild_axis(
            axis,
            AxisUpdate {
                points: Some(axis.points() - skip),
                ..AxisUpdate::default()
            },
        )?;
        axis_state.group_delay = remaining;
        ResolvedOperation::TimeDomainShiftFoldV1 {
            applied_delay,
            skip,
            fold,
            residual,
        }
    }))
}
