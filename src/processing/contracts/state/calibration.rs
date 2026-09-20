use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisUnit;
use crate::processed::ProcessedAxis;
use crate::processing::contracts::operation::*;
use std::f64::consts::PI;

use super::{AxisUpdate, StateError};

const SW_DWELL_TOLERANCE: f64 = 1e-9;

pub(crate) fn delay_matches(left: f64, right: f64) -> bool {
    const ABS_TOLERANCE: f64 = 1e-12;
    const REL_TOLERANCE: f64 = 1e-9;
    (left - right).abs() <= ABS_TOLERANCE + REL_TOLERANCE * left.abs().max(right.abs())
}

pub(super) fn frequency_frame_axis(
    axis: &ProcessedAxis,
    frame: &FrequencyFrame,
    reference: Option<&crate::acquisition::ChemicalShiftReference>,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<ProcessedAxis, StateError> {
    let convert = |frequency_hz: f64| -> Result<f64, StateError> {
        match frame {
            FrequencyFrame::Hertz => Ok(frequency_hz),
            FrequencyFrame::Ppm(_) => reference
                .ok_or_else(|| invalid("chemical-shift reference is absent"))?
                .ppm(frequency_hz)
                .map_err(|_| invalid("chemical-shift reference conversion failed")),
        }
    };
    let coordinates = match axis.coordinates() {
        AxisCoordinates::Uniform { start, step } => {
            let converted_start = convert(*start)?;
            let converted_next = convert(*start + *step)?;
            AxisCoordinates::Uniform {
                start: converted_start,
                step: converted_next - converted_start,
            }
        }
        AxisCoordinates::Explicit(values) => {
            let mut converted = Vec::new();
            converted
                .try_reserve_exact(values.len())
                .map_err(|_| StateError::AllocationFailure)?;
            for &value in values {
                converted.push(convert(value)?);
            }
            AxisCoordinates::Explicit(converted)
        }
        AxisCoordinates::Unknown => return Err(invalid("axis has no frequency coordinates")),
    };
    rebuild_axis(
        axis,
        AxisUpdate {
            unit: Some(Some(match frame {
                FrequencyFrame::Hertz => AxisUnit::Hertz,
                FrequencyFrame::Ppm(_) => AxisUnit::Ppm,
            })),
            coordinates: Some(coordinates),
            ..AxisUpdate::default()
        },
    )
}

pub(super) fn require_time_signal(
    axis: &ProcessedAxis,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<(), StateError> {
    if axis.domain() != AxisDomain::Time || !axis.role().is_signal() {
        return Err(invalid("requires a time-domain signal axis"));
    }
    Ok(())
}

pub(crate) fn positive_uniform_time(
    axis: &ProcessedAxis,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<(f64, f64), StateError> {
    match axis.coordinates() {
        AxisCoordinates::Uniform { start, step } if *step > 0.0 => Ok((*start, *step)),
        AxisCoordinates::Unknown => Err(StateError::MissingTimeCalibration),
        _ => Err(invalid("requires positive uniform time coordinates")),
    }
}

pub(crate) fn spectral_width(
    axis: &ProcessedAxis,
    dt: f64,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<f64, StateError> {
    if let Some(sw) = axis.spectral_width_hz() {
        if (sw * dt - 1.0).abs() > SW_DWELL_TOLERANCE {
            return Err(invalid("spectral width and dwell time disagree"));
        }
        Ok(sw)
    } else {
        Ok(1.0 / dt)
    }
}

pub(crate) fn window_weight(
    window: &Window,
    axis: &ProcessedAxis,
    axis_index: usize,
    index: usize,
) -> Result<f64, StateError> {
    let points = axis.points();
    let weight = match window {
        Window::SineBell {
            offset,
            end,
            power,
            first_point_scale,
        } => {
            let fraction = if points == 1 {
                0.0
            } else {
                index as f64 / (points - 1) as f64
            };
            let mut weight = (PI * (offset + (end - offset) * fraction))
                .sin()
                .powf(*power);
            if index == 0 {
                weight *= first_point_scale;
            }
            weight
        }
        Window::LorentzToGauss { lb_hz, gb_hz } => {
            if *lb_hz == 0.0 && *gb_hz == 0.0 {
                return Ok(1.0);
            }
            let invalid = |reason| StateError::InvalidState {
                axis: axis_index,
                operation: "Lorentz-to-Gauss window",
                reason,
            };
            let (_, dt) = positive_uniform_time(axis, &invalid)?;
            let t = index as f64 * dt;
            (PI * lb_hz * t - (PI * gb_hz * t).powi(2) / (4.0 * 2.0_f64.ln())).exp()
        }
        Window::Exponential { lb_hz } => {
            // Zero broadening is independent of time calibration. A nonzero
            // Hz width needs a verified dwell, even if vendor SW is present.
            if *lb_hz == 0.0 {
                return Ok(1.0);
            }
            let invalid = |reason| StateError::InvalidState {
                axis: axis_index,
                operation: "exponential window",
                reason,
            };
            let (_, dt) = positive_uniform_time(axis, &invalid)?;
            let sw = spectral_width(axis, dt, &invalid)?;
            (-PI * lb_hz * index as f64 / sw).exp()
        }
    };
    if !weight.is_finite() {
        return Err(StateError::InvalidParameter("window value"));
    }
    Ok(weight)
}

pub(super) fn rebuild_axis(
    axis: &ProcessedAxis,
    update: AxisUpdate,
) -> Result<ProcessedAxis, StateError> {
    // Build in one checked call so transient domain/unit combinations are never exposed.
    let rebuilt = axis.rebuilt(
        update.domain.unwrap_or(axis.domain()),
        update.unit.unwrap_or(axis.unit()),
        update.points.unwrap_or(axis.points()),
        update
            .coordinates
            .unwrap_or_else(|| axis.coordinates().clone()),
        update
            .component_basis
            .unwrap_or_else(|| axis.component_basis().clone()),
        update.spectral_width_hz.unwrap_or(axis.spectral_width_hz()),
    );
    rebuilt.map_err(Into::into)
}
pub(super) fn checked_floor_to_usize(value: f64) -> Result<usize, StateError> {
    if !value.is_finite() || value < 0.0 || value.floor() >= usize::MAX as f64 {
        return Err(StateError::SizeOverflow);
    }
    Ok(value.floor() as usize)
}
