use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisUnit;
use crate::processed::ComponentBasis;
use crate::processing::contracts::operation::*;

use super::super::calibration::{
    positive_uniform_time, rebuild_axis, require_time_signal, spectral_width, window_weight,
};
use super::super::{AxisState, AxisUpdate, StateError};
use super::AxisTransition;

pub(super) fn window(
    axis_state: &mut AxisState,
    axis_index: usize,
    window: &Window,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        window.validate().map_err(StateError::InvalidParameter)?;
        require_time_signal(axis, &invalid)?;
        let mut identity = true;
        for index in 0..axis.points() {
            identity &= window_weight(window, axis, axis_index, index)? == 1.0;
        }
        if identity {
            return Ok(AxisTransition::Identity(ResolvedOperation::Window(
                window.clone(),
            )));
        }
        ResolvedOperation::Window(window.clone())
    }))
}

pub(super) fn zero_fill(
    axis_state: &mut AxisState,
    zero_fill: &ZeroFill,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        require_time_signal(axis, &invalid)?;
        if zero_fill.target_points < axis.points() {
            return Err(invalid(
                "target must not be smaller than the current point count",
            ));
        }
        if zero_fill.target_points == axis.points() {
            return Ok(AxisTransition::Identity(ResolvedOperation::ZeroFill {
                target_points: zero_fill.target_points,
            }));
        }
        positive_uniform_time(axis, &invalid)?;
        let coordinates = match axis.coordinates() {
            AxisCoordinates::Uniform { start, step } => AxisCoordinates::Uniform {
                start: *start,
                step: *step,
            },
            AxisCoordinates::Unknown => AxisCoordinates::Unknown,
            AxisCoordinates::Explicit(_) => {
                return Err(invalid(
                    "explicit coordinates cannot be extended without invention",
                ));
            }
        };
        axis_state.axis = rebuild_axis(
            axis,
            AxisUpdate {
                points: Some(zero_fill.target_points),
                coordinates: Some(coordinates),
                ..AxisUpdate::default()
            },
        )?;
        ResolvedOperation::ZeroFill {
            target_points: zero_fill.target_points,
        }
    }))
}

pub(super) fn standard_zero_fill(
    axis_state: &mut AxisState,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        require_time_signal(axis, &invalid)?;
        positive_uniform_time(axis, &invalid)?;
        let doubled = axis
            .points()
            .checked_mul(2)
            .ok_or(StateError::SizeOverflow)?;
        let target_points = doubled
            .checked_next_power_of_two()
            .ok_or(StateError::SizeOverflow)?;
        axis_state.axis = rebuild_axis(
            axis,
            AxisUpdate {
                points: Some(target_points),
                coordinates: Some(axis.coordinates().clone()),
                ..AxisUpdate::default()
            },
        )?;
        ResolvedOperation::ZeroFill { target_points }
    }))
}

pub(super) fn fourier_transform(
    axis_state: &mut AxisState,
    transform: &FourierTransform,
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        require_time_signal(axis, &invalid)?;
        if !matches!(
            axis.component_basis(),
            ComponentBasis::Scalar
                | ComponentBasis::Cartesian
                | ComponentBasis::SharedComplex { .. }
        ) {
            return Err(invalid("component basis is not transformable complex data"));
        }
        let (t0, dt) = positive_uniform_time(axis, &invalid)?;
        let sw = spectral_width(axis, dt, &invalid)?;
        let points = axis.points();
        let step = sw / points as f64;
        let start = -((points / 2) as f64) * step;
        axis_state.axis = rebuild_axis(
            axis,
            AxisUpdate {
                domain: Some(AxisDomain::Frequency),
                unit: Some(Some(AxisUnit::Hertz)),
                coordinates: Some(AxisCoordinates::Uniform { start, step }),
                component_basis: Some(match axis.component_basis() {
                    shared @ ComponentBasis::SharedComplex { .. } => shared.clone(),
                    _ => ComponentBasis::Cartesian,
                }),
                spectral_width_hz: Some(Some(sw)),
                ..AxisUpdate::default()
            },
        )?;
        let _ = t0;
        axis_state.latest_fft = Some(transform.sign);
        ResolvedOperation::FourierTransform(transform.sign)
    }))
}
