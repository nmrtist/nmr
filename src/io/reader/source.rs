use crate::Complex64;
use crate::ExecutionContext;
use crate::ReadError;

/// Private adapter boundary for logical and acquisition-ordered trace reads.
pub(crate) trait TraceSource: Send + Sync {
    /// Peak numeric buffers for a trace, including its returned samples.
    /// Retained adapter metadata is accounted separately.
    fn trace_numeric_bytes(&self) -> Result<usize, ReadError>;

    fn snapshot_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        _working_limit: usize,
        _metadata: &crate::VendorMetadata,
    ) -> Result<Option<crate::provenance::SourceDigest>, ReadError> {
        control.check_cancelled()?;
        Ok(None)
    }
    fn read_trace_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError>;

    fn read_scheduled_trace_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        _acquisition: usize,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        self.read_trace_controlled(control, coordinate)
    }
}
