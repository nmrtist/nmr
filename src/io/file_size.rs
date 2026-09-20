//! Path metadata checks owned by the input layer.

use crate::read_error::{ReadError, ReadResource};
use std::path::Path;

pub(crate) fn ensure_file_size(
    path: &Path,
    limit: usize,
    resource: ReadResource,
) -> Result<usize, ReadError> {
    let metadata = std::fs::metadata(path).map_err(|error| ReadError::io(path, error))?;
    if metadata.len() > limit as u64 {
        let required = usize::try_from(metadata.len()).map_err(|_| ReadError::SizeOverflow)?;
        return Err(ReadError::limit(resource, limit, required));
    }
    usize::try_from(metadata.len()).map_err(|_| ReadError::SizeOverflow)
}
