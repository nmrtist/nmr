use crate::processing::contracts::error::ProcessingError;
use crate::{Complex64, ExecutionContext};

pub(in crate::processing) fn charge_block(
    control: &mut ExecutionContext<'_>,
    index: usize,
    total: usize,
) -> Result<(), ProcessingError> {
    if index % 4096 == 0 {
        control.charge((total - index).min(4096) as u128)?;
    }
    Ok(())
}

pub(in crate::processing) fn clone_samples(
    samples: &[f64],
    control: &mut ExecutionContext<'_>,
) -> Result<Vec<f64>, ProcessingError> {
    control.check_cancelled()?;
    let mut output = try_vec_capacity(samples.len())?;
    for block in samples.chunks(4096) {
        control.check_cancelled()?;
        output.extend_from_slice(block);
    }
    Ok(output)
}

pub(in crate::processing) fn try_vec_capacity<T>(
    capacity: usize,
) -> Result<Vec<T>, ProcessingError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ProcessingError::AllocationFailure)?;
    Ok(values)
}

pub(in crate::processing) fn try_zeroed(length: usize) -> Result<Vec<f64>, ProcessingError> {
    let mut values = try_vec_capacity(length)?;
    values.resize(length, 0.0);
    Ok(values)
}

pub(in crate::processing) fn try_zeroed_usize(
    length: usize,
) -> Result<Vec<usize>, ProcessingError> {
    let mut values = try_vec_capacity(length)?;
    values.resize(length, 0);
    Ok(values)
}

pub(in crate::processing) fn try_zeroed_complex(
    length: usize,
) -> Result<Vec<Complex64>, ProcessingError> {
    let mut values = try_vec_capacity(length)?;
    values.resize(length, Complex64::new(0.0, 0.0));
    Ok(values)
}
