//! Internal model/evidence failures, independent of any transport format.

#[derive(Debug, thiserror::Error)]
pub(crate) enum ModelError {
    #[error("invalid model structure")]
    Structure,
    #[error("invalid model: {0}")]
    Validation(String),
    #[error("unsupported history version {0}")]
    UnsupportedHistoryVersion(String),
    #[error("scientific digest mismatch")]
    DigestMismatch,
    #[error(transparent)]
    Execution(#[from] crate::execution::ExecutionError),
}
