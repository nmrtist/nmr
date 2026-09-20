use crate::execution::ExecutionError;
use thiserror::Error;

/// Failure while validating, bounding or writing a report/comparison.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ReportError {
    /// Cooperative cancellation or execution resource failure.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// A source or derived numeric value is not finite.
    #[error("non-finite report or comparison number")]
    NonFinite,
    /// The requested report exceeds its output-payload budget.
    #[error("report requires {required} bytes, limit {limit}")]
    #[non_exhaustive]
    LimitExceeded {
        /// Bytes needed so far, or exact validated report size.
        required: usize,
        /// Caller-specified payload limit.
        limit: usize,
    },
    /// Comparison inputs have different shapes or axis semantics.
    #[error("comparison inputs have incompatible shape or axis semantics")]
    IncompatibleInputs,
    /// A metric or option is undefined for the supplied domain.
    #[error("invalid comparison/report request: {0}")]
    Invalid(&'static str),
    /// A writer failed; it may have received a report prefix.
    #[error("report writer failed: {0}")]
    Io(std::io::Error),
}

impl From<std::io::Error> for ReportError {
    fn from(error: std::io::Error) -> Self {
        match crate::io::io_control_error(&error) {
            Some(e) => Self::Execution(e),
            None => Self::Io(error),
        }
    }
}
