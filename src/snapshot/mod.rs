//! Versioned embedded snapshots with exact binary samples and explicit history acceptance.
//! Paths are recorded locators and are never accessed by restoration. The host
//! owns compression, file transactions, and storage of earlier replay inputs.

mod model;
mod paths;
mod registered;
mod stream;
pub(crate) mod wire;
use crate::{
    Dataset, ExecutionContext,
    execution::{ExecutionError, ExecutionStage},
};
use std::io::{Read, Write};
use wire::{Budget, Codec};

/// Current independently versioned snapshot interpretation.
pub const SNAPSHOT_VERSION: u32 = 1;

/// Limits checked before allocating transport and restored model payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotLimits {
    /// Entire frame including header and integrity trailer.
    pub max_bytes: u64,
    /// All binary numeric arrays, including exact coordinate arrays.
    pub max_sample_bytes: usize,
    /// Transport tree and restored metadata payload allocations.
    pub max_metadata_bytes: usize,
    /// Combined numeric and metadata payload allocations; excludes host input.
    pub max_working_bytes: usize,
    /// Maximum transport nesting, also bounding external ancestry depth (hard cap 256).
    pub max_depth: usize,
}
impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_bytes: 1024 * 1024 * 1024,
            max_sample_bytes: 512 * 1024 * 1024,
            max_metadata_bytes: 128 * 1024 * 1024,
            max_working_bytes: 640 * 1024 * 1024,
            max_depth: 128,
        }
    }
}

/// Failure to write, validate or restore a snapshot.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// Cancellation or a shared execution resource failed.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// Stream access failed; writers may retain a prefix.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The frame is incomplete.
    #[error("truncated snapshot")]
    Truncated,
    /// Integrity checksum or final commit marker differs.
    #[error("snapshot integrity check failed")]
    Integrity,
    /// A length, depth or payload bound was exceeded.
    #[error("snapshot resource limit exceeded")]
    ResourceLimit,
    /// Size conversion or arithmetic overflowed.
    #[error("snapshot size overflow")]
    SizeOverflow,
    /// A bounded allocation failed.
    #[error("snapshot allocation failed")]
    Allocation,
    /// Wire tags, model structure or references disagree.
    #[error("invalid snapshot structure")]
    Structure,
    /// Snapshot version is unsupported.
    #[error("unsupported snapshot version {0}")]
    UnsupportedVersion(u32),
    /// Algorithm or history rule is unsupported.
    #[error("unsupported snapshot history version {0}")]
    UnsupportedHistoryVersion(String),
    /// Archived scientific values failed model validation.
    #[error("invalid snapshot model: {0}")]
    Validation(String),
    /// Recomputed current scientific identity differs from the archive.
    #[error("snapshot scientific digest mismatch")]
    DigestMismatch,
    /// A platform-specific path cannot be restored losslessly on this platform.
    #[error("snapshot path encoding is unsupported on this platform")]
    PathEncoding,
}

impl SnapshotError {
    fn sampling_declaration(
        error: crate::raw::model::sampling_declaration::SamplingDeclarationError,
    ) -> Self {
        match error {
            crate::raw::model::sampling_declaration::SamplingDeclarationError::Execution(error) => {
                Self::Execution(error)
            }
            other => Self::Validation(crate::ReadError::from(other).to_string()),
        }
    }
}

/// Explicit host acceptance of recorded scientific history, without claiming re-execution.
#[derive(Clone, Copy, Debug)]
pub struct AcceptRecordedHistory;

/// Decodes exactly one complete snapshot buffer, rejecting bytes after the frame.
/// Use [`read_snapshot`] for a framed stream embedded in a larger container.
pub fn decode_snapshot(
    bytes: &[u8],
    limits: SnapshotLimits,
) -> Result<CheckedSnapshot, SnapshotError> {
    decode_snapshot_with_context(bytes, limits, &mut ExecutionContext::default())
}
/// Decodes a complete buffer with the caller's shared context and strict exhaustion.
pub fn decode_snapshot_with_context(
    bytes: &[u8],
    limits: SnapshotLimits,
    control: &mut ExecutionContext<'_>,
) -> Result<CheckedSnapshot, SnapshotError> {
    let mut remainder = bytes;
    let checked = read_snapshot_with_context(&mut remainder, limits, control)?;
    if !remainder.is_empty() {
        return Err(SnapshotError::Structure);
    }
    Ok(checked)
}

/// Structurally and scientifically checked archive awaiting a host trust decision.
#[derive(Debug)]
pub struct CheckedSnapshot {
    dataset: Dataset,
}
impl CheckedSnapshot {
    /// Checked current scientific identity. Ancestor samples were not recomputed.
    pub fn canonical_digests(&self) -> crate::provenance::CanonicalDatasetDigests {
        self.dataset.canonical_digests()
    }
    /// Inspects the archived description without granting a processing input.
    pub fn descriptor(&self) -> crate::dataset::DescriptorRef<'_> {
        self.dataset.descriptor()
    }
    /// Inspects retained reading metadata before accepting the history.
    pub fn metadata(&self) -> &crate::dataset::DatasetMetadata {
        self.dataset.metadata()
    }
    /// Accepts archived history and restores a processing input. No execution fact is added.
    pub fn restore(mut self, _accept: AcceptRecordedHistory) -> Dataset {
        self.dataset.accept_archived_history();
        self.dataset
    }
}

/// Writes one complete frame. A generic writer may retain a prefix on failure.
pub fn write_snapshot(
    dataset: &Dataset,
    writer: &mut impl Write,
    limits: SnapshotLimits,
) -> Result<(), SnapshotError> {
    write_snapshot_with_context(dataset, writer, limits, &mut ExecutionContext::default())
}
/// Writes a frame with cooperative cancellation between binary chunks.
pub fn write_snapshot_with_context(
    dataset: &Dataset,
    writer: &mut impl Write,
    limits: SnapshotLimits,
    control: &mut ExecutionContext<'_>,
) -> Result<(), SnapshotError> {
    control.begin(ExecutionStage::Snapshot, None, None)?;
    let mut ancestry = dataset
        .as_processed()
        .map(|p| (p.provenance().origin(), 0usize))
        .into_iter()
        .collect::<Vec<_>>();
    while let Some((origin, depth)) = ancestry.pop() {
        control.check_cancelled()?;
        if depth > limits.max_depth.min(256) / 8 {
            return Err(SnapshotError::ResourceLimit);
        }
        match origin {
            crate::processed::ProcessedOrigin::External(boundary) => {
                ancestry.push((boundary.parent().provenance().origin(), depth + 1))
            }
            crate::processed::ProcessedOrigin::Library(boundary) => {
                for input in boundary.inputs() {
                    if let crate::derivation::DerivationInput::Processed(parent) = input {
                        ancestry.push((parent.provenance().origin(), depth + 1));
                    }
                }
            }
            _ => {}
        }
    }
    let mut budget = Budget::new(control, limits);
    budget.reserve::<u8>(4096 * 16)?;
    let value = dataset.to_wire(&mut budget)?;
    let length = stream::size(&value, budget.control, 0, limits.max_depth)?;
    if length.checked_add(60).is_none_or(|n| n > limits.max_bytes) {
        return Err(SnapshotError::ResourceLimit);
    }
    stream::write(writer, &value, length, budget.control)
}
/// Reads one framed snapshot; subsequent host stream bytes remain unread.
pub fn read_snapshot(
    reader: &mut impl Read,
    limits: SnapshotLimits,
) -> Result<CheckedSnapshot, SnapshotError> {
    read_snapshot_with_context(reader, limits, &mut ExecutionContext::default())
}
/// Reads, bounds, checks integrity and validates the current scientific model.
pub fn read_snapshot_with_context(
    reader: &mut impl Read,
    limits: SnapshotLimits,
    control: &mut ExecutionContext<'_>,
) -> Result<CheckedSnapshot, SnapshotError> {
    control.begin(ExecutionStage::Snapshot, None, None)?;
    let mut budget = Budget::new(control, limits);
    budget.reserve::<u8>(4096 * 16)?;
    let value = stream::read(reader, &mut budget)?;
    let dataset = Dataset::from_wire(value, &mut budget)?;
    dataset
        .validate_recorded(budget.control)
        .map_err(SnapshotError::model)?;
    Ok(CheckedSnapshot { dataset })
}

impl SnapshotError {
    fn model(error: crate::internal::ModelError) -> Self {
        use crate::internal::ModelError as M;
        match error {
            M::Structure => Self::Structure,
            M::Validation(message) => Self::Validation(message),
            M::UnsupportedHistoryVersion(version) => Self::UnsupportedHistoryVersion(version),
            M::DigestMismatch => Self::DigestMismatch,
            M::Execution(error) => Self::Execution(error),
        }
    }
}
