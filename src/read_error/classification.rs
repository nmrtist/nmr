use crate::Format;
use crate::acquisition::AxisIndex;
use crate::processed::ProcessedValidationError;
use crate::raw::model::AccessError;
use crate::raw::model::ValidationError;
use std::io;
use thiserror::Error;

use super::{InputSource, ParameterError, ReadCandidate};

/// Resource governed by a reader byte limit.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadResource {
    /// Combined numeric sample-source bytes.
    SourceBytes,
    /// Vendor metadata bytes.
    MetadataBytes,
    /// Temporary decoding or mapping bytes.
    WorkingBytes,
    /// One decoded trace's bytes.
    TraceBytes,
    /// One logical region result's bytes.
    RegionBytes,
    /// A fully materialized dataset's bytes.
    MaterializedBytes,
    /// Raw lanes on one logical axis.
    ComponentLanes,
    /// Coefficients in one resolved component transform.
    TransformCoefficients,
    /// Phases in one periodic lane modulation.
    ModulationPeriod,
    /// Charged component-transform arithmetic work.
    TransformWork,
}

/// Stable machine-readable code for a recognized unsupported acquisition feature.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UnsupportedFeatureCode(&'static str);

impl UnsupportedFeatureCode {
    /// Vendor decoding requires explicit acceptance of incomplete independent evidence.
    pub const EXPERIMENTAL_VENDOR_SEMANTICS: Self = Self("EXPERIMENTAL_VENDOR_SEMANTICS");
    /// Generic unresolved acquisition semantics without a more specific feature code.
    pub const UNRESOLVED_ACQUISITION_SEMANTICS: Self = Self("UNRESOLVED_ACQUISITION_SEMANTICS");
    /// A multidimensional component layout cannot be factored by logical axis.
    pub const NON_SEPARABLE_COMPONENT_LAYOUT: Self = Self("NON_SEPARABLE_COMPONENT_LAYOUT");
    /// Varian component/array semantics are outside the v1 truth table.
    pub const VARIAN_UNSUPPORTED_COMPONENT_LAYOUT: Self =
        Self("VARIAN_UNSUPPORTED_COMPONENT_LAYOUT");

    /// Known unsupported axis units.
    pub const AXIS_UNITS: Self = Self("AXIS_UNITS");
    /// Known unsupported component layout.
    pub const COMPONENT_LAYOUT: Self = Self("COMPONENT_LAYOUT");
    /// Known unsupported missing calibration.
    pub const MISSING_CALIBRATION: Self = Self("MISSING_CALIBRATION");
    /// Known unsupported numeric encoding.
    pub const NUMERIC_ENCODING: Self = Self("NUMERIC_ENCODING");
    /// Known unsupported sampling layout.
    pub const SAMPLING_LAYOUT: Self = Self("SAMPLING_LAYOUT");
    /// Known unsupported spectrum representation.
    pub const SPECTRUM_REPRESENTATION: Self = Self("SPECTRUM_REPRESENTATION");
    /// Known unsupported unsupported rank.
    pub const UNSUPPORTED_RANK: Self = Self("UNSUPPORTED_RANK");
    /// Returns the frozen code string.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Structured reason for a detection, decoding, or dataset-access failure.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ReadErrorReason {
    /// Cooperative execution control failed.
    #[error(transparent)]
    Execution(#[from] crate::execution::ExecutionError),
    /// Input does not match a supported dataset.
    #[error("unrecognized dataset at {input}: {detail}")]
    #[non_exhaustive]
    Unrecognized {
        /// Input being identified.
        input: InputSource,
        /// Format-specific diagnostic detail.
        detail: String,
    },
    /// A required dataset part is absent.
    #[error("incomplete dataset at {input}: {detail}")]
    #[non_exhaustive]
    Incomplete {
        /// Incomplete input.
        input: InputSource,
        /// Missing-part detail.
        detail: String,
    },
    /// More than one input or vendor format matches.
    #[error("ambiguous dataset at {input}: {detail}")]
    #[non_exhaustive]
    Ambiguous {
        /// Input whose identity is ambiguous.
        input: InputSource,
        /// Conflicting-match detail.
        detail: String,
        /// Stable, sorted resolver candidates.
        candidates: Vec<ReadCandidate>,
    },
    /// A precise format assertion did not match the input.
    #[error("format assertion did not match {input}: expected {expected:?}")]
    #[non_exhaustive]
    FormatMismatch {
        /// Input whose format did not match.
        input: InputSource,
        /// Asserted format.
        expected: Format,
        /// Stable, sorted candidates actually found.
        candidates: Vec<ReadCandidate>,
    },
    /// Input ends before its declared byte boundary.
    #[error("truncated {input}: expected {expected} bytes, found {actual}")]
    #[non_exhaustive]
    Truncated {
        /// Truncated input.
        input: InputSource,
        /// Required byte boundary.
        expected: usize,
        /// Available byte count.
        actual: usize,
    },
    /// Input violates a structural or numeric format invariant.
    #[error("corrupt {input}: {detail}")]
    #[non_exhaustive]
    Corrupt {
        /// Structurally invalid input.
        input: InputSource,
        /// Violated format invariant.
        detail: String,
    },
    /// Source metadata is malformed or semantically inconsistent.
    #[error("invalid metadata in {input}: {detail}")]
    #[non_exhaustive]
    InvalidMetadata {
        /// Metadata source.
        input: InputSource,
        /// Typed or textual field name when one is implicated.
        field: Option<String>,
        /// Stable diagnostic detail.
        detail: String,
        /// Structured parameter failure retained as the source, when applicable.
        #[source]
        parameter: Option<ParameterError>,
    },
    /// Input uses a recognized but unsupported semantic representation.
    #[error("unsupported feature {code:?} in {input}")]
    #[non_exhaustive]
    UnsupportedFeature {
        /// Input using the unsupported representation.
        input: InputSource,
        /// Stable machine-readable feature code.
        code: UnsupportedFeatureCode,
        /// Implicated normalized axis, when known.
        axis: Option<AxisIndex>,
        /// Source-layer facts supporting the rejection.
        evidence: Vec<String>,
    },
    /// A configured resource limit was exceeded.
    #[error("{resource:?} limit exceeded: limit {limit}, required {required}")]
    #[non_exhaustive]
    LimitExceeded {
        /// Limited resource.
        resource: ReadResource,
        /// Configured byte limit.
        limit: usize,
        /// Bytes required by the operation.
        required: usize,
    },
    /// Caller assertions conflict with explicit source or header facts.
    #[error("caller assertion conflicts with source evidence: {detail}")]
    #[non_exhaustive]
    AssertionConflict {
        /// Stable conflict detail.
        detail: String,
    },
    /// A source changed after it was opened and validated.
    #[error("source changed while being read: {input}")]
    #[non_exhaustive]
    SourceChanged {
        /// Source whose checked identity changed.
        input: InputSource,
    },
    /// A controlled fallible allocation failed.
    #[error("could not allocate {requested_bytes} bytes")]
    #[non_exhaustive]
    Allocation {
        /// Bytes requested from the allocator.
        requested_bytes: usize,
    },
    /// A filesystem operation failed.
    #[error("I/O error reading {input}: {error}")]
    #[non_exhaustive]
    Io {
        /// Filesystem input involved in the operation.
        input: InputSource,
        #[source]
        /// Underlying operating-system error.
        error: io::Error,
    },
    /// A caller-visible input or aggregate contract was invalid.
    #[error("invalid read request: {detail}")]
    #[non_exhaustive]
    Invalid {
        /// Contract violation detail.
        detail: String,
    },
    /// A logical acquisition access request failed.
    #[error("{0}")]
    Access(
        #[from]
        #[source]
        AccessError,
    ),
    /// A normalized-model invariant failed.
    #[error("{0}")]
    Model(
        #[from]
        #[source]
        ValidationError,
    ),
    /// A normalized processed-model invariant failed.
    #[error("{0}")]
    ProcessedModel(
        #[from]
        #[source]
        ProcessedValidationError,
    ),
    /// A byte or shape calculation overflowed `usize`.
    #[error("size computation overflow")]
    SizeOverflow,
}

impl ReadErrorReason {
    /// Returns the input source carried by source-specific reasons.
    pub fn input_source(&self) -> Option<&InputSource> {
        match self {
            Self::Unrecognized { input, .. }
            | Self::Incomplete { input, .. }
            | Self::Ambiguous { input, .. }
            | Self::FormatMismatch { input, .. }
            | Self::Truncated { input, .. }
            | Self::Corrupt { input, .. }
            | Self::InvalidMetadata { input, .. }
            | Self::UnsupportedFeature { input, .. }
            | Self::SourceChanged { input }
            | Self::Io { input, .. } => Some(input),
            _ => None,
        }
    }
}

/// Stable machine-readable category for a [`ReadError`](crate::ReadError).
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadErrorKind {
    /// The caller cancelled the operation.
    Cancelled,
    /// Input does not match a supported dataset.
    Unrecognized,
    /// A required input part is missing.
    Incomplete,
    /// Format or file selection is ambiguous.
    Ambiguous,
    /// A precise format assertion did not match the input.
    FormatMismatch,
    /// Input ends before a declared byte boundary.
    Truncated,
    /// Input violates a format invariant.
    Corrupt,
    /// Representation is recognized but unsupported.
    UnsupportedFeature,
    /// A resource limit was exceeded.
    LimitExceeded,
    /// Source metadata is missing, malformed, or semantically inconsistent.
    InvalidMetadata,
    /// Caller assertions conflict with source evidence.
    AssertionConflict,
    /// A source changed after validation.
    SourceChanged,
    /// More than one observation exists at one logical coordinate.
    AmbiguousObservation,
    /// A controlled allocation failed.
    Allocation,
    /// Filesystem access failed.
    Io,
    /// A requested access operation is invalid.
    Invalid,
    /// A valid logical coordinate was not acquired in a sparse schedule.
    UnsampledCoordinate,
    /// A valid logical region contains no acquired trace in a sparse schedule.
    UnsampledRegion,
    /// Normalized model validation failed.
    Model,
    /// A checked size computation overflowed.
    SizeOverflow,
}
