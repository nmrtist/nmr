use crate::raw::model::RawFormat;
use std::error::Error as StdError;
use std::fmt;

use super::InputSource;

/// Stable classification of a vendor parameter failure.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterErrorKind {
    /// Parameter storage size overflowed the platform capacity.
    SizeOverflow,
    /// A parameter buffer could not be allocated.
    #[non_exhaustive]
    Allocation {
        /// Requested payload capacity.
        requested_bytes: usize,
    },
    /// A required parameter is absent.
    Missing,
    /// A parameter occurs more than once.
    Duplicate,
    /// A parameter record cannot be parsed.
    Malformed,
    /// A parsed parameter has an invalid value or meaning.
    Invalid,
}

/// Structured vendor parameter failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterError(Box<ParameterErrorInner>);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParameterErrorInner {
    format: RawFormat,
    source: InputSource,
    parameter: Option<String>,
    kind: ParameterErrorKind,
    detail: String,
}

impl ParameterError {
    pub(crate) fn new(
        format: RawFormat,
        source: InputSource,
        parameter: Option<String>,
        kind: ParameterErrorKind,
        detail: impl Into<String>,
    ) -> Self {
        Self(Box::new(ParameterErrorInner {
            format,
            source,
            parameter,
            kind,
            detail: detail.into(),
        }))
    }

    /// Returns the vendor format whose parameter contract failed.
    pub fn format(&self) -> RawFormat {
        self.0.format
    }

    /// Returns the concrete parameter input.
    pub fn input_source(&self) -> &InputSource {
        &self.0.source
    }

    /// Returns the parameter name when one record is implicated.
    pub fn parameter(&self) -> Option<&str> {
        self.0.parameter.as_deref()
    }

    /// Returns the stable parameter-error category.
    pub fn kind(&self) -> ParameterErrorKind {
        self.0.kind
    }

    /// Returns format-specific diagnostic detail.
    pub fn detail(&self) -> &str {
        &self.0.detail
    }
}

impl fmt::Display for ParameterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parameter = self.0.parameter.as_deref().unwrap_or("parameter record");
        write!(
            formatter,
            "{:?} {parameter} error in {}: {}",
            self.0.kind, self.0.source, self.0.detail
        )
    }
}

impl StdError for ParameterError {}
