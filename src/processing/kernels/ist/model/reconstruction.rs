use crate::Complex64;
use crate::execution::ExecutionContext;
use crate::execution::ExecutionStage;
use crate::execution::ProgressTotal;
use rustfft::FftDirection;

use super::iteration::initial_columns;
use super::iteration::plan_fft;
use super::iteration::reconstruct_iteration;
use super::iteration::relative_change;
use super::iteration::scatter_measured;
use super::resources::checked_product_u128;
use super::resources::estimate_work;
use super::resources::memory_preflight;
use super::resources::try_complex_vec;
use super::resources::try_f64_vec;
use super::resources::try_u8_vec;
use super::{IstError, IstInput, IstOptions, IstOutput, validate_options};

pub(super) fn reconstruct(
    input: &IstInput,
    options: IstOptions,
    control: &mut ExecutionContext<'_>,
    general: bool,
) -> Result<IstOutput, IstError> {
    control.begin(ExecutionStage::Reconstruction, None, None)?;

    validate_options(options)?;
    let sigma = options
        .noise_standard_deviation
        .ok_or(IstError::NoiseEstimateRequired)?;
    let estimated_work = estimate_work(input, options.max_iterations, general)?;
    if estimated_work > options.max_reconstruction_work {
        return Err(IstError::WorkLimit);
    }
    let memory = memory_preflight(input, general)?;
    if memory.output_bytes > options.max_output_bytes {
        return Err(IstError::OutputLimit);
    }
    if memory.peak_working_bytes > options.max_working_bytes {
        return Err(IstError::WorkingLimit);
    }
    control.ensure_work(estimated_work)?;
    control.observe_payload(memory.peak_working_bytes);
    control.begin(
        ExecutionStage::Reconstruction,
        None,
        Some(ProgressTotal::UpperBound(estimated_work)),
    )?;
    let scatter_work = (input.nlogical * input.f2_points * input.cartesian_fields) as u128;
    control.charge(estimate_work(input, 0, general)? - scatter_work)?;

    let mut compact_y = try_complex_vec(input.complex_len())?;
    for (index, (target, pair)) in compact_y
        .iter_mut()
        .zip(input.components.chunks_exact(2))
        .enumerate()
    {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        *target = Complex64::new(pair[0], pair[1]);
    }

    let k = 2 * input.cartesian_fields;
    let mut mask = try_u8_vec(input.nlogical)?;
    for &coordinate in &input.measured_indices {
        mask[coordinate] = 1;
    }
    let dense_len = checked_product_u128(&[
        input.nlogical as u128,
        input.f2_points as u128,
        input.cartesian_fields as u128,
    ])?;
    let dense_len = usize::try_from(dense_len).map_err(|_| IstError::SizeOverflow)?;
    let mut x_current = try_complex_vec(dense_len)?;
    scatter_measured(control, input, &compact_y, &mut x_current)?;

    let forward = plan_fft(input.nlogical, FftDirection::Forward)?;
    let inverse = plan_fft(input.nlogical, FftDirection::Inverse)?;
    let line_len = input
        .nlogical
        .checked_mul(input.cartesian_fields)
        .ok_or(IstError::SizeOverflow)?;
    let mut lines = try_complex_vec(line_len)?;
    let scratch_len = forward
        .get_inplace_scratch_len()
        .max(inverse.get_inplace_scratch_len());
    let mut fft_scratch = try_complex_vec(scratch_len)?;

    let mut columns = initial_columns(
        control,
        input,
        &x_current,
        &mut lines,
        &mut fft_scratch,
        &forward,
    )?;
    if columns.iter().all(|column| column.converged) && !general {
        return Err(IstError::NoRecoverableSignal);
    }
    let full = general && input.measured_indices.len() == input.nlogical;
    if full {
        for column in &mut columns {
            column.converged = true;
        }
    }
    // A larger iteration ceiling must not slow threshold continuation. Retain
    // the existing 0.98 continuation rate once the budget permits it.
    let decay = if general {
        (1e-6_f64 / 0.9)
            .powf(1.0 / options.max_iterations.saturating_sub(10).max(1) as f64)
            .min(0.98)
    } else {
        0.98
    };
    // Both the noise threshold and numerical floor are column-local.
    // Cartesian fields remain grouped within each direct column.
    let log_p = (input.nlogical as f64).ln();
    let k_f64 = k as f64;
    let noise_floor = sigma
        * (input.measured_indices.len() as f64).sqrt()
        * (k_f64 + 2.0 * (k_f64 * log_p).sqrt() + 2.0 * log_p).sqrt();
    if !noise_floor.is_finite() {
        return Err(IstError::NumericalInvariantViolation);
    }
    for column in &mut columns {
        if column.converged {
            continue;
        }
        let amplitude = column.lambda;
        column.lambda = 0.9 * amplitude;
        column.floor = noise_floor.max(1e-6 * amplitude);
        if !column.lambda.is_finite() || !column.floor.is_finite() {
            return Err(IstError::NumericalInvariantViolation);
        }
        if noise_floor >= column.lambda {
            if !general {
                return Err(IstError::NoRecoverableSignal);
            }
            // A low-signal direct column does not invalidate its neighbours.
            // Start at the noise floor and apply the usual shrink/replace map.
            // When the floor exceeds the entire initial spectrum, zero-filled
            // measured data is already a fixed point of that map. In the narrow
            // interval [0.9*amplitude, amplitude), run the map normally: freezing
            // here would incorrectly discard above-threshold coefficients.
            column.lambda = column.floor;
        }
    }

    let mut x_next = try_complex_vec(dense_len)?;
    let mut previous = if general {
        try_complex_vec(dense_len)?
    } else {
        Vec::new()
    };
    if general {
        previous.copy_from_slice(&x_current);
    }
    let column_work = (estimate_work(input, 1, general)? - estimate_work(input, 0, general)?)
        / input.f2_points as u128;
    let mut iterations = 0usize;
    for _ in 0..options.max_iterations {
        if general && columns.iter().all(|c| c.converged) {
            break;
        }
        control.check_cancelled()?;
        iterations += 1;
        reconstruct_iteration(
            control,
            input,
            &x_current,
            &mut x_next,
            &mut lines,
            &mut fft_scratch,
            &forward,
            &inverse,
            &mask,
            &columns,
            column_work,
        )?;
        scatter_measured(control, input, &compact_y, &mut x_next)?;
        for (f2, column) in columns.iter_mut().enumerate() {
            if column.converged {
                continue;
            }
            column.iterations += 1;
            let relative = relative_change(input, f2, &x_next, &x_current);
            column.relative_change = relative;
            if !relative.is_finite() {
                return Err(IstError::NumericalInvariantViolation);
            }
            if column.lambda <= column.floor && relative <= 1e-6 {
                column.converged_at_floor += 1;
            } else {
                column.converged_at_floor = 0;
            }
            column.converged = column.converged_at_floor == 3;
            if general {
                // At a fixed threshold T = P_measured prox_g is projected
                // gradient descent on the smooth Moreau envelope of g. Apply
                // FISTA extrapolation only once that threshold stops changing.
                // The stopping residual above is T(y)-y, not the momentum step.
                let beta = if column.lambda <= column.floor && !column.converged {
                    let next_t =
                        (1.0 + (1.0 + 4.0 * column.momentum_t * column.momentum_t).sqrt()) / 2.0;
                    let beta = (column.momentum_t - 1.0) / next_t;
                    column.momentum_t = next_t;
                    beta
                } else {
                    0.0
                };
                for (ordinal, index) in super::iteration::column_indices(input, f2).enumerate() {
                    if ordinal % 4096 == 0 {
                        control.check_cancelled()?;
                    }
                    let actual = x_next[index];
                    // All three arrays carry identical retained samples. Avoid
                    // arithmetic on measured values to preserve signed-zero bits.
                    let row = index / (input.f2_points * input.cartesian_fields);
                    if mask[row] == 0 {
                        x_next[index] = actual + (actual - previous[index]) * beta;
                        if !x_next[index].re.is_finite() || !x_next[index].im.is_finite() {
                            return Err(IstError::NumericalInvariantViolation);
                        }
                    }
                    previous[index] = actual;
                }
            }
            column.lambda = (column.lambda * decay).max(column.floor);
        }
        std::mem::swap(&mut x_current, &mut x_next);
        if columns.iter().all(|column| column.converged) {
            break;
        }
    }
    if columns.iter().any(|column| !column.converged) {
        return Err(IstError::DidNotConverge);
    }

    // At scatter, compact y, x_final, column diagnostics and public output remain.
    drop(x_next);
    drop(previous);
    drop(lines);
    drop(fft_scratch);
    drop(mask);
    drop(forward);
    drop(inverse);
    control.charge(scatter_work)?;
    let mut components = try_f64_vec(dense_len.checked_mul(2).ok_or(IstError::SizeOverflow)?)?;
    for (index, (target, value)) in components.chunks_exact_mut(2).zip(&x_current).enumerate() {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        target[0] = value.re;
        target[1] = value.im;
    }
    verify_retained_bits(control, input, &components)?;
    drop(compact_y);
    drop(x_current);
    control.complete_work()?;
    Ok(IstOutput {
        nlogical: input.nlogical,
        f2_points: input.f2_points,
        cartesian_fields: input.cartesian_fields,
        components,
        iterations,
        estimated_work,
        noise_standard_deviation: sigma,
        columns,
    })
}

pub(super) fn verify_retained_bits(
    control: &mut ExecutionContext<'_>,
    input: &IstInput,
    output: &[f64],
) -> Result<(), IstError> {
    for (measured, &coordinate) in input.measured_indices.iter().enumerate() {
        for f2 in 0..input.f2_points {
            if f2 % 4096 == 0 {
                control.check_cancelled()?;
            }
            for field in 0..input.cartesian_fields {
                control.check_cancelled()?;
                for component in 0..2 {
                    let source =
                        (((measured * input.f2_points + f2) * input.cartesian_fields + field) * 2)
                            + component;
                    let target = (((coordinate * input.f2_points + f2) * input.cartesian_fields
                        + field)
                        * 2)
                        + component;
                    if input.components[source].to_bits() != output[target].to_bits() {
                        return Err(IstError::RetainedBitsMismatch);
                    }
                }
            }
        }
    }
    Ok(())
}
