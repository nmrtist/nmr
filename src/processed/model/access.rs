use super::*;

pub(super) fn validate_rank(rank: usize) -> Result<(), ProcessedValidationError> {
    if rank == 0 {
        return Err(ProcessedValidationError::EmptyAxes);
    }
    Ok(())
}

pub(super) fn storage_shape(
    shape: &[usize],
    component_counts: &[usize],
) -> Result<Vec<usize>, ProcessedValidationError> {
    if shape.len() != component_counts.len() {
        return Err(ProcessedValidationError::ComponentRankMismatch);
    }
    shape
        .iter()
        .zip(component_counts)
        .map(|(&points, &components)| {
            points
                .checked_mul(components)
                .ok_or(ProcessedValidationError::SizeOverflow)
        })
        .collect()
}

pub(super) fn checked_product_access(values: &[usize]) -> Result<usize, ProcessedAccessError> {
    values.iter().try_fold(1usize, |total, &value| {
        total
            .checked_mul(value)
            .ok_or(ProcessedAccessError::SizeOverflow)
    })
}

pub(super) fn validate_components(
    component_counts: &[usize],
    components: &[usize],
) -> Result<(), ProcessedAccessError> {
    if components.len() != component_counts.len() {
        return Err(ProcessedAccessError::RankMismatch {
            expected: component_counts.len(),
            logical: component_counts.len(),
            components: components.len(),
        });
    }
    for (axis, (&component, &count)) in components.iter().zip(component_counts).enumerate() {
        if component >= count {
            return Err(ProcessedAccessError::ComponentOutOfBounds {
                axis,
                component,
                count,
            });
        }
    }
    Ok(())
}
