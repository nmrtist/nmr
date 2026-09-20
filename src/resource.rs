//! Payload-capacity resource controls, not process RSS or allocator guarantees.
//!
//! Explicit processing, bounded automatic-phase application, plot preparation and
//! Dataset region copies share output/metadata/total-working capacity semantics.
//! Low-level readers retain stream-oriented `ReadLimits`; their temporary working
//! budget can exclude final materialized output, which has its own limit. Standalone
//! mathematical kernels document separate controls and do not inherit a session
//! budget from a previously prepared or read dataset.

/// One independently limited part of a library-controlled working set.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    /// Returned numeric sample storage.
    OutputBytes,
    /// Newly allocated descriptions, history, provenance, indexes and related metadata.
    MetadataBytes,
    /// All simultaneously live library-controlled payload, including output and metadata.
    WorkingBytes,
}

/// A capacity bound exceeds the configured limit before its allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{resource:?} requires {required} bytes; limit is {limit}")]
pub struct LimitExceeded {
    /// Resource whose bound exceeded the limit.
    pub resource: ResourceKind,
    /// Configured maximum bytes.
    pub limit: usize,
    /// Conservatively required bytes at the failed check.
    pub required: usize,
}

/// Independent sample, metadata and total working payload limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryLimits {
    output: usize,
    metadata: usize,
    working: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            output: 512 * 1024 * 1024,
            metadata: 512 * 1024 * 1024,
            working: 512 * 1024 * 1024,
        }
    }
}

impl MemoryLimits {
    /// Creates finite 512 MiB limits for each resource.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets returned numeric sample bytes.
    pub fn max_output_bytes(mut self, bytes: usize) -> Self {
        self.output = bytes;
        self
    }
    /// Sets new metadata payload bytes.
    pub fn max_metadata_bytes(mut self, bytes: usize) -> Self {
        self.metadata = bytes;
        self
    }
    /// Sets total working payload bytes; output and metadata are included.
    pub fn max_working_bytes(mut self, bytes: usize) -> Self {
        self.working = bytes;
        self
    }
    /// Returns the sample limit.
    pub fn output_bytes(self) -> usize {
        self.output
    }
    /// Returns the metadata limit.
    pub fn metadata_bytes(self) -> usize {
        self.metadata
    }
    /// Returns the total working limit.
    pub fn working_bytes(self) -> usize {
        self.working
    }
}

/// Conservative capacity bounds for prepared processing or a Dataset region copy.
/// Input storage already held by the caller is excluded. Output and metadata
/// are subsets of working bytes, not additional charges to add to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceEstimate {
    output: usize,
    metadata: usize,
    working: usize,
}

impl ResourceEstimate {
    pub(crate) fn new(output: usize, metadata: usize, working: usize) -> Self {
        Self {
            output,
            metadata,
            working,
        }
    }
    /// Returns final sample capacity in bytes.
    pub fn output_bytes(self) -> usize {
        self.output
    }
    /// Returns the conservative metadata capacity bound.
    pub fn metadata_bytes(self) -> usize {
        self.metadata
    }
    /// Returns the complete conservative working capacity bound.
    pub fn working_bytes(self) -> usize {
        self.working
    }
}

/// Deterministic upper bound and consumption counter for numerical work.
/// A ledger is a caller-managed counter, not a global budget session. Explicit clones
/// have independent consumption; reuse the same mutable ledger to accumulate work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkLedger {
    limit: u128,
    used: u128,
}

impl WorkLedger {
    /// Default total processing work limit covering reconstruction and other steps.
    pub const DEFAULT_PROCESSING_LIMIT: u128 = 40_000_000_000;

    /// Creates an empty ledger with the given work-unit limit.
    pub fn new(limit: u128) -> Self {
        Self { limit, used: 0 }
    }

    /// Creates an empty ledger with the default total processing limit.
    pub fn processing_default() -> Self {
        Self::new(Self::DEFAULT_PROCESSING_LIMIT)
    }

    /// Returns charged work units.
    pub fn used(&self) -> u128 {
        self.used
    }

    /// Returns the configured work-unit limit.
    pub fn limit(&self) -> u128 {
        self.limit
    }

    pub(crate) fn charge(&mut self, units: u128) -> Result<(), ResourceError> {
        let used = self
            .used
            .checked_add(units)
            .ok_or(ResourceError::SizeOverflow)?;
        if used > self.limit {
            return Err(ResourceError::WorkLimit);
        }
        self.used = used;
        Ok(())
    }
}

/// Failures in pure resource arithmetic or work charging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceError {
    SizeOverflow,
    WorkLimit,
}
