use super::*;

/// Shared errors.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ValidationError {
    /// Temporary storage needed to validate an owned model could not be allocated.
    #[error("could not allocate {requested_bytes} bytes for model validation")]
    Allocation {
        /// Requested temporary payload capacity.
        requested_bytes: usize,
    },
    /// A shared mathematical-axis invariant failed.
    #[error(transparent)]
    Axis(#[from] AxisValidationError),
    /// Canonical normalization evidence is invalid.
    #[error(transparent)]
    Evidence(#[from] EvidenceValidationError),
    /// A component transform is invalid.
    #[error(transparent)]
    Transform(#[from] TransformValidationError),
    /// An axis or shape contains zero logical points.
    #[error("acquisition has no axes or an axis has zero points")]
    ZeroAxis,
    /// A descriptor has no axes.
    #[error("acquisition descriptor has no axes")]
    EmptyAxes,
    /// A region has mismatched rank, no axes, or a zero-sized axis.
    #[error("region start and shape must have equal non-zero rank and positive extents")]
    InvalidRegion,
    /// The direct acquisition axis is absent from the fastest position.
    #[error("the direct acquisition axis must be fastest")]
    DirectAxisNotFastest,
    /// A raw signal role was paired with a parameter domain.
    #[error("raw axis role and mathematical domain disagree")]
    RawRoleDomainMismatch,
    /// Shape and component-lane vectors have different ranks.
    #[error("component lane rank does not match logical shape rank")]
    ComponentRankMismatch,
    /// Dense sample length differs from the expanded storage shape.
    #[error("dense sample length does not match storage shape")]
    DenseLengthMismatch,
    /// A real or imaginary sample component is not finite.
    #[error("sample data contains a non-finite value")]
    NonFiniteSample,
    /// The direct axis is real but one or more samples have a nonzero imaginary part.
    #[error("sample data has imaginary values on a real direct axis")]
    ImaginarySamplesOnRealAxis,
    /// A sparse trace length differs from its component/direct shape.
    #[error("sparse trace length does not match component and direct-axis shape")]
    SparseTraceLengthMismatch,
    /// Sparse data contains no acquired traces.
    #[error("sparse data contains no sampled traces")]
    EmptySparseData,
    /// A sampling coordinate has the wrong indirect rank.
    #[error("sampling coordinate rank does not match the indirect grid")]
    SamplingRankMismatch,
    /// A sampling coordinate lies outside its logical grid.
    #[error("sampling coordinate is out of bounds")]
    SamplingOutOfBounds,
    /// Schedule grid and acquisition indirect shape disagree.
    #[error("sampling grid does not match the acquisition's indirect shape")]
    ScheduleGridMismatch,
    /// A schedule contains no acquired coordinates.
    #[error("a present sampling schedule contains no coordinates")]
    EmptySchedule,
    /// Sparse data was supplied without a coordinate schedule.
    #[error("sparse sample data requires a matching sampling schedule")]
    MissingScheduleForSparseData,
    /// Schedule order or completeness disagrees with sample storage.
    #[error("sampling schedule and sample storage disagree")]
    ScheduleDataMismatch,
    /// Sparse observations reuse an acquisition-order ordinal.
    #[error("sparse observation ordinals must be unique")]
    DuplicateObservationOrdinal,
    /// Descriptor point/lane shape differs from sample data.
    #[error("descriptor axes and sample data disagree")]
    DescriptorDataMismatch,
    /// A named numeric metadata value is invalid.
    #[error("invalid numeric value for {0}")]
    InvalidNumber(&'static str),
    /// A parameter axis carries evidence that has meaning only for a signal axis.
    #[error("parameter axes cannot carry nucleus, sweep, frequency, or group-delay evidence")]
    SignalEvidenceOnParameterAxis,
    /// Group-delay state was attached to a non-direct axis.
    #[error("group-delay state is only valid on the direct acquisition axis")]
    GroupDelayOnNonDirectAxis,
    /// A source parameter name or retained source label is empty.
    #[error("source parameter names must not be empty")]
    EmptySourceParameter,
    /// A portable metadata value and its source evidence are not both present.
    #[error("portable metadata and source evidence must be present together")]
    IncompleteSourceEvidence,
    /// Diffusion metadata does not reference a gradient-strength parameter axis.
    #[error("diffusion metadata does not reference a gradient-strength parameter axis")]
    DiffusionGradientAxisMismatch,
    /// A validated shape computation overflowed `usize`.
    #[error("validated size computation overflow")]
    SizeOverflow,
}

/// Structured failure produced by a logical acquisition access request.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AccessError {
    /// A trace coordinate has the wrong indirect rank.
    #[error("trace coordinate rank mismatch: expected {expected}, found {actual}")]
    TraceRankMismatch {
        /// Required number of indirect coordinate values.
        expected: usize,
        /// Number of coordinate values supplied by the caller.
        actual: usize,
    },
    /// A trace coordinate exceeds one logical indirect axis.
    #[error("trace coordinate index {index} exceeds axis {axis} with {points} points")]
    TraceOutOfBounds {
        /// Zero-based logical axis index.
        axis: usize,
        /// Zero-based coordinate requested by the caller.
        index: usize,
        /// Number of logical points on the axis.
        points: usize,
    },
    /// Region origin or shape has the wrong logical rank.
    #[error(
        "region rank mismatch: expected {expected}, start has {start_rank}, shape has {shape_rank}"
    )]
    RegionRankMismatch {
        /// Required logical rank.
        expected: usize,
        /// Number of origin values supplied by the caller.
        start_rank: usize,
        /// Number of extent values supplied by the caller.
        shape_rank: usize,
    },
    /// A region has zero length on one logical axis.
    #[error("region is empty on axis {axis}")]
    EmptyRegion {
        /// Zero-based logical axis index.
        axis: usize,
    },
    /// A region exceeds one logical axis or its end coordinate overflows.
    #[error(
        "region starting at {start} with length {length} exceeds axis {axis} with {points} points"
    )]
    RegionOutOfBounds {
        /// Zero-based logical axis index.
        axis: usize,
        /// Zero-based region origin on the axis.
        start: usize,
        /// Requested region length on the axis.
        length: usize,
        /// Number of logical points on the axis.
        points: usize,
    },
    /// A valid logical coordinate is absent from a sparse sampling schedule.
    #[error("logical acquisition coordinate {coordinate:?} was not sampled")]
    UnsampledCoordinate {
        /// Zero-based logical indirect coordinate requested by the caller.
        coordinate: Vec<usize>,
    },
    /// More than one observation exists at a requested logical coordinate.
    #[error("logical acquisition coordinate {coordinate:?} has repeated observations")]
    AmbiguousObservation {
        /// Zero-based logical indirect coordinate requested by the caller.
        coordinate: Vec<usize>,
    },
    /// No sparse observation exists for an acquisition-order ordinal.
    #[error("observation ordinal {ordinal:?} is not available")]
    ObservationUnavailable {
        /// Requested acquisition-order ordinal.
        ordinal: ObservationOrdinal,
    },
    /// A valid logical region contains no trace from a sparse sampling schedule.
    #[error("logical acquisition region {start:?} with shape {shape:?} was not sampled")]
    UnsampledRegion {
        /// Zero-based logical region origin.
        start: Vec<usize>,
        /// Logical region shape.
        shape: Vec<usize>,
    },
}

impl From<AxisValidationError> for ReadError {
    fn from(error: AxisValidationError) -> Self {
        ValidationError::Axis(error).into()
    }
}

impl From<ProvenanceError> for ReadError {
    fn from(error: ProvenanceError) -> Self {
        Self::new(
            None,
            ReadErrorReason::Corrupt {
                input: InputSource::memory("normalized provenance"),
                detail: error.to_string(),
            },
        )
    }
}
