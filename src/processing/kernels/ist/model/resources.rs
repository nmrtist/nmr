use crate::Complex64;

use super::iteration::fft_error;
use super::{ColumnState, IstError, IstInput, MemoryEstimate};

pub(super) fn memory_preflight(
    input: &IstInput,
    accelerated: bool,
) -> Result<MemoryEstimate, IstError> {
    memory_shape(
        input.nlogical,
        input.measured_indices.len(),
        input.f2_points,
        input.cartesian_fields,
        accelerated,
    )
}
pub(super) fn memory_shape(
    n: usize,
    m: usize,
    f2: usize,
    c: usize,
    accelerated: bool,
) -> Result<MemoryEstimate, IstError> {
    let y_bytes = m
        .checked_mul(f2)
        .and_then(|v| v.checked_mul(c))
        .ok_or(IstError::SizeOverflow)?
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(IstError::SizeOverflow)?;
    let dense_complex = n
        .checked_mul(f2)
        .and_then(|value| value.checked_mul(c))
        .ok_or(IstError::SizeOverflow)?;
    let dense_bytes = dense_complex
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(IstError::SizeOverflow)?;
    let output_bytes = dense_complex
        .checked_mul(2)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(IstError::SizeOverflow)?;
    let fft = crate::processing::kernels::fft::bounds(n).map_err(fft_error)?;
    let plans = fft
        .planning
        .max(fft.retained)
        .checked_mul(2)
        .ok_or(IstError::SizeOverflow)?;
    let line_bytes = n
        .checked_mul(c)
        .and_then(|value| value.checked_mul(std::mem::size_of::<Complex64>()))
        .ok_or(IstError::SizeOverflow)?;
    let column_bytes = f2
        .checked_mul(std::mem::size_of::<ColumnState>())
        .ok_or(IstError::SizeOverflow)?;
    let iteration_peak = checked_sum(&[
        column_bytes,
        y_bytes,
        dense_bytes,
        dense_bytes,
        if accelerated { dense_bytes } else { 0 }, // previous non-extrapolated iterate
        n,
        line_bytes,
        plans,
        fft.scratch,
    ])?;
    let scatter_peak = checked_sum(&[column_bytes, y_bytes, dense_bytes, output_bytes])?;
    Ok(MemoryEstimate {
        output_bytes,
        peak_working_bytes: iteration_peak.max(scatter_peak),
    })
}

pub(super) fn estimate_work(
    input: &IstInput,
    iterations: usize,
    accelerated: bool,
) -> Result<u128, IstError> {
    work_shape(
        input.nlogical,
        input.measured_indices.len(),
        input.f2_points,
        input.cartesian_fields,
        iterations,
        accelerated,
    )
}
pub(crate) fn shape_resources(
    n: usize,
    m: usize,
    f2: usize,
    c: usize,
    iterations: usize,
) -> Result<(crate::resource::ResourceEstimate, u128), IstError> {
    let memory = memory_shape(n, m, f2, c, true)?;
    Ok((
        crate::resource::ResourceEstimate::new(memory.output_bytes, 0, memory.peak_working_bytes),
        work_shape(n, m, f2, c, iterations, true)?,
    ))
}
pub(super) fn work_shape(
    nlogical: usize,
    measured: usize,
    f2_points: usize,
    fields: usize,
    iterations: usize,
    accelerated: bool,
) -> Result<u128, IstError> {
    let n = nlogical as u128;
    let m = measured as u128;
    let f2 = f2_points as u128;
    let c = fields as u128;
    let log_n = ceil_log2(nlogical) as u128;
    let gather = checked_product_u128(&[m, f2, c, 2])?;
    let grid = checked_product_u128(&[n, f2, c])?;
    let initial_fft = checked_product_u128(&[f2, c, n, log_n])?;
    let per_iteration_fft = checked_product_u128(&[2, f2, c, n, log_n])?;
    let per_iteration_grid = checked_product_u128(&[if accelerated { 24 } else { 8 }, n, f2, c])?;
    let column_work = checked_product_u128(&[12, f2])?;
    let iteration = per_iteration_fft
        .checked_add(column_work)
        .ok_or(IstError::SizeOverflow)?
        .checked_add(per_iteration_grid)
        .ok_or(IstError::SizeOverflow)?
        .checked_mul(iterations as u128)
        .ok_or(IstError::SizeOverflow)?;
    let scatter = grid;
    [
        gather,
        grid,
        if accelerated { grid } else { 0 },
        initial_fft,
        iteration,
        scatter,
    ]
    .into_iter()
    .try_fold(0u128, |total, value| total.checked_add(value))
    .ok_or(IstError::SizeOverflow)
}

pub(super) fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

pub(super) fn checked_product_u128(values: &[u128]) -> Result<u128, IstError> {
    values.iter().try_fold(1u128, |total, value| {
        total.checked_mul(*value).ok_or(IstError::SizeOverflow)
    })
}

pub(super) fn checked_sum(values: &[usize]) -> Result<usize, IstError> {
    values.iter().try_fold(0usize, |total, value| {
        total.checked_add(*value).ok_or(IstError::SizeOverflow)
    })
}

pub(super) fn try_complex_vec(length: usize) -> Result<Vec<Complex64>, IstError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| IstError::AllocationFailure)?;
    values.resize(length, Complex64::new(0.0, 0.0));
    Ok(values)
}

pub(super) fn try_f64_vec(length: usize) -> Result<Vec<f64>, IstError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| IstError::AllocationFailure)?;
    values.resize(length, 0.0);
    Ok(values)
}

pub(super) fn try_u8_vec(length: usize) -> Result<Vec<u8>, IstError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| IstError::AllocationFailure)?;
    values.resize(length, 0);
    Ok(values)
}
