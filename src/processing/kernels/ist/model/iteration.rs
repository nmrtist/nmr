use crate::Complex64;
use crate::execution::ExecutionContext;
use crate::internal::numeric::ScaledNeumaier;
use crate::internal::numeric::scaled_l2;
use rustfft::Fft;
use rustfft::FftDirection;
use std::sync::Arc;

use super::{ColumnState, IstError, IstInput};

pub(super) fn column_indices(input: &IstInput, f2: usize) -> impl Iterator<Item = usize> + '_ {
    // Iteration buffers are [F2,N,C], keeping each independent column together.
    // Within a column retain logical/field order for compensated reductions.
    let length = input.nlogical * input.cartesian_fields;
    f2 * length..(f2 + 1) * length
}

pub(super) fn relative_change(
    control: &ExecutionContext<'_>,
    input: &IstInput,
    f2: usize,
    next: &[Complex64],
    current: &[Complex64],
) -> Result<f64, IstError> {
    let mut difference_scale = 0.0_f64;
    let mut current_scale = 0.0_f64;
    for (ordinal, index) in column_indices(input, f2).enumerate() {
        if ordinal % 4096 == 0 {
            control.check_cancelled()?;
        }
        let (next, current) = (next[index], current[index]);
        difference_scale = difference_scale
            .max((next.re - current.re).abs())
            .max((next.im - current.im).abs());
        current_scale = current_scale.max(current.re.abs()).max(current.im.abs());
    }
    let mut difference_sum = ScaledNeumaier::default();
    let mut current_sum = ScaledNeumaier::default();
    for (ordinal, index) in column_indices(input, f2).enumerate() {
        if ordinal % 4096 == 0 {
            control.check_cancelled()?;
        }
        let (next, current) = (next[index], current[index]);
        if difference_scale != 0.0 {
            for value in [next.re - current.re, next.im - current.im] {
                let normalized = value / difference_scale;
                difference_sum.add(normalized * normalized);
            }
        }
        if current_scale != 0.0 {
            for value in [current.re, current.im] {
                let normalized = value / current_scale;
                current_sum.add(normalized * normalized);
            }
        }
    }
    let numerator = difference_scale * difference_sum.total().sqrt();
    let denominator = current_scale * current_sum.total().sqrt();
    Ok(numerator / denominator)
}

pub(super) fn scatter_measured(
    control: &mut ExecutionContext<'_>,
    input: &IstInput,
    compact_y: &[Complex64],
    dense: &mut [Complex64],
) -> Result<(), IstError> {
    for (measured, &coordinate) in input.measured_indices.iter().enumerate() {
        for f2 in 0..input.f2_points {
            if f2 % 4096 == 0 {
                control.check_cancelled()?;
            }
            for field in 0..input.cartesian_fields {
                let compact = (measured * input.f2_points + f2) * input.cartesian_fields + field;
                let target = (f2 * input.nlogical + coordinate) * input.cartesian_fields + field;
                dense[target].re = compact_y[compact].re;
                dense[target].im = compact_y[compact].im;
            }
        }
    }
    Ok(())
}

pub(super) fn initial_columns(
    control: &mut ExecutionContext<'_>,
    input: &IstInput,
    x: &[Complex64],
    lines: &mut [Complex64],
    scratch: &mut [Complex64],
    forward: &Arc<dyn Fft<f64>>,
) -> Result<Vec<ColumnState>, IstError> {
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(input.f2_points)
        .map_err(|_| IstError::AllocationFailure)?;
    for f2 in 0..input.f2_points {
        if f2 % 4096 == 0 {
            control.check_cancelled()?;
        }
        let mut amplitude = 0.0_f64;
        gather_lines(control, input, x, f2, lines)?;
        transform_lines(control, input, lines, scratch, forward)?;
        for frequency in 0..input.nlogical {
            if frequency % 1024 == 0 {
                control.check_cancelled()?;
            }
            let rho = scaled_l2((0..input.cartesian_fields).flat_map(|field| {
                let value = lines[field * input.nlogical + frequency];
                [value.re, value.im]
            }));
            if !rho.is_finite() {
                return Err(IstError::NoRecoverableSignal);
            }
            amplitude = amplitude.max(rho);
        }
        columns.push(ColumnState {
            lambda: amplitude,
            floor: 0.0,
            relative_change: 0.0,
            converged_at_floor: 0,
            iterations: 0,
            converged: amplitude == 0.0,
            momentum_t: 1.0,
        });
    }
    Ok(columns)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn reconstruct_iteration(
    control: &mut ExecutionContext<'_>,
    input: &IstInput,
    current: &[Complex64],
    next: &mut [Complex64],
    lines: &mut [Complex64],
    scratch: &mut [Complex64],
    forward: &Arc<dyn Fft<f64>>,
    inverse: &Arc<dyn Fft<f64>>,
    mask: &[u8],
    columns: &[ColumnState],
    column_work: u128,
) -> Result<(), IstError> {
    for (f2, column) in columns.iter().enumerate() {
        control.charge(column_work)?;
        if column.converged {
            // Freeze a column at its own stopping iteration. A strong neighbor
            // cannot change either its threshold or its convergence decision.
            for (ordinal, index) in column_indices(input, f2).enumerate() {
                if ordinal % 4096 == 0 {
                    control.check_cancelled()?;
                }
                next[index] = current[index];
            }
            continue;
        }
        let lambda = column.lambda;
        gather_lines(control, input, current, f2, lines)?;
        transform_lines(control, input, lines, scratch, forward)?;
        for frequency in 0..input.nlogical {
            if frequency % 1024 == 0 {
                control.check_cancelled()?;
            }
            let rho = scaled_l2((0..input.cartesian_fields).flat_map(|field| {
                let value = lines[field * input.nlogical + frequency];
                [value.re, value.im]
            }));
            let factor = if rho == 0.0 {
                0.0
            } else {
                (1.0 - lambda / rho).max(0.0)
            };
            if !factor.is_finite() {
                return Err(IstError::NumericalInvariantViolation);
            }
            for field in 0..input.cartesian_fields {
                lines[field * input.nlogical + frequency] *= factor;
            }
        }
        transform_lines(control, input, lines, scratch, inverse)?;
        for (logical, measured) in mask.iter().enumerate() {
            if logical % 1024 == 0 {
                control.check_cancelled()?;
            }
            if *measured != 0 {
                continue;
            }
            for field in 0..input.cartesian_fields {
                let source = field * input.nlogical + logical;
                let target = (f2 * input.nlogical + logical) * input.cartesian_fields + field;
                next[target].re = lines[source].re / input.nlogical as f64;
                next[target].im = lines[source].im / input.nlogical as f64;
            }
        }
    }
    Ok(())
}

pub(super) fn gather_lines(
    control: &mut ExecutionContext<'_>,
    input: &IstInput,
    dense: &[Complex64],
    f2: usize,
    lines: &mut [Complex64],
) -> Result<(), IstError> {
    for logical in 0..input.nlogical {
        if logical % 1024 == 0 {
            control.check_cancelled()?;
        }
        for field in 0..input.cartesian_fields {
            let source = (f2 * input.nlogical + logical) * input.cartesian_fields + field;
            let target = field * input.nlogical + logical;
            lines[target] = dense[source];
        }
    }
    Ok(())
}

pub(super) fn transform_lines(
    control: &mut ExecutionContext<'_>,
    input: &IstInput,
    lines: &mut [Complex64],
    scratch: &mut [Complex64],
    fft: &Arc<dyn Fft<f64>>,
) -> Result<(), IstError> {
    for field in 0..input.cartesian_fields {
        control.check_cancelled()?;
        let start = field * input.nlogical;
        fft.process_with_scratch(&mut lines[start..start + input.nlogical], scratch);
    }
    Ok(())
}

pub(super) fn plan_fft(
    points: usize,
    direction: FftDirection,
) -> Result<Arc<dyn Fft<f64>>, IstError> {
    crate::processing::kernels::fft::plan(points, direction).map_err(fft_error)
}

pub(super) fn fft_error(error: crate::processing::kernels::fft::FftError) -> IstError {
    match error {
        crate::processing::kernels::fft::FftError::SizeOverflow => IstError::SizeOverflow,
        _ => IstError::NumericalInvariantViolation,
    }
}
