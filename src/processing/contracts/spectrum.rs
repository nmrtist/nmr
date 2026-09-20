use super::error::ProcessingError;
use super::{
    operation::*,
    state::{PlanState, StateError},
};
use crate::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use crate::processed::{ComponentBasis, ProcessedAxis};

pub(crate) fn uniform_step(axis: &ProcessedAxis) -> Result<f64, StateError> {
    match axis.coordinates() {
        AxisCoordinates::Uniform { step, .. } if *step != 0.0 => Ok(*step),
        AxisCoordinates::Explicit(x) if x.len() > 1 => {
            let step = x[1] - x[0];
            if step != 0.0
                && x.windows(2)
                    .all(|p| ((p[1] - p[0]) - step).abs() <= 1e-10 * step.abs())
            {
                Ok(step)
            } else {
                Err(StateError::InvalidParameter("uniform coordinates required"))
            }
        }
        _ => Err(StateError::InvalidParameter("uniform coordinates required")),
    }
}

pub(crate) fn effective_window(requested: usize, n: usize) -> usize {
    if n < 3 {
        1
    } else {
        (requested.max(3) | 1).min(if n % 2 == 0 { n - 1 } else { n })
    }
}

pub(crate) fn transition(
    state: &mut PlanState,
    axis_index: usize,
    request: &SpectrumOperation,
) -> Result<ResolvedOperation, StateError> {
    let invalid = |reason| StateError::InvalidState {
        axis: axis_index,
        operation: "spectrum operation",
        reason,
    };
    let selected = state
        .axes
        .get(axis_index)
        .ok_or_else(|| invalid("axis is out of bounds"))?;
    let axis = &selected.axis;
    let n = axis.points();
    let c = axis.component_count();
    if state
        .axes
        .iter()
        .any(|a| matches!(a.axis.component_basis(), ComponentBasis::Encoded(_)))
    {
        return Err(invalid(
            "resolve encoded components before spectrum manipulation",
        ));
    }
    let mut resolved = request.clone();
    let mut coordinates = axis.coordinates().clone();
    let mut points = n;
    let mut basis = axis.component_basis().clone();
    let mut rebuild = false;
    use SpectrumOperation as Op;
    let shared = state.axes.iter().any(|a| {
        matches!(
            a.axis.component_basis(),
            ComponentBasis::SharedComplex { .. }
        )
    });
    if shared
        && !matches!(
            request,
            Op::Reverse
                | Op::Invert
                | Op::Affine { .. }
                | Op::RetainRange { .. }
                | Op::Reference { .. }
                | Op::Slice { .. }
        )
    {
        return Err(invalid(
            "project shared complex components before reducing or filtering spectrum axes",
        ));
    }
    match request {
        Op::Slice { index, component } => {
            if *index >= n || *component >= c {
                return Err(invalid("slice index or component is out of bounds"));
            }
            if shared && *component != 0 {
                return Err(invalid(
                    "shared complex slicing requires component zero and retains the complete pair",
                ));
            }
        }
        Op::Sum { component } | Op::Skyline { component } => {
            if *component >= c {
                return Err(invalid("component is out of bounds"));
            }
        }
        Op::Reverse | Op::Invert | Op::Affine { .. } | Op::RetainRange { .. } => {}
        _ if axis.domain() != AxisDomain::Frequency || !axis.role().is_signal() => {
            return Err(invalid("requires a frequency-domain signal axis"));
        }
        _ => {}
    }
    match request {
        Op::RetainRange { start, end } => {
            if start >= end || *end > n {
                return Err(invalid("invalid retained range"));
            }
            points = end - start;
            coordinates = match axis.coordinates() {
                AxisCoordinates::Uniform {
                    start: origin,
                    step,
                } => AxisCoordinates::Uniform {
                    start: step.mul_add(*start as f64, *origin),
                    step: *step,
                },
                AxisCoordinates::Explicit(values) => {
                    AxisCoordinates::Explicit(values[*start..*end].to_vec())
                }
                AxisCoordinates::Unknown => AxisCoordinates::Unknown,
            };
            rebuild = true;
        }
        Op::Reference { delta_ppm } => {
            if !delta_ppm.is_finite() || axis.unit() != Some(AxisUnit::Ppm) {
                return Err(invalid("requires a finite ppm shift on a ppm axis"));
            }
            coordinates = match &coordinates {
                AxisCoordinates::Uniform { start, step } => AxisCoordinates::Uniform {
                    start: start + delta_ppm,
                    step: *step,
                },
                AxisCoordinates::Explicit(values) => {
                    AxisCoordinates::Explicit(values.iter().map(|v| v + delta_ppm).collect())
                }
                AxisCoordinates::Unknown => return Err(invalid("requires known coordinates")),
            };
            rebuild = true;
        }
        Op::Affine { scale, real_offset } if !scale.is_finite() || !real_offset.is_finite() => {
            return Err(invalid("non-finite affine parameters"));
        }
        Op::MovingAverage { window } => {
            if n >= 3 {
                uniform_step(axis)?;
            }
            resolved = Op::MovingAverage {
                window: effective_window(*window, n),
            };
        }
        Op::SavitzkyGolay { window, order } => {
            if n >= 3 {
                uniform_step(axis)?;
            }
            let window = effective_window(*window, n);
            let order = if window == 1 {
                0
            } else {
                (*order).max(1).min(window - 1)
            };
            if order > 12 {
                return Err(invalid("polynomial degree above 12"));
            }
            resolved = Op::SavitzkyGolay { window, order };
        }
        Op::Baseline(method) => match method {
            RealBaseline::Offset => {}
            RealBaseline::Polynomial { order } if *order > 12 || n <= *order => {
                return Err(invalid("polynomial degree or sample count"));
            }
            RealBaseline::Polynomial { .. } => {}
            RealBaseline::Asls {
                lambda,
                asymmetry,
                iterations,
            } => {
                if n < 3
                    || !lambda.is_finite()
                    || !(1.0..=1e12).contains(lambda)
                    || !asymmetry.is_finite()
                    || !(1e-6..=0.5).contains(asymmetry)
                    || !(1..=100).contains(iterations)
                {
                    return Err(invalid("AsLS parameters or sample count"));
                }
            }
        },
        Op::Normalize(Normalization::Constant(value)) if !value.is_finite() || *value == 0.0 => {
            return Err(invalid("normalization divisor must be finite and nonzero"));
        }
        Op::Normalize(Normalization::TotalArea { singleton_width }) => {
            if n == 1 {
                if !singleton_width.is_some_and(|w| w.is_finite() && w > 0.0) {
                    return Err(invalid(
                        "singleton area requires an explicit positive width",
                    ));
                }
            } else {
                uniform_step(axis)?;
            }
        }
        Op::Bin { width, .. } => {
            if !width.is_finite() || *width <= 0.0 {
                return Err(invalid("bin width must be finite and positive"));
            }
            let per = bin_points(axis, *width)?;
            points = n.div_ceil(per);
            let mut values = Vec::new();
            values
                .try_reserve_exact(points)
                .map_err(|_| StateError::AllocationFailure)?;
            for start in (0..n).step_by(per) {
                let end = (start.saturating_add(per)).min(n);
                let a =
                    coordinate(axis, start).map_err(|_| invalid("requires known coordinates"))?;
                let b =
                    coordinate(axis, end - 1).map_err(|_| invalid("requires known coordinates"))?;
                values.push(a / 2.0 + b / 2.0);
            }
            coordinates = AxisCoordinates::Explicit(values);
            rebuild = true;
        }
        Op::Magnitude => {
            basis = ComponentBasis::Scalar;
            rebuild = true;
        }
        _ => {}
    }
    if request.removes_axis() {
        if state.axes.len() != 2 {
            return Err(invalid("dimension reduction requires rank two"));
        }
        state.axes.remove(axis_index);
        // If the removed axis owned the pair, materialize it on the surviving
        // axis in that axis's imaginary orientation. Its evidence and phase
        // state remain attached to the same physical axis.
        let survivor = &mut state.axes[0].axis;
        if matches!(
            survivor.component_basis(),
            ComponentBasis::SharedComplex { .. }
        ) {
            *survivor = survivor.rebuilt(
                survivor.domain(),
                survivor.unit(),
                survivor.points(),
                survivor.coordinates().clone(),
                ComponentBasis::Cartesian,
                survivor.spectral_width_hz(),
            )?;
        }
        let mut origin = state.absolute_origin.to_vec();
        origin.remove(axis_index);
        state.absolute_origin = origin.into();
        state.observation_ordinals = None;
    } else {
        if rebuild {
            state.axes[axis_index].axis = axis.rebuilt(
                axis.domain(),
                axis.unit(),
                points,
                coordinates,
                basis,
                axis.spectral_width_hz(),
            )?;
        }
        if let Op::RetainRange { start, .. } = request {
            let mut origin = state.absolute_origin.to_vec();
            origin[axis_index] = origin[axis_index]
                .checked_add(*start)
                .ok_or(StateError::SizeOverflow)?;
            state.absolute_origin = origin.into();
        }
        let selected = &mut state.axes[axis_index];
        if let Op::Reference { delta_ppm } = request {
            if let Some(reference) = &selected.chemical_shift_reference {
                // The original evidence remains inspectable in earlier records.
                selected.chemical_shift_reference = Some(
                    crate::acquisition::ChemicalShiftReference::user_constructed(
                        reference.carrier_ppm() + delta_ppm,
                        reference.reference_frequency_mhz(),
                    )
                    .map_err(|_| invalid("reference shift overflow"))?,
                );
            }
        }
        if matches!(
            request,
            Op::Reverse | Op::Bin { .. } | Op::RetainRange { .. }
        ) {
            selected.latest_fft = None;
        }
        if matches!(request, Op::Magnitude) {
            selected.phase_applied = false;
            selected.latest_fft = None;
        }
        selected.operation_count = selected
            .operation_count
            .checked_add(1)
            .ok_or(StateError::SizeOverflow)?;
    }
    Ok(ResolvedOperation::Spectrum(resolved))
}

pub(crate) fn bin_points(axis: &ProcessedAxis, width: f64) -> Result<usize, StateError> {
    if axis.points() == 1 {
        return Ok(1);
    }
    let ratio = width / uniform_step(axis)?.abs();
    if !ratio.is_finite() {
        return Err(StateError::InvalidParameter("bin width/spacing overflow"));
    }
    Ok(ratio.round().max(1.0).min(axis.points() as f64) as usize)
}

pub(crate) fn evaluate(coefficients: &[f64], x: f64) -> f64 {
    coefficients
        .iter()
        .rev()
        .fold(0.0_f64, |v, c| v.mul_add(x, *c))
}

pub(crate) fn coordinate(axis: &ProcessedAxis, index: usize) -> Result<f64, ProcessingError> {
    match axis.coordinates() {
        AxisCoordinates::Uniform { start, step } => Ok(step.mul_add(index as f64, *start)),
        AxisCoordinates::Explicit(values) => Ok(values[index]),
        AxisCoordinates::Unknown => Err(ProcessingError::MissingCapability {
            capability: "axis coordinates",
            axis: None,
        }),
    }
}
