//! Validated lazy reader and access to normalized acquisitions.

use crate::raw::{RawDescriptor, RawProvenance, SamplingSchedule};
pub(crate) use source::TraceSource;

/// An opened and validated raw acquisition.
///
/// The concrete vendor adapter, file handles, and byte layout remain private.
/// Reads are expressed only in normalized logical acquisition coordinates.
pub struct Reader {
    descriptor: RawDescriptor,
    provenance: RawProvenance,
    sampling: Option<SamplingSchedule>,
    source: Box<dyn TraceSource>,
    max_region_bytes: usize,
    max_materialized_bytes: usize,
    max_working_bytes: usize,
    retained_bytes: usize,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Reader")
            .field("descriptor", &self.descriptor)
            .field("sampling_schedule", &self.sampling)
            .finish_non_exhaustive()
    }
}

mod access;
mod materialize;
mod region;
mod source;
mod storage;
mod trace;

#[cfg(test)]
mod tests;
