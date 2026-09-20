use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisUnit;
use crate::processed::ComponentBasis;
use crate::processing::contracts::operation::*;

use super::super::calibration::{frequency_frame_axis, rebuild_axis};
use super::super::{AxisState, AxisUpdate, StateError};
use super::AxisTransition;

pub(super) fn phase(
    axis_state: &mut AxisState,
    correction: &PhaseCorrection,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        if axis.domain() != AxisDomain::Frequency
            || !matches!(
                axis.component_basis(),
                ComponentBasis::Cartesian | ComponentBasis::SharedComplex { .. }
            )
            || !axis.role().is_signal()
        {
            return Err(invalid("requires a Cartesian frequency-domain signal axis"));
        }
        axis_state.phase_applied = true;
        ResolvedOperation::PhaseCorrection(*correction)
    }))
}

pub(super) fn baseline(
    axis_state: &mut AxisState,
    profile: &BaselineProfile,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        if axis.domain() != AxisDomain::Frequency
            || axis.unit() != Some(AxisUnit::Hertz)
            || !axis.role().is_signal()
            || !matches!(axis.component_basis(), ComponentBasis::Scalar)
            || axis.component_count() != 1
        {
            return Err(invalid(
                "requires a scalar frequency-domain signal on an ascending Hertz axis",
            ));
        }
        if axis.points() < 3 || axis.direction() != crate::axis::AxisDirection::Ascending {
            return Err(invalid(
                "requires at least three strictly increasing coordinates",
            ));
        }
        ResolvedOperation::BaselineCorrection(*profile)
    }))
}

pub(super) fn frequency_frame(
    axis_state: &mut AxisState,
    axis_index: usize,
    frame: &FrequencyFrame,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        if axis.domain() != AxisDomain::Frequency
            || axis.unit() != Some(AxisUnit::Hertz)
            || !axis.role().is_signal()
        {
            return Err(invalid("requires a calibrated Hertz signal axis"));
        }
        if *frame == FrequencyFrame::Hertz {
            if matches!(axis.coordinates(), AxisCoordinates::Unknown) {
                return Err(invalid("axis has no frequency coordinates"));
            }
            return Ok(AxisTransition::Identity(
                ResolvedOperation::ResolveFrequencyFrame {
                    frame: frame.clone(),
                    reference: None,
                },
            ));
        }
        let reference = match frame {
            FrequencyFrame::Hertz => None,
            FrequencyFrame::Ppm(ReferenceSource::AxisEvidence) => {
                Some(axis_state.chemical_shift_reference.clone().ok_or(
                    StateError::MissingCapability {
                        capability: "chemical-shift reference",
                        axis: Some(axis_index),
                    },
                )?)
            }
            FrequencyFrame::Ppm(ReferenceSource::Explicit(value)) => Some(value.clone()),
        };
        axis_state.axis = frequency_frame_axis(axis, frame, reference.as_ref(), &invalid)?;
        axis_state.chemical_shift_reference = reference.clone();
        ResolvedOperation::ResolveFrequencyFrame {
            frame: frame.clone(),
            reference,
        }
    }))
}

pub(super) fn reverse(
    axis_state: &mut AxisState,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        let coordinates = match axis.coordinates() {
            AxisCoordinates::Uniform { .. } | AxisCoordinates::Explicit(_)
                if axis.points() == 1 =>
            {
                axis.coordinates().clone()
            }
            AxisCoordinates::Uniform { start, step } => AxisCoordinates::Uniform {
                start: step.mul_add((axis.points() - 1) as f64, *start),
                step: -*step,
            },
            AxisCoordinates::Explicit(values) => {
                AxisCoordinates::Explicit(values.iter().rev().copied().collect())
            }
            AxisCoordinates::Unknown => {
                return Err(invalid("requires established coordinates"));
            }
        };
        axis_state.axis = rebuild_axis(
            axis,
            AxisUpdate {
                coordinates: Some(coordinates),
                ..AxisUpdate::default()
            },
        )?;
        // A singleton does not permute bins. For longer axes, do not infer
        // restoration of the canonical mapping from coordinate direction.
        if axis_state.axis.points() > 1 {
            axis_state.latest_fft = None;
        }
        ResolvedOperation::ReverseAxis
    }))
}
