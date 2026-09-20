use super::fft::fft_work;
use super::{buffer::*, tensor::*};
use crate::acquisition::ModulationIndexDomain;
use crate::axis::AxisCoordinates;
use crate::internal::numeric::scaled_l2;
use crate::processed::ComponentBasis;
use crate::processing::contracts::{
    error::ProcessingError,
    operation::*,
    prepared_step::PreparedStep,
    profile::PositivePeaksV1,
    state::{StateError, positive_uniform_time, spectral_width},
};
use crate::{Complex64, ExecutionContext};
use rustfft::{Fft, FftDirection};
use std::{f64::consts::PI, sync::Arc};

impl FourierExponentSign {
    pub(in crate::processing) fn sigma(self) -> f64 {
        match self {
            Self::Negative => -1.0,
            Self::Positive => 1.0,
        }
    }

    pub(in crate::processing) fn direction(self) -> FftDirection {
        match self {
            Self::Negative => FftDirection::Forward,
            Self::Positive => FftDirection::Inverse,
        }
    }
}

pub(in crate::processing) fn baseline_axis(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    profile: PositivePeaksV1,
) -> Result<Vec<f64>, ProcessingError> {
    let axis = &step.before.axes()[step.axis()];
    let coordinates = physical_coordinates(control, axis)?;
    // The selected axis is scalar. Expanding every other axis into its logical
    // points and component lanes gathers each independent scalar trace once.
    let shape = storage_shape(&step.before)?;
    let mut output = try_zeroed(current.len())?;
    for line in 0..other_line_count(&shape, step.axis())? {
        control.check_cancelled()?;
        let other = other_coordinate(&shape, step.axis(), line)?;
        let mut values = try_vec_capacity(axis.points())?;
        for point in 0..axis.points() {
            values.push(sample_at(current, &shape, step.axis(), &other, point)?);
        }
        let corrected = profile.subtract_controlled(&coordinates, &values, control)?;
        for (point, value) in corrected.into_iter().enumerate() {
            set_sample(&mut output, &shape, step.axis(), &other, point, value)?;
        }
    }
    Ok(output)
}

pub(in crate::processing) fn apply_component_transform(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    resolved: &crate::acquisition::ResolvedComponentTransform,
    observation_ordinals: Option<&[usize]>,
    grid_origin: i64,
) -> Result<Vec<f64>, ProcessingError> {
    if step.before.axes().len() != 2
        || step.axis() != 0
        || !matches!(
            step.before.axes()[1].component_basis(),
            ComponentBasis::Cartesian
        )
    {
        return Err(ProcessingError::Mapping(
            "component transform requires a rank-2 input with Cartesian direct samples",
        ));
    }
    let shape = step.before.logical_shape();
    let output_len = checked_product(&storage_shape(&step.after)?)?;
    let mut output = try_zeroed(output_len)?;
    let lanes = resolved.input_lanes();
    let mut input = Vec::new();
    input
        .try_reserve_exact(lanes)
        .map_err(|_| ProcessingError::AllocationFailure)?;
    for indirect in 0..shape[0] {
        for direct in 0..shape[1] {
            control.charge((lanes as u128) * 2)?;
            let logical = [indirect, direct];
            input.clear();
            for lane in 0..lanes {
                input.push(Complex64::new(
                    tensor_sample(current, &step.before, &logical, &[lane, 0])?,
                    tensor_sample(current, &step.before, &logical, &[lane, 1])?,
                ));
            }
            let modulation_index = match resolved.transform().modulation().domain() {
                ModulationIndexDomain::AbsoluteGridCoordinate(axis) => {
                    let value = *logical.get(axis.index()).ok_or(ProcessingError::Mapping(
                        "component modulation axis is outside the descriptor",
                    ))?;
                    grid_origin
                        .checked_add(
                            i64::try_from(value).map_err(|_| ProcessingError::SizeOverflow)?,
                        )
                        .ok_or(ProcessingError::SizeOverflow)?
                }
                ModulationIndexDomain::ObservationOrdinal => i64::try_from(
                    observation_ordinals
                        .and_then(|mapping| mapping.get(indirect))
                        .copied()
                        .unwrap_or(indirect),
                )
                .map_err(|_| ProcessingError::SizeOverflow)?,
            };
            let transformed = resolved
                .transform()
                .apply(&input, modulation_index)
                .map_err(|_| ProcessingError::Mapping("component transform application failed"))?;
            for (component, value) in transformed.into_iter().enumerate() {
                tensor_set(
                    &mut output,
                    &step.after,
                    &logical,
                    &[component, 0],
                    value.re,
                )?;
                tensor_set(
                    &mut output,
                    &step.after,
                    &logical,
                    &[component, 1],
                    value.im,
                )?;
            }
        }
    }
    Ok(output)
}

pub(in crate::processing) fn project_scalar(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    projection: Projection,
) -> Result<Vec<f64>, ProcessingError> {
    let shape = step.before.logical_shape();
    let logical_count = checked_product(&shape)?;
    let component_counts = step.before.component_counts();
    let component_count = checked_product(&component_counts)?;
    let zero_components = try_zeroed_usize(shape.len())?;
    let mut output = try_vec_capacity(logical_count)?;
    for logical_index in 0..logical_count {
        charge_block(control, logical_index, logical_count)?;
        let logical = unflatten(&shape, logical_index)?;
        let value = match projection {
            Projection::Real
            | Projection::RealAbsorptive
            | Projection::RealSigned
            | Projection::UnphasedReal => {
                tensor_sample(current, &step.before, &logical, &zero_components)?
            }
            Projection::Magnitude => {
                let mut fields = try_vec_capacity(component_count)?;
                for component_index in 0..component_count {
                    let components = unflatten(&component_counts, component_index)?;
                    fields.push(tensor_sample(current, &step.before, &logical, &components)?);
                }
                scaled_l2(fields)
            }
        };
        if !value.is_finite() {
            return Err(ProcessingError::NumericalInvariantViolation);
        }
        output.push(value);
    }
    Ok(output)
}

pub(in crate::processing) fn reverse_axis(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
) -> Result<Vec<f64>, ProcessingError> {
    let storage = storage_shape(&step.before)?;
    let component_count = step.before.axes()[step.axis()].component_count();
    let points = step.before.axes()[step.axis()].points();
    let mut output = try_zeroed(current.len())?;
    for (target_index, target) in output.iter_mut().enumerate() {
        charge_block(control, target_index, current.len())?;
        let mut source_coordinate = unflatten(&storage, target_index)?;
        let expanded = source_coordinate[step.axis()];
        let logical = expanded / component_count;
        let component = expanded % component_count;
        source_coordinate[step.axis()] = (points - 1 - logical)
            .checked_mul(component_count)
            .and_then(|value| value.checked_add(component))
            .ok_or(ProcessingError::SizeOverflow)?;
        *target = current[flatten(&storage, &source_coordinate)?];
    }
    Ok(output)
}

pub(in crate::processing) fn map_retained(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    map: impl Fn(usize, f64) -> Result<f64, ProcessingError>,
) -> Result<Vec<f64>, ProcessingError> {
    let shape = storage_shape(&step.before)?;
    let mut next = try_vec_capacity(current.len())?;
    for (index, &value) in current.iter().enumerate() {
        charge_block(control, index, current.len())?;
        let coordinate = unflatten(&shape, index)?;
        let components = step.before.axes()[step.axis()].component_count();
        next.push(map(coordinate[step.axis()] / components, value)?);
    }
    Ok(next)
}

pub(in crate::processing) fn zero_fill(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
) -> Result<Vec<f64>, ProcessingError> {
    let old_shape = storage_shape(&step.before)?;
    let new_shape = storage_shape(&step.after)?;
    let new_len = checked_product(&new_shape)?;
    let mut next = try_zeroed(new_len)?;
    for (old_index, &value) in current.iter().enumerate() {
        charge_block(control, old_index, current.len())?;
        let coordinate = unflatten(&old_shape, old_index)?;
        let new_index = flatten(&new_shape, &coordinate)?;
        next[new_index] = value;
    }
    Ok(next)
}

pub(in crate::processing) fn fft_axis(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    sign: FourierExponentSign,
) -> Result<Vec<f64>, ProcessingError> {
    let before_axis = &step.before.axes()[step.axis()];
    let points = before_axis.points();
    let (_, dt) = positive_uniform_time(before_axis, &|reason| StateError::InvalidState {
        axis: step.axis(),
        operation: "Fourier transform",
        reason,
    })?;
    let sw = spectral_width(before_axis, dt, &|reason| StateError::InvalidState {
        axis: step.axis(),
        operation: "Fourier transform",
        reason,
    })?;
    let t0 = match before_axis.coordinates() {
        AxisCoordinates::Uniform { start, .. } => *start,
        _ => unreachable!("preflight requires uniform coordinates"),
    };
    let fft = plan_fft(points, sign.direction())?;
    let mut scratch = try_zeroed_complex(fft.get_inplace_scratch_len())?;
    map_complex_axis(control, current, step, |control, values| {
        control.charge(fft_work(points))?;
        fft.process_with_scratch(values, &mut scratch);
        values.rotate_left(points.div_ceil(2));
        for (point, value) in values.iter_mut().enumerate() {
            let q = point as isize - (points / 2) as isize;
            let frequency = q as f64 * sw / points as f64;
            *value *= unit_complex(sign.sigma() * 2.0 * PI * frequency * t0);
        }
        Ok(())
    })
}

pub(in crate::processing) fn complex_pointwise(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    map: impl Fn(usize, usize, Complex64) -> Complex64,
) -> Result<Vec<f64>, ProcessingError> {
    let points = step.before.axes()[step.axis()].points();
    map_complex_axis(control, current, step, |control, values| {
        for (point, value) in values.iter_mut().enumerate() {
            charge_block(control, point, points)?;
            *value = map(point, points, *value);
        }
        Ok(())
    })
}

// A shared axis rotates the owner's complex pair in its own imaginary
// orientation. It must never Fourier transform those two fields separately.
fn map_complex_axis(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    mut map: impl FnMut(&mut ExecutionContext<'_>, &mut [Complex64]) -> Result<(), ProcessingError>,
) -> Result<Vec<f64>, ProcessingError> {
    let selected = step.axis();
    let points = step.before.axes()[selected].points();
    let before = storage_shape(&step.before)?;
    let after = storage_shape(&step.after)?;
    let mut output = try_zeroed(checked_product(&after)?)?;
    let shared = match step.before.axes()[selected].component_basis() {
        ComponentBasis::SharedComplex { axis, conjugated } => Some((
            axis.index() - usize::from(axis.index() > selected),
            if *conjugated { -1.0 } else { 1.0 },
        )),
        _ => None,
    };
    for line in 0..other_line_count(&before, selected)? {
        control.check_cancelled()?;
        let mut other = other_coordinate(&before, selected, line)?;
        if let Some((owner, _)) = shared {
            if other[owner] % 2 != 0 {
                continue;
            }
        }
        let mut values = if let Some((owner, orientation)) = shared {
            let mut values = try_vec_capacity(points)?;
            for point in 0..points {
                if point % 4096 == 0 {
                    control.check_cancelled()?;
                }
                let re = sample_at(current, &before, selected, &other, point)?;
                other[owner] += 1;
                let im = sample_at(current, &before, selected, &other, point)?;
                other[owner] -= 1;
                values.push(Complex64::new(re, orientation * im));
            }
            values
        } else {
            gather_complex(control, current, &before, selected, &other, points)?
        };
        map(control, &mut values)?;
        if let Some((owner, orientation)) = shared {
            for (point, value) in values.iter().enumerate() {
                if point % 4096 == 0 {
                    control.check_cancelled()?;
                }
                set_sample(&mut output, &after, selected, &other, point, value.re)?;
                other[owner] += 1;
                set_sample(
                    &mut output,
                    &after,
                    selected,
                    &other,
                    point,
                    orientation * value.im,
                )?;
                other[owner] -= 1;
            }
        } else {
            scatter_complex(control, &mut output, &after, selected, &other, &values)?;
        }
    }
    Ok(output)
}

pub(in crate::processing) fn time_domain_shift_fold_v1(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    delay: f64,
    skip: usize,
    fold: usize,
) -> Result<Vec<f64>, ProcessingError> {
    let before_shape = storage_shape(&step.before)?;
    let after_shape = storage_shape(&step.after)?;
    let points = step.before.axes()[step.axis()].points();
    let output_points = points - skip;
    let mut next = try_zeroed(checked_product(&after_shape)?)?;
    let negative = plan_fft(points, FftDirection::Forward)?;
    let positive = plan_fft(points, FftDirection::Inverse)?;
    let scratch_len = negative
        .get_inplace_scratch_len()
        .max(positive.get_inplace_scratch_len());
    let mut scratch = try_zeroed_complex(scratch_len)?;
    for line in 0..other_line_count(&before_shape, step.axis())? {
        control.charge(2 * fft_work(points))?;
        let other = other_coordinate(&before_shape, step.axis(), line)?;
        let mut values =
            gather_complex(control, current, &before_shape, step.axis(), &other, points)?;
        // nmrglue fsh2: negative DFT of ifftshift(x), normalized by N,
        // positive delay phase, then positive DFT and fftshift.
        values.rotate_left(points / 2);
        negative.process_with_scratch(&mut values, &mut scratch);
        for (index, value) in values.iter_mut().enumerate() {
            *value = *value / points as f64
                * unit_complex(2.0 * PI * delay * index as f64 / points as f64);
        }
        positive.process_with_scratch(&mut values, &mut scratch);
        values.rotate_left(points.div_ceil(2));
        if fold != 0 {
            // Preserve the complete pre-fold tail so overlapping additions do
            // not observe values already modified at the front of the trace.
            let mut tail = try_vec_capacity(fold)?;
            tail.extend_from_slice(&values[points - fold..]);
            for index in 0..fold {
                values[index] += tail[fold - 1 - index];
            }
        }
        scatter_complex(
            control,
            &mut next,
            &after_shape,
            step.axis(),
            &other,
            &values[..output_points],
        )?;
    }
    Ok(next)
}

pub(in crate::processing) fn plan_fft(
    points: usize,
    direction: FftDirection,
) -> Result<Arc<dyn Fft<f64>>, ProcessingError> {
    crate::processing::kernels::fft::plan(points, direction).map_err(Into::into)
}
