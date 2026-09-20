use crate::ReadError;

/// Reads exactly the already-budgeted size without growing if the file changes.
pub(crate) fn read_sized_file(path: &std::path::Path, size: usize) -> Result<Vec<u8>, ReadError> {
    read_sized_file_controlled(&mut crate::ExecutionContext::default(), path, size)
}

pub(crate) fn read_sized_file_controlled(
    control: &mut crate::ExecutionContext<'_>,
    path: &std::path::Path,
    size: usize,
) -> Result<Vec<u8>, ReadError> {
    use std::io::Read;
    control.check_cancelled()?;
    let mut file = std::fs::File::open(path).map_err(|error| ReadError::io(path, error))?;
    let actual = usize::try_from(
        file.metadata()
            .map_err(|error| ReadError::io(path, error))?
            .len(),
    )
    .map_err(|_| ReadError::SizeOverflow)?;
    if actual < size {
        return Err(ReadError::truncated(path.into(), size, actual));
    }
    if actual > size {
        return Err(ReadError::corrupt(
            path.into(),
            "bytes remain beyond the budgeted source size",
        ));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| ReadError::allocation(size))?;
    bytes.resize(size, 0);
    let mut read = 0;
    while read < size {
        control.check_cancelled()?;
        let end = (read + 32768).min(size);
        match file.read(&mut bytes[read..end]) {
            Ok(0) => return Err(ReadError::truncated(path.into(), size, read)),
            Ok(count) => {
                read += count;
                control.io(count)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ReadError::io(path, error)),
        }
    }
    let mut extra = [0];
    if file
        .read(&mut extra)
        .map_err(|error| ReadError::io(path, error))?
        != 0
    {
        return Err(ReadError::corrupt(
            path.into(),
            "bytes remain beyond the budgeted source size",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::read_sized_file;
    use crate::ReadErrorKind;

    #[test]
    fn sized_read_rejects_length_changes_without_allocating_declared_capacity() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), [1, 2, 3]).unwrap();
        assert_eq!(read_sized_file(file.path(), 3).unwrap(), [1, 2, 3]);
        assert_eq!(
            read_sized_file(file.path(), 2).unwrap_err().kind(),
            ReadErrorKind::Corrupt
        );
        assert_eq!(
            read_sized_file(file.path(), 4).unwrap_err().kind(),
            ReadErrorKind::Truncated
        );
        assert_eq!(
            read_sized_file(file.path(), usize::MAX).unwrap_err().kind(),
            ReadErrorKind::Truncated
        );
    }
}
