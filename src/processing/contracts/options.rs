use crate::resource::WorkLedger;

pub(in crate::processing) const DEFAULT_LIMIT: usize = 512 * 1024 * 1024;

/// Per-call resource controls for processing.
/// Working bytes include output, new metadata and scratch, excluding the caller's
/// resident input. The component-transform work limit applies to that kernel;
/// automatic analysis separately accepts a caller-managed [`WorkLedger`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessingOptions {
    pub(in crate::processing) max_output_bytes: usize,
    pub(in crate::processing) max_working_bytes: usize,
    pub(in crate::processing) max_metadata_bytes: usize,
    pub(in crate::processing) max_transform_work: u128,
    // Internal reservation for the prepared vectors retained during execution.
    pub(in crate::processing) prepared_bytes: usize,
    pub(in crate::processing) axis_bytes: usize,
    pub(in crate::processing) state_bytes: usize,
    pub(in crate::processing) container_bytes: usize,
    pub(in crate::processing) context_bytes: usize,
}

impl ProcessingOptions {
    /// Uses shared output, metadata and total working payload limits.
    pub fn memory(mut self, limits: crate::resource::MemoryLimits) -> Self {
        self.max_output_bytes = limits.output_bytes();
        self.max_working_bytes = limits.working_bytes();
        self.max_metadata_bytes = limits.metadata_bytes();
        self
    }

    /// Sets the maximum newly allocated processing metadata payload.
    pub fn max_metadata_bytes(mut self, bytes: usize) -> Self {
        self.max_metadata_bytes = bytes;
        self
    }

    /// Returns the metadata payload limit.
    pub fn metadata_bytes(self) -> usize {
        self.max_metadata_bytes
    }
    /// Creates default 512 MiB output and working limits.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum final `ProcessedData` sample bytes.
    pub fn max_output_bytes(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Sets the maximum numerical buffers, prepared vectors, state arrays and new axis storage.
    /// Includes copied raw snapshots, provenance, history and aggregate context.
    /// Also charges conservative temporary descriptor/index array bounds.
    pub fn max_working_bytes(mut self, bytes: usize) -> Self {
        self.max_working_bytes = bytes;
        self
    }

    /// Sets the maximum component-transform multiply-accumulate work.
    pub fn max_transform_work(mut self, units: u128) -> Self {
        self.max_transform_work = units;
        self
    }

    /// Returns the final output limit.
    pub fn output_bytes(self) -> usize {
        self.max_output_bytes
    }

    /// Returns the working-set limit.
    pub fn working_bytes(self) -> usize {
        self.max_working_bytes
    }

    /// Returns the component-transform work limit.
    pub fn transform_work(self) -> u128 {
        self.max_transform_work
    }
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            max_output_bytes: DEFAULT_LIMIT,
            max_working_bytes: DEFAULT_LIMIT,
            max_metadata_bytes: DEFAULT_LIMIT,
            max_transform_work: WorkLedger::DEFAULT_PROCESSING_LIMIT,
            prepared_bytes: 0,
            axis_bytes: 0,
            state_bytes: 0,
            container_bytes: 0,
            context_bytes: 0,
        }
    }
}
