//! Reader-owned storage and normalization budget for explicit sampling.

use crate::{ReadError, ReadLimits, ReadResource, SamplingDeclaration};

fn storage_bytes(declaration: &SamplingDeclaration) -> Option<usize> {
    let words = declaration.indices().iter().try_fold(
        declaration
            .grid()
            .len()
            .checked_add(declaration.indirect_lanes().len())?,
        |sum, row| sum.checked_add(row.len()),
    )?;
    words
        .checked_mul(std::mem::size_of::<usize>())?
        .checked_add(
            declaration
                .indices()
                .len()
                .checked_mul(std::mem::size_of::<Vec<usize>>())?,
        )?
        .checked_add(declaration.source().len())?
        .checked_add(declaration.assertion().as_str().len())?
        .checked_add(std::mem::size_of::<SamplingDeclaration>() + 4 * std::mem::size_of::<usize>())
}
pub(super) fn limits(
    declaration: &SamplingDeclaration,
    limits: ReadLimits,
) -> Result<ReadLimits, ReadError> {
    let bytes = storage_bytes(declaration).ok_or(ReadError::SizeOverflow)?;
    if bytes > limits.metadata_bytes() {
        return Err(ReadError::limit(
            ReadResource::MetadataBytes,
            limits.metadata_bytes(),
            bytes,
        ));
    }
    // Also reserves normalization rows and duplicate-check indexes before allocation.
    let working = bytes.checked_mul(3).ok_or(ReadError::SizeOverflow)?;
    if working > limits.working_bytes() {
        return Err(ReadError::limit(
            ReadResource::WorkingBytes,
            limits.working_bytes(),
            working,
        ));
    }
    // Reduce the adapter's available budget so its Reader retains this charge
    // throughout materialization, including adapters without retained counters.
    Ok(limits
        .max_working_bytes(limits.working_bytes() - working)
        .max_metadata_bytes(limits.metadata_bytes() - bytes))
}
