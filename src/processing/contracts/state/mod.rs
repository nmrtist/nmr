//! Canonical evidence used while resolving deterministic processing operations.
//! State models, initialization and exhaustive deterministic transitions.

use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisUnit;
use crate::processed::ComponentBasis;

mod calibration;
mod initialization;
mod model;
mod projection;
mod transition;

pub(crate) use calibration::{positive_uniform_time, spectral_width, window_weight};
#[cfg(test)]
pub(crate) use initialization::STATE_CONSTRUCTIONS;
pub(crate) use initialization::expand_raw_descriptor;
pub(crate) use model::{AxisState, PlanState, ProcessingDelayState, StateError};
pub(crate) use projection::{
    transition_auto_phase, transition_projection, validate_auto_phase_axis,
};
pub(crate) use transition::transition;

#[derive(Default)]
struct AxisUpdate {
    domain: Option<AxisDomain>,
    unit: Option<Option<AxisUnit>>,
    points: Option<usize>,
    coordinates: Option<AxisCoordinates>,
    component_basis: Option<ComponentBasis>,
    spectral_width_hz: Option<Option<f64>>,
}
