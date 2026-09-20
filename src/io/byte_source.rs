use crate::ExecutionContext;
use crate::ReadError;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

/// Checked positional access to one acquisition payload.
pub(crate) struct ByteSource {
    file: Mutex<fs::File>,
    path: PathBuf,
    length: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(test)]
    bytes_read: AtomicU64,
    #[cfg(test)]
    read_calls: AtomicU64,
}

impl ByteSource {
    #[cfg(test)]
    pub(crate) fn read_exact_at(&self, offset: usize, length: usize) -> Result<Vec<u8>, ReadError> {
        self.read_exact_at_controlled(&mut ExecutionContext::default(), offset, length)
    }

    #[cfg(test)]
    pub(crate) fn verify_range(
        &self,
        offset: usize,
        expected: &[u8],
        working_limit: usize,
    ) -> Result<(), ReadError> {
        self.verify_range_controlled(
            &mut ExecutionContext::default(),
            offset,
            expected,
            working_limit,
        )
    }

    #[cfg(test)]
    pub(crate) fn snapshot(
        &self,
        working_limit: usize,
    ) -> Result<crate::provenance::SourceDigest, ReadError> {
        self.snapshot_controlled(&mut ExecutionContext::default(), working_limit)
    }

    pub(crate) fn open(path: PathBuf) -> Result<Self, ReadError> {
        let file = fs::File::open(&path).map_err(|error| ReadError::io(path.clone(), error))?;
        let metadata = file
            .metadata()
            .map_err(|error| ReadError::io(path.clone(), error))?;
        Ok(Self {
            file: Mutex::new(file),
            path,
            length: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(test)]
            bytes_read: AtomicU64::new(0),
            #[cfg(test)]
            read_calls: AtomicU64::new(0),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn length(&self) -> Result<usize, ReadError> {
        usize::try_from(self.length).map_err(|_| ReadError::SizeOverflow)
    }

    /// Copies and hashes one immutable private materialization view. No original
    /// path is reopened, and the copy buffer is released before trace decoding.
    pub(crate) fn snapshot_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        working_limit: usize,
    ) -> Result<crate::provenance::SourceDigest, ReadError> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| ReadError::source_changed(self.path.clone().into()))?;
        let verify = |file: &fs::File| -> Result<(), ReadError> {
            let metadata = file
                .metadata()
                .map_err(|error| ReadError::io(self.path.clone(), error))?;
            if metadata.len() != self.length || metadata.modified().ok() != self.modified {
                return Err(ReadError::source_changed(self.path.clone().into()));
            }
            Ok(())
        };
        verify(&file)?;
        if working_limit == 0 {
            return Err(ReadError::limit(crate::ReadResource::WorkingBytes, 0, 1));
        }
        let capacity = self.length()?.min(64 * 1024).min(working_limit).max(1);
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(capacity)
            .map_err(|_| ReadError::allocation(capacity))?;
        buffer.resize(capacity, 0);
        let mut snapshot =
            tempfile::tempfile().map_err(|error| ReadError::io(self.path.clone(), error))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| ReadError::io(self.path.clone(), error))?;
        let mut remaining = self.length;
        let mut hash = Sha256::new();
        while remaining != 0 {
            control.check_cancelled()?;
            let count = usize::try_from(remaining.min(capacity as u64))
                .map_err(|_| ReadError::SizeOverflow)?;
            file.read_exact(&mut buffer[..count])
                .map_err(|error| ReadError::io(self.path.clone(), error))?;
            snapshot
                .write_all(&buffer[..count])
                .map_err(|error| ReadError::io(self.path.clone(), error))?;
            control.io(count * 2)?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        verify(&file)?;
        snapshot
            .seek(SeekFrom::Start(0))
            .map_err(|error| ReadError::io(self.path.clone(), error))?;
        control.check_cancelled()?;
        *file = snapshot;
        Ok(crate::provenance::SourceDigest::Sha256(
            hash.finalize().into(),
        ))
    }

    pub(crate) fn verify_range_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        offset: usize,
        expected: &[u8],
        working_limit: usize,
    ) -> Result<(), ReadError> {
        let capacity = expected.len().min(64 * 1024).min(working_limit);
        if expected.is_empty() {
            return Ok(());
        }
        if capacity == 0 {
            return Err(ReadError::limit(crate::ReadResource::WorkingBytes, 0, 1));
        }
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(capacity)
            .map_err(|_| ReadError::allocation(capacity))?;
        buffer.resize(capacity, 0);
        for (index, chunk) in expected.chunks(capacity).enumerate() {
            let position = index
                .checked_mul(capacity)
                .and_then(|value| offset.checked_add(value))
                .ok_or(ReadError::SizeOverflow)?;
            self.read_exact_at_into_controlled(control, position, &mut buffer[..chunk.len()])?;
            if &buffer[..chunk.len()] != chunk {
                return Err(ReadError::source_changed(self.path.clone().into()));
            }
        }
        Ok(())
    }

    pub(crate) fn read_exact_at_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        offset: usize,
        length: usize,
    ) -> Result<Vec<u8>, ReadError> {
        control.check_cancelled()?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ReadError::allocation(length))?;
        bytes.resize(length, 0);
        self.read_exact_at_into_controlled(control, offset, &mut bytes)?;
        Ok(bytes)
    }

    pub(crate) fn read_exact_at_into_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), ReadError> {
        let mut file = self.file.lock().map_err(|_| {
            ReadError::io(
                self.path.clone(),
                std::io::Error::other("acquisition source lock was poisoned"),
            )
        })?;
        file.seek(SeekFrom::Start(
            u64::try_from(offset).map_err(|_| ReadError::SizeOverflow)?,
        ))
        .map_err(|error| ReadError::io(self.path.clone(), error))?;
        for block in bytes.chunks_mut(32768) {
            control.check_cancelled()?;
            file.read_exact(block)
                .map_err(|error| ReadError::io(self.path.clone(), error))?;
            control.io(block.len())?;
        }
        #[cfg(test)]
        self.bytes_read
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        #[cfg(test)]
        self.read_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn read_calls(&self) -> u64 {
        self.read_calls.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn bytes_read(&self) -> u64 {
        self.bytes_read.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_digest_and_reads_share_the_private_copy() {
        let original = tempfile::NamedTempFile::new().unwrap();
        let bytes = b"header:0123456789:trailer";
        fs::write(original.path(), bytes).unwrap();
        let source = ByteSource::open(original.path().to_path_buf()).unwrap();
        let digest = source.snapshot(3).unwrap();
        fs::write(original.path(), b"replaced original bytes").unwrap();
        assert_eq!(
            digest,
            crate::provenance::SourceDigest::Sha256(Sha256::digest(bytes).into())
        );
        assert_eq!(source.read_exact_at(0, bytes.len()).unwrap(), bytes);
        source.verify_range(0, b"header", 2).unwrap();
        assert_eq!(
            source.verify_range(0, b"HEADER", 2).unwrap_err().kind(),
            crate::raw::ReadErrorKind::SourceChanged
        );
    }

    #[test]
    fn snapshot_rejects_zero_working_budget_and_changed_file() {
        let original = tempfile::NamedTempFile::new().unwrap();
        fs::write(original.path(), b"1234").unwrap();
        let source = ByteSource::open(original.path().to_path_buf()).unwrap();
        assert!(matches!(
            source.snapshot(0).unwrap_err().reason(),
            crate::raw::ReadErrorReason::LimitExceeded {
                resource: crate::ReadResource::WorkingBytes,
                limit: 0,
                required: 1
            }
        ));
        fs::write(original.path(), b"12345").unwrap();
        assert_eq!(
            source.snapshot(3).unwrap_err().kind(),
            crate::raw::ReadErrorKind::SourceChanged
        );
    }
}
