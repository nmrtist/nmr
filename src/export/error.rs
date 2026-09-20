use super::*;

/// Failures from bounded deterministic NPZ V1 export.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ExportError {
    /// Cooperative cancellation or execution resource failure.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// A checked size computation overflowed.
    #[error("NPZ export size computation overflow")]
    SizeOverflow,
    /// An NPY v1.0 header would exceed its `u16` length field.
    #[error("NPY v1.0 header exceeds 65535 bytes")]
    NpyHeaderTooLarge,
    /// An NPY header could not satisfy the fixed ASCII contract.
    #[error("invalid NPY v1.0 header")]
    InvalidNpyHeader,
    /// One member, offset, or entry count does not fit ZIP32.
    #[error("NPZ export exceeds a ZIP32 field")]
    Zip32Limit,
    /// The complete archive exceeds the fixed 512 MiB limit.
    #[error("NPZ archive exceeds the 512 MiB export limit")]
    ArchiveLimit,
    /// Deterministic export work exceeded the supplied ledger.
    #[error(transparent)]
    Work(#[from] ProcessingError),
    /// A bounded allocation failed.
    #[error("NPZ export allocation failed")]
    AllocationFailure,
    /// The temporary output could not be created beside the target.
    #[error("failed to create NPZ temporary file: {0}")]
    CreateTemporary(io::Error),
    /// Archive writing failed.
    #[error(transparent)]
    Io(io::Error),
    /// The target already exists and was not overwritten.
    #[error("NPZ export target already exists")]
    TargetExists,
    /// Publishing failed; the named temporary link may remain when cleanup failed.
    #[error(
        "failed to publish NPZ (temporary path: {temporary_path:?}, cleanup_failed: {cleanup_failed})"
    )]
    Publish {
        /// Original filesystem error.
        #[source]
        source: io::Error,
        /// Temporary link used beside the destination.
        temporary_path: PathBuf,
        /// Whether best-effort removal of the temporary link also failed.
        cleanup_failed: bool,
    },
}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        match crate::io::io_control_error(&error) {
            Some(e) => Self::Execution(e),
            None => Self::Io(error),
        }
    }
}
