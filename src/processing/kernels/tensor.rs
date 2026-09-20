use super::buffer::*;
use crate::axis::AxisCoordinates;
use crate::processed::{ProcessedAxis, ProcessedDescriptor};
use crate::processing::contracts::error::ProcessingError;
use crate::{Complex64, ExecutionContext};

pub(in crate::processing) fn physical_coordinates(
    control: &mut ExecutionContext<'_>,
    axis: &ProcessedAxis,
) -> Result<Vec<f64>, ProcessingError> {
    match axis.coordinates() {
        AxisCoordinates::Uniform { start, step } => {
            let mut coordinates = try_vec_capacity(axis.points())?;
            for index in 0..axis.points() {
                if index % 4096 == 0 {
                    control.check_cancelled()?;
                }
                let value = step.mul_add(index as f64, *start);
                if !value.is_finite() {
                    return Err(ProcessingError::NumericalInvariantViolation);
                }
                coordinates.push(value);
            }
            Ok(coordinates)
        }
        AxisCoordinates::Explicit(values) => Ok(values.clone()),
        AxisCoordinates::Unknown => Err(ProcessingError::Mapping(
            "baseline correction requires physical coordinates",
        )),
    }
}

pub(in crate::processing) fn gather_complex(
    control: &mut ExecutionContext<'_>,
    samples: &[f64],
    shape: &[usize],
    axis: usize,
    other: &[usize],
    points: usize,
) -> Result<Vec<Complex64>, ProcessingError> {
    let components = shape[axis] / points;
    if !matches!(components, 1 | 2) {
        return Err(ProcessingError::Mapping(
            "selected axis is not scalar or complex",
        ));
    }
    let mut values = try_vec_capacity(points)?;
    for point in 0..points {
        if point % 4096 == 0 {
            control.check_cancelled()?;
        }
        let real = sample_at(samples, shape, axis, other, point * components)?;
        let imaginary = if components == 2 {
            sample_at(samples, shape, axis, other, point * components + 1)?
        } else {
            0.0
        };
        values.push(Complex64::new(real, imaginary));
    }
    Ok(values)
}

pub(in crate::processing) fn scatter_complex(
    control: &mut ExecutionContext<'_>,
    samples: &mut [f64],
    shape: &[usize],
    axis: usize,
    other: &[usize],
    values: &[Complex64],
) -> Result<(), ProcessingError> {
    for (point, value) in values.iter().enumerate() {
        if point % 4096 == 0 {
            control.check_cancelled()?;
        }
        set_sample(samples, shape, axis, other, point * 2, value.re)?;
        set_sample(samples, shape, axis, other, point * 2 + 1, value.im)?;
    }
    Ok(())
}

pub(in crate::processing) fn sample_at(
    samples: &[f64],
    shape: &[usize],
    axis: usize,
    other: &[usize],
    expanded: usize,
) -> Result<f64, ProcessingError> {
    let index = line_sample_index(shape, axis, other, expanded)?;
    samples
        .get(index)
        .copied()
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn tensor_sample(
    samples: &[f64],
    descriptor: &ProcessedDescriptor,
    logical: &[usize],
    components: &[usize],
) -> Result<f64, ProcessingError> {
    samples
        .get(tensor_index(descriptor, logical, components)?)
        .copied()
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn tensor_index(
    descriptor: &ProcessedDescriptor,
    logical: &[usize],
    components: &[usize],
) -> Result<usize, ProcessingError> {
    let axes = descriptor.axes();
    if logical.len() != axes.len() || components.len() != axes.len() {
        return Err(ProcessingError::SizeOverflow);
    }
    let mut index = 0usize;
    for axis in 0..axes.len() {
        if logical[axis] >= axes[axis].points() || components[axis] >= axes[axis].component_count()
        {
            return Err(ProcessingError::SizeOverflow);
        }
        let expanded = logical[axis]
            .checked_mul(axes[axis].component_count())
            .and_then(|value| value.checked_add(components[axis]))
            .ok_or(ProcessingError::SizeOverflow)?;
        index = index
            .checked_mul(axes[axis].points())
            .and_then(|value| value.checked_mul(axes[axis].component_count()))
            .and_then(|value| value.checked_add(expanded))
            .ok_or(ProcessingError::SizeOverflow)?;
    }
    Ok(index)
}

pub(in crate::processing) fn tensor_set(
    samples: &mut [f64],
    descriptor: &ProcessedDescriptor,
    logical: &[usize],
    components: &[usize],
    value: f64,
) -> Result<(), ProcessingError> {
    let index = tensor_index(descriptor, logical, components)?;
    *samples
        .get_mut(index)
        .ok_or(ProcessingError::SizeOverflow)? = value;
    Ok(())
}

pub(in crate::processing) fn set_sample(
    samples: &mut [f64],
    shape: &[usize],
    axis: usize,
    other: &[usize],
    expanded: usize,
    value: f64,
) -> Result<(), ProcessingError> {
    let index = line_sample_index(shape, axis, other, expanded)?;
    *samples
        .get_mut(index)
        .ok_or(ProcessingError::SizeOverflow)? = value;
    Ok(())
}

pub(in crate::processing) fn line_sample_index(
    shape: &[usize],
    axis: usize,
    other: &[usize],
    expanded: usize,
) -> Result<usize, ProcessingError> {
    if axis >= shape.len() || other.len().checked_add(1) != Some(shape.len()) {
        return Err(ProcessingError::SizeOverflow);
    }
    let mut other_values = other.iter();
    shape
        .iter()
        .enumerate()
        .try_fold(0usize, |offset, (dimension, &extent)| {
            let point = if dimension == axis {
                expanded
            } else {
                *other_values.next().ok_or(ProcessingError::SizeOverflow)?
            };
            if point >= extent {
                return Err(ProcessingError::SizeOverflow);
            }
            offset
                .checked_mul(extent)
                .and_then(|value| value.checked_add(point))
                .ok_or(ProcessingError::SizeOverflow)
        })
}

pub(in crate::processing) fn merge_coordinate(
    rank: usize,
    axis: usize,
    other: &[usize],
    expanded: usize,
) -> Result<Vec<usize>, ProcessingError> {
    if axis >= rank || other.len().checked_add(1) != Some(rank) {
        return Err(ProcessingError::SizeOverflow);
    }
    let mut coordinate = try_vec_capacity(rank)?;
    let mut other_index = 0;
    for index in 0..rank {
        if index == axis {
            coordinate.push(expanded);
        } else {
            coordinate.push(other[other_index]);
            other_index += 1;
        }
    }
    Ok(coordinate)
}

pub(in crate::processing) fn other_line_count(
    shape: &[usize],
    axis: usize,
) -> Result<usize, ProcessingError> {
    shape
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != axis)
        .try_fold(1usize, |total, (_, extent)| {
            total
                .checked_mul(*extent)
                .ok_or(ProcessingError::SizeOverflow)
        })
}

pub(in crate::processing) fn other_coordinate(
    shape: &[usize],
    axis: usize,
    mut line: usize,
) -> Result<Vec<usize>, ProcessingError> {
    let mut extents = try_vec_capacity(shape.len().saturating_sub(1))?;
    for (index, extent) in shape.iter().enumerate() {
        if index != axis {
            extents.push(*extent);
        }
    }
    let mut coordinate = try_zeroed_usize(extents.len())?;
    for index in (0..extents.len()).rev() {
        coordinate[index] = line % extents[index];
        line /= extents[index];
    }
    Ok(coordinate)
}

pub(in crate::processing) fn unit_complex(angle: f64) -> Complex64 {
    Complex64::new(angle.cos(), angle.sin())
}

pub(in crate::processing) fn storage_shape(
    descriptor: &ProcessedDescriptor,
) -> Result<Vec<usize>, ProcessingError> {
    let mut shape = try_vec_capacity(descriptor.axes().len())?;
    for axis in descriptor.axes() {
        shape.push(
            axis.points()
                .checked_mul(axis.component_count())
                .ok_or(ProcessingError::SizeOverflow)?,
        );
    }
    Ok(shape)
}

pub(in crate::processing) fn checked_product(values: &[usize]) -> Result<usize, ProcessingError> {
    values.iter().try_fold(1usize, |total, value| {
        total
            .checked_mul(*value)
            .ok_or(ProcessingError::SizeOverflow)
    })
}

pub(in crate::processing) fn flatten(
    shape: &[usize],
    coordinate: &[usize],
) -> Result<usize, ProcessingError> {
    if shape.len() != coordinate.len() {
        return Err(ProcessingError::SizeOverflow);
    }
    shape
        .iter()
        .zip(coordinate)
        .try_fold(0usize, |offset, (&extent, &index)| {
            if index >= extent {
                return Err(ProcessingError::SizeOverflow);
            }
            offset
                .checked_mul(extent)
                .and_then(|value| value.checked_add(index))
                .ok_or(ProcessingError::SizeOverflow)
        })
}

pub(in crate::processing) fn unflatten(
    shape: &[usize],
    mut index: usize,
) -> Result<Vec<usize>, ProcessingError> {
    if index >= checked_product(shape)? {
        return Err(ProcessingError::SizeOverflow);
    }
    let mut coordinate = try_zeroed_usize(shape.len())?;
    for axis in (0..shape.len()).rev() {
        coordinate[axis] = index % shape[axis];
        index /= shape[axis];
    }
    Ok(coordinate)
}
