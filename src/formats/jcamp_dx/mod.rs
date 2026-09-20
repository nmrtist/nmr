use crate::ReadError;
use crate::processed::ProcessedDataset;

/// Borrowed bytes of one complete JCAMP-DX file.
#[derive(Clone, Copy, Debug)]
pub struct Parts<'a> {
    bytes: &'a [u8],
}

impl<'a> Parts<'a> {
    /// Creates in-memory JCAMP-DX parts.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

/// Decodes a checked in-memory JCAMP-DX NMR spectrum.
pub fn read_parts(parts: Parts<'_>) -> Result<ProcessedDataset, ReadError> {
    crate::formats::jcamp_dx::decoding::read_parts(parts.bytes)
}

/// Decodes borrowed bytes with explicit source, metadata and numerical limits.
/// The borrowed source buffer is not charged as newly owned working memory.
pub fn read_parts_with_limits(
    parts: Parts<'_>,
    limits: crate::ReadLimits,
) -> Result<ProcessedDataset, ReadError> {
    crate::formats::jcamp_dx::decoding::read_parts_with_limits(parts.bytes, limits)
}

pub(crate) mod decoding;
