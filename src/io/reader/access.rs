use crate::ReadError;
use crate::raw::RawDescriptor;
use crate::raw::RawProvenance;
use crate::raw::SamplingSchedule;

use super::{Reader, TraceSource};

impl Reader {
    pub(crate) fn new(
        descriptor: RawDescriptor,
        provenance: RawProvenance,
        sampling: Option<SamplingSchedule>,
        source: Box<dyn TraceSource>,
        max_region_bytes: usize,
        max_materialized_bytes: usize,
        max_working_bytes: usize,
    ) -> Self {
        Self {
            descriptor,
            provenance,
            sampling,
            source,
            max_region_bytes,
            max_materialized_bytes,
            max_working_bytes,
            retained_bytes: 0,
        }
    }

    pub(crate) fn with_retained_bytes(mut self, bytes: usize) -> Result<Self, ReadError> {
        self.retained_bytes = bytes;
        self.check_numeric_working(0, 0, 0)?;
        Ok(self)
    }

    /// Returns normalized axes, acquisition metadata, and provenance.
    pub fn descriptor(&self) -> &RawDescriptor {
        &self.descriptor
    }

    /// Returns source provenance, including opaque format metadata.
    pub fn provenance(&self) -> &RawProvenance {
        &self.provenance
    }

    /// Returns the logical NUS schedule when acquisition was coordinate-driven.
    pub fn sampling_schedule(&self) -> Option<&SamplingSchedule> {
        self.sampling.as_ref()
    }

    pub(super) fn with_format(&self, error: ReadError) -> ReadError {
        match self.provenance.format() {
            Some(format) => error.with_format(format),
            None => error,
        }
    }

    pub(super) fn direct_points(&self) -> usize {
        self.descriptor
            .axes()
            .last()
            .expect("a validated acquisition has a direct axis")
            .points()
    }

    pub(super) fn is_sparse(&self) -> Result<bool, ReadError> {
        let Some(schedule) = &self.sampling else {
            return Ok(false);
        };
        Ok(!schedule.is_complete_unique()?)
    }
}
