use crate::Format;
use crate::acquisition::AxisIndex;
use crate::acquisition::EvidenceValidationError;
use crate::acquisition::TransformValidationError;
use crate::raw::model::AccessError;
use crate::raw::model::ValidationError;
use std::error::Error as StdError;
use std::fmt;
use std::io;

use super::{
    InputSource, ParameterError, ParameterErrorKind, ReadCandidate, ReadErrorKind, ReadErrorReason,
    ReadResource, UnsupportedFeatureCode,
};

/// Error produced while detecting, reading, or accessing a dataset.
#[derive(Debug)]
pub struct ReadError {
    format: Option<Format>,
    reason: ReadErrorReason,
}

impl ReadError {
    #[allow(non_upper_case_globals)]
    pub(crate) const SizeOverflow: Self = Self {
        format: None,
        reason: ReadErrorReason::SizeOverflow,
    };

    pub(crate) fn new(format: Option<Format>, reason: ReadErrorReason) -> Self {
        Self { format, reason }
    }

    pub(crate) fn with_format(mut self, format: impl Into<Format>) -> Self {
        if self.format.is_none() {
            self.format = Some(format.into());
        }
        self
    }

    pub(crate) fn truncated(input: InputSource, expected: usize, actual: usize) -> Self {
        Self::new(
            None,
            ReadErrorReason::Truncated {
                input,
                expected,
                actual,
            },
        )
    }

    pub(crate) fn io(path: impl Into<InputSource>, error: io::Error) -> Self {
        Self::new(
            None,
            ReadErrorReason::Io {
                input: path.into(),
                error,
            },
        )
    }

    pub(crate) fn limit(resource: ReadResource, limit: usize, required: usize) -> Self {
        Self::new(
            None,
            ReadErrorReason::LimitExceeded {
                resource,
                limit,
                required,
            },
        )
    }

    pub(crate) fn allocation(requested_bytes: usize) -> Self {
        Self::new(None, ReadErrorReason::Allocation { requested_bytes })
    }

    pub(crate) fn unrecognized(input: InputSource, detail: impl Into<String>) -> Self {
        Self::new(
            None,
            ReadErrorReason::Unrecognized {
                input,
                detail: detail.into(),
            },
        )
    }

    pub(crate) fn incomplete(input: InputSource, detail: impl Into<String>) -> Self {
        Self::new(
            None,
            ReadErrorReason::Incomplete {
                input,
                detail: detail.into(),
            },
        )
    }

    pub(crate) fn ambiguous(input: InputSource, detail: impl Into<String>) -> Self {
        Self::new(
            None,
            ReadErrorReason::Ambiguous {
                input,
                detail: detail.into(),
                candidates: Vec::new(),
            },
        )
    }

    pub(crate) fn ambiguous_candidates(
        input: InputSource,
        detail: impl Into<String>,
        mut candidates: Vec<ReadCandidate>,
    ) -> Self {
        candidates.sort();
        candidates.dedup();
        Self::new(
            None,
            ReadErrorReason::Ambiguous {
                input,
                detail: detail.into(),
                candidates,
            },
        )
    }

    pub(crate) fn format_mismatch(
        input: InputSource,
        expected: Format,
        mut candidates: Vec<ReadCandidate>,
    ) -> Self {
        candidates.sort();
        candidates.dedup();
        Self::new(
            None,
            ReadErrorReason::FormatMismatch {
                input,
                expected,
                candidates,
            },
        )
    }

    pub(crate) fn corrupt(input: InputSource, detail: impl Into<String>) -> Self {
        Self::new(
            None,
            ReadErrorReason::Corrupt {
                input,
                detail: detail.into(),
            },
        )
    }

    pub(crate) fn invalid_metadata(
        input: InputSource,
        field: Option<impl Into<String>>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new(
            None,
            ReadErrorReason::InvalidMetadata {
                input,
                field: field.map(Into::into),
                detail: detail.into(),
                parameter: None,
            },
        )
    }

    pub(crate) fn unsupported(input: InputSource, detail: impl Into<String>) -> Self {
        Self::unsupported_feature(
            input,
            UnsupportedFeatureCode::UNRESOLVED_ACQUISITION_SEMANTICS,
            None,
            vec![detail.into()],
        )
    }

    pub(crate) fn unsupported_code(
        input: InputSource,
        code: UnsupportedFeatureCode,
        detail: impl Into<String>,
    ) -> Self {
        Self::unsupported_feature(input, code, None, vec![detail.into()])
    }

    pub(crate) fn unsupported_feature(
        input: InputSource,
        code: UnsupportedFeatureCode,
        axis: Option<AxisIndex>,
        evidence: Vec<String>,
    ) -> Self {
        Self::new(
            None,
            ReadErrorReason::UnsupportedFeature {
                input,
                code,
                axis,
                evidence,
            },
        )
    }

    pub(crate) fn assertion_conflict(detail: impl Into<String>) -> Self {
        Self::new(
            None,
            ReadErrorReason::AssertionConflict {
                detail: detail.into(),
            },
        )
    }

    pub(crate) fn source_changed(input: InputSource) -> Self {
        Self::new(None, ReadErrorReason::SourceChanged { input })
    }

    /// Returns the vendor format when established without conflict.
    pub fn format(&self) -> Option<Format> {
        self.format
    }

    /// Returns the stable high-level category.
    pub fn kind(&self) -> ReadErrorKind {
        match &self.reason {
            ReadErrorReason::Execution(crate::execution::ExecutionError::Cancelled) => {
                ReadErrorKind::Cancelled
            }
            ReadErrorReason::Execution(crate::execution::ExecutionError::WorkLimit) => {
                ReadErrorKind::LimitExceeded
            }
            ReadErrorReason::Execution(crate::execution::ExecutionError::SizeOverflow) => {
                ReadErrorKind::SizeOverflow
            }
            ReadErrorReason::Unrecognized { .. } => ReadErrorKind::Unrecognized,
            ReadErrorReason::Incomplete { .. } => ReadErrorKind::Incomplete,
            ReadErrorReason::Ambiguous { .. } => ReadErrorKind::Ambiguous,
            ReadErrorReason::FormatMismatch { .. } => ReadErrorKind::FormatMismatch,
            ReadErrorReason::Truncated { .. } => ReadErrorKind::Truncated,
            ReadErrorReason::Corrupt { .. } => ReadErrorKind::Corrupt,
            ReadErrorReason::InvalidMetadata { .. } => ReadErrorKind::InvalidMetadata,
            ReadErrorReason::UnsupportedFeature { .. } => ReadErrorKind::UnsupportedFeature,
            ReadErrorReason::LimitExceeded { .. } => ReadErrorKind::LimitExceeded,
            ReadErrorReason::AssertionConflict { .. } => ReadErrorKind::AssertionConflict,
            ReadErrorReason::SourceChanged { .. } => ReadErrorKind::SourceChanged,
            ReadErrorReason::Allocation { .. } => ReadErrorKind::Allocation,
            ReadErrorReason::Io { .. } => ReadErrorKind::Io,
            ReadErrorReason::Access(AccessError::UnsampledCoordinate { .. }) => {
                ReadErrorKind::UnsampledCoordinate
            }
            ReadErrorReason::Access(AccessError::UnsampledRegion { .. }) => {
                ReadErrorKind::UnsampledRegion
            }
            ReadErrorReason::Access(AccessError::AmbiguousObservation { .. }) => {
                ReadErrorKind::AmbiguousObservation
            }
            ReadErrorReason::Access(_) => ReadErrorKind::Invalid,
            ReadErrorReason::Invalid { .. } => ReadErrorKind::Invalid,
            ReadErrorReason::Model(_) => ReadErrorKind::Model,
            ReadErrorReason::ProcessedModel(_) => ReadErrorKind::Model,
            ReadErrorReason::SizeOverflow => ReadErrorKind::SizeOverflow,
        }
    }

    /// Returns the complete structured failure reason.
    pub fn reason(&self) -> &ReadErrorReason {
        &self.reason
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(format) = self.format {
            write!(formatter, "{format:?} reader error: ")?;
        }
        fmt::Display::fmt(&self.reason, formatter)
    }
}

impl StdError for ReadError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.reason)
    }
}

impl From<ParameterError> for ReadError {
    fn from(error: ParameterError) -> Self {
        match error.kind() {
            ParameterErrorKind::SizeOverflow => {
                return Self::SizeOverflow.with_format(error.format());
            }
            ParameterErrorKind::Allocation { requested_bytes } => {
                return Self::allocation(requested_bytes).with_format(error.format());
            }
            _ => {}
        }
        let input = error.input_source().clone();
        let field = error.parameter().map(str::to_owned);
        let detail = error.detail().to_owned();
        Self::new(
            Some(error.format().into()),
            ReadErrorReason::InvalidMetadata {
                input,
                field,
                detail,
                parameter: Some(error),
            },
        )
    }
}

impl From<AccessError> for ReadError {
    fn from(error: AccessError) -> Self {
        Self::new(None, ReadErrorReason::Access(error))
    }
}

impl From<ValidationError> for ReadError {
    fn from(error: ValidationError) -> Self {
        match error {
            ValidationError::Allocation { requested_bytes } => Self::allocation(requested_bytes),
            error => Self::new(None, ReadErrorReason::Model(error)),
        }
    }
}

impl From<EvidenceValidationError> for ReadError {
    fn from(error: EvidenceValidationError) -> Self {
        Self::from(ValidationError::Evidence(error))
    }
}

impl From<TransformValidationError> for ReadError {
    fn from(error: TransformValidationError) -> Self {
        Self::from(ValidationError::Transform(error))
    }
}

impl From<crate::execution::ExecutionError> for ReadError {
    fn from(error: crate::execution::ExecutionError) -> Self {
        Self::new(None, ReadErrorReason::Execution(error))
    }
}
