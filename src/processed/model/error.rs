use super::*;

/// Validation failures specific to processed scientific products.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProcessedValidationError {
    /// A processed descriptor or tensor has no axes.
    #[error("processed data requires at least one axis")]
    EmptyAxes,
    /// A shared axis invariant failed.
    #[error(transparent)]
    Axis(#[from] AxisValidationError),
    /// More than one axis claims the unique direct acquisition role.
    #[error("processed descriptor has multiple direct acquisition axes")]
    MultipleDirectAxes,
    /// A logical extent is zero.
    #[error("processed data has a zero logical extent")]
    ZeroExtent,
    /// Component counts do not match the logical rank.
    #[error("component counts do not match the logical rank")]
    ComponentRankMismatch,
    /// A component count is zero.
    #[error("component count is zero")]
    ZeroComponentCount,
    /// Component basis, count, domain, and role disagree.
    #[error("component basis is incompatible with count, domain, or role")]
    InvalidComponentBasis,
    /// Axis evidence is non-finite, out of range, or attached to a non-signal axis.
    #[error("processed axis evidence is invalid for its value, role, or domain")]
    InvalidAxisEvidence,
    /// A parameter axis carries evidence that has meaning only for a signal axis.
    #[error("parameter axes cannot carry nucleus, sweep, frequency, or group-delay evidence")]
    SignalEvidenceOnParameterAxis,
    /// Scalar storage length does not match expanded storage shape.
    #[error("processed sample length does not match storage shape")]
    SampleLengthMismatch,
    /// Processed scalar storage contains a non-finite value.
    #[error("processed sample data contains a non-finite value")]
    NonFiniteSample,
    /// Descriptor shape or components disagree with data.
    #[error("processed descriptor and data disagree")]
    DescriptorDataMismatch,
    /// A checked shape computation overflowed.
    #[error("processed size computation overflow")]
    SizeOverflow,
    /// Declared lineage does not have one entry for every processed axis.
    #[error("declared raw lineage length does not match processed rank")]
    OriginRankMismatch,
    /// Declared lineage repeats an axis or references an absent raw axis.
    #[error("declared raw lineage has an invalid mapping on processed axis {axis}")]
    InvalidAxisLineage {
        /// Processed axis with an invalid source reference.
        axis: usize,
    },
    /// Axis role or a known nucleus disagrees with declared raw lineage.
    #[error("declared raw origin is incompatible on axis {axis}")]
    OriginAxisMismatch {
        /// Processed axis index.
        axis: usize,
    },
    /// Only the processing engine may construct or attach library-derived provenance.
    #[error("library-derived provenance cannot be attached by a public constructor")]
    LibraryDerivedProvenance,
    /// Processing history does not reproduce the dataset descriptor or its raw anchors.
    #[error("processing history is inconsistent with its baseline, origin, or descriptor")]
    InvalidProcessingHistory,
}

/// Checked-access failures for processed component tensors.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProcessedAccessError {
    /// A scalar view requires one component on every axis.
    #[error("storage is not scalar")]
    NotScalar,
    /// Only an explicitly Cartesian component basis denotes real and imaginary parts.
    #[error("component axis is not Cartesian")]
    NotCartesian,
    /// A selected axis is outside the tensor.
    #[error("axis {axis} exceeds rank {rank}")]
    AxisOutOfBounds {
        /// Requested axis.
        axis: usize,
        /// Tensor rank.
        rank: usize,
    },
    /// Logical and component coordinate ranks must equal the data rank.
    #[error(
        "coordinate rank mismatch: expected {expected}, logical {logical}, components {components}"
    )]
    RankMismatch {
        /// Required rank.
        expected: usize,
        /// Logical coordinate rank.
        logical: usize,
        /// Component coordinate rank.
        components: usize,
    },
    /// A logical coordinate is outside an axis.
    #[error("logical index {index} exceeds axis {axis} with {points} points")]
    LogicalOutOfBounds {
        /// Axis index.
        axis: usize,
        /// Requested logical index.
        index: usize,
        /// Logical extent.
        points: usize,
    },
    /// A component coordinate is outside an axis.
    #[error("component {component} exceeds axis {axis} with {count} components")]
    ComponentOutOfBounds {
        /// Axis index.
        axis: usize,
        /// Requested component.
        component: usize,
        /// Component count.
        count: usize,
    },
    /// A checked offset computation overflowed.
    #[error("processed access size computation overflow")]
    SizeOverflow,
}
