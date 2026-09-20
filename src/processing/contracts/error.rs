//! Public processing failures, independent of execution entry points.

use crate::processed::ProcessedValidationError;
use crate::processing::contracts::state::StateError;
use crate::processing::kernels::acme::PhaseOptimizationError;
use crate::processing::kernels::asls::AslsError;
use thiserror::Error;

/// Failures from checked processing, state transitions, canonical profiles, or limits.
///
/// Experimental algorithm failures share the same compatibility policy as other
/// variants. Match this non-exhaustive enum with a fallback for future variants.
#[non_exhaustive]
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ProcessingError {
    /// Automatic noise analysis failed without a fallback sigma.
    #[error(transparent)]
    NoiseEstimation(crate::processing::NusNoiseError),
    /// General or fixed NUS reconstruction failed its explicit contract.
    #[error(transparent)]
    Reconstruction(crate::processing::kernels::ist::IstError),
    /// Location of a failed caller operation; indices are zero-based.
    #[error("{phase:?} step {step_index:?} ({target:?}): {source}")]
    Step {
        /// Position in the submitted plan or history, absent for a whole-plan failure.
        step_index: Option<usize>,
        /// Requested operation scope, absent for a whole-plan failure.
        target: Option<crate::processing::contracts::operation::OperationTarget>,
        /// Stage at which the failure occurred.
        phase: ProcessingPhase,
        /// Structured underlying failure.
        source: Box<ProcessingError>,
    },
    /// Cooperative execution was cancelled; no partial dataset is returned.
    #[error("execution cancelled")]
    Cancelled,
    /// An archived algorithm or history rule is not supported.
    #[error("unsupported history algorithm: {0}")]
    UnsupportedHistoryVersion(String),
    /// A dataset-wide operation is incompatible with the current state.
    #[error("{operation}: {reason}")]
    InvalidDatasetState {
        /// Name of the requested operation.
        operation: &'static str,
        /// Why the whole-dataset operation is unavailable.
        reason: &'static str,
    },
    /// A structured shared memory resource limit was exceeded.
    #[error(transparent)]
    LimitExceeded(#[from] crate::resource::LimitExceeded),
    /// Replay received a different number of ordered external inputs.
    #[error("replay input count mismatch: expected {expected}, found {actual}")]
    InputCountMismatch {
        /// Number of inputs bound by the supported history.
        expected: usize,
        /// Number of inputs supplied by the caller.
        actual: usize,
    },
    /// A processing plan contains no operations.
    #[error("processing plan is empty")]
    EmptyPlan,
    /// A named parameter is invalid.
    #[error("invalid processing parameter: {0}")]
    InvalidParameter(&'static str),
    /// The selected axis or operation order is scientifically invalid.
    #[error("{operation} is invalid on axis {axis}: {reason}")]
    InvalidState {
        /// Zero-based selected axis.
        axis: usize,
        /// Operation name.
        operation: &'static str,
        /// Stable reason.
        reason: &'static str,
    },
    /// Raw component lanes cannot be mapped without inventing meaning.
    #[error("raw-to-processed mapping failed: {0}")]
    Mapping(&'static str),
    /// Dense processing deliberately supports only rank one and two.
    #[error("dense processing does not support rank {rank}")]
    UnsupportedRank {
        /// Raw descriptor rank.
        rank: usize,
    },
    /// A requested operation requires optional canonical evidence that is absent.
    #[error("missing processing capability {capability} on axis {axis:?}")]
    MissingCapability {
        /// Stable capability name.
        capability: &'static str,
        /// Implicated axis, when applicable.
        axis: Option<usize>,
    },
    /// A prepared plan was applied to a different descriptor or sample identity.
    #[error("prepared plan input identity does not match the dataset")]
    InputIdentityMismatch,
    /// Delay evidence and an explicit value disagree.
    #[error("explicit delay disagrees with axis delay evidence")]
    DelayEvidenceMismatch,
    /// A time-domain operation requires a positive uniform dwell that is absent.
    #[error("time-domain signal has no positive uniform dwell calibration")]
    MissingTimeCalibration,
    /// A checked byte, shape, skip, or offset computation overflowed.
    #[error("processing size computation overflow")]
    SizeOverflow,
    /// Charged deterministic numerical work exceeds the configured work limit.
    #[error("processing work exceeds the configured work limit")]
    WorkLimit,
    /// A sample or scratch buffer reservation failed.
    #[error("processing allocation failed")]
    AllocationFailure,
    /// Finite input produced a non-finite value inside a required numerical operation.
    #[error("processing numerical invariant was violated")]
    NumericalInvariantViolation,
    /// Bounded automatic phase optimization failed.
    #[error(transparent)]
    PhaseOptimization(PhaseOptimizationError),
    /// Fixed asymmetric least-squares correction failed.
    #[error(transparent)]
    Baseline(AslsError),
    /// The resulting processed model failed aggregate validation.
    #[error(transparent)]
    Validation(#[from] ProcessedValidationError),
}

/// Stage of a processing failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingPhase {
    /// State, request, and resource validation.
    Preflight,
    /// Numerical execution.
    Execution,
    /// Interpretation or execution of recorded history.
    Replay,
}

/// Stable categories for consumer branching without matching explanatory text.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingErrorCode {
    /// Missing scientific evidence or unsupported representation.
    MissingCapability,
    /// Operation order or scientific state is invalid.
    InvalidState,
    /// A request parameter is invalid.
    InvalidRequest,
    /// Work, memory, arithmetic capacity, or allocation failed.
    ResourceLimit,
    /// Cooperative cancellation.
    Cancelled,
    /// Unsupported algorithm or history interpretation.
    UnsupportedHistoryVersion,
    /// Supplied data does not match the bound identity.
    IdentityMismatch,
    /// A numerical algorithm failed its contract.
    NumericalFailure,
}

impl ProcessingError {
    /// Returns the innermost typed cause while retaining location on this value.
    pub fn root_cause(&self) -> &Self {
        match self {
            Self::Step { source, .. } => source.root_cause(),
            _ => self,
        }
    }
    /// Consumes location wrappers and returns the original typed cause.
    pub fn into_root_cause(self) -> Self {
        match self {
            Self::Step { source, .. } => source.into_root_cause(),
            _ => self,
        }
    }
    /// Returns the zero-based caller step index, when available.
    pub fn step_index(&self) -> Option<usize> {
        match self {
            Self::Step { step_index, .. } => *step_index,
            _ => None,
        }
    }
    /// Returns a stable failure classification.
    pub fn code(&self) -> ProcessingErrorCode {
        use ProcessingErrorCode as C;
        match self.root_cause() {
            Self::Reconstruction(error) => match error {
                crate::processing::kernels::ist::IstError::Cancelled => C::Cancelled,
                crate::processing::kernels::ist::IstError::OutputLimit
                | crate::processing::kernels::ist::IstError::WorkingLimit
                | crate::processing::kernels::ist::IstError::WorkLimit
                | crate::processing::kernels::ist::IstError::SizeOverflow
                | crate::processing::kernels::ist::IstError::AllocationFailure => C::ResourceLimit,
                crate::processing::kernels::ist::IstError::InvalidInput
                | crate::processing::kernels::ist::IstError::InvalidOptions
                | crate::processing::kernels::ist::IstError::NoiseEstimateRequired => {
                    C::InvalidRequest
                }
                _ => C::NumericalFailure,
            },
            Self::NoiseEstimation(
                crate::processing::NusNoiseError::UnsupportedOperation { .. }
                | crate::processing::NusNoiseError::InsufficientSamples,
            ) => C::MissingCapability,
            Self::NoiseEstimation(_) => C::NumericalFailure,
            Self::Cancelled => C::Cancelled,
            Self::UnsupportedHistoryVersion(_) => C::UnsupportedHistoryVersion,
            Self::MissingCapability { .. }
            | Self::MissingTimeCalibration
            | Self::UnsupportedRank { .. } => C::MissingCapability,
            Self::InvalidState { .. }
            | Self::InvalidDatasetState { .. }
            | Self::Mapping(_)
            | Self::DelayEvidenceMismatch
            | Self::Validation(_) => C::InvalidState,
            Self::LimitExceeded(_)
            | Self::SizeOverflow
            | Self::WorkLimit
            | Self::AllocationFailure => C::ResourceLimit,
            Self::InputIdentityMismatch | Self::InputCountMismatch { .. } => C::IdentityMismatch,
            Self::NumericalInvariantViolation | Self::PhaseOptimization(_) | Self::Baseline(_) => {
                C::NumericalFailure
            }
            _ => C::InvalidRequest,
        }
    }
    pub(crate) fn located(
        self,
        step_index: Option<usize>,
        target: Option<crate::processing::contracts::operation::OperationTarget>,
        phase: ProcessingPhase,
    ) -> Self {
        if matches!(self, Self::Step { .. }) {
            self
        } else {
            Self::Step {
                step_index,
                target,
                phase,
                source: Box::new(self),
            }
        }
    }
}

impl From<StateError> for ProcessingError {
    fn from(error: StateError) -> Self {
        match error {
            StateError::Cancelled => Self::Cancelled,
            StateError::InvalidDatasetState { operation, reason } => {
                Self::InvalidDatasetState { operation, reason }
            }
            StateError::MissingCapability { capability, axis } => {
                Self::MissingCapability { capability, axis }
            }
            StateError::InvalidParameter(parameter) => Self::InvalidParameter(parameter),
            StateError::InvalidState {
                axis,
                operation,
                reason,
            } => Self::InvalidState {
                axis,
                operation,
                reason,
            },
            StateError::Mapping(detail) => Self::Mapping(detail),
            StateError::DelayEvidenceMismatch => Self::DelayEvidenceMismatch,
            StateError::MissingTimeCalibration => Self::MissingTimeCalibration,
            StateError::SizeOverflow => Self::SizeOverflow,
            StateError::AllocationFailure => Self::AllocationFailure,
            StateError::Validation(error) => Self::Validation(error),
        }
    }
}

impl From<crate::resource::ResourceError> for ProcessingError {
    fn from(value: crate::resource::ResourceError) -> Self {
        match value {
            crate::resource::ResourceError::SizeOverflow => Self::SizeOverflow,
            crate::resource::ResourceError::WorkLimit => Self::WorkLimit,
        }
    }
}

impl From<crate::processing::kernels::fft::FftError> for ProcessingError {
    fn from(value: crate::processing::kernels::fft::FftError) -> Self {
        match value {
            crate::processing::kernels::fft::FftError::SizeOverflow => Self::SizeOverflow,
            crate::processing::kernels::fft::FftError::InvalidLength => {
                Self::InvalidParameter("FFT length")
            }
            crate::processing::kernels::fft::FftError::ScratchBound => {
                Self::Mapping("FFT backend exceeded audited scratch bound")
            }
        }
    }
}

impl From<PhaseOptimizationError> for ProcessingError {
    fn from(error: PhaseOptimizationError) -> Self {
        match error {
            PhaseOptimizationError::Execution(e) => e.into(),
            PhaseOptimizationError::WorkLimit => Self::WorkLimit,
            other => Self::PhaseOptimization(other),
        }
    }
}
impl From<AslsError> for ProcessingError {
    fn from(error: AslsError) -> Self {
        match error {
            AslsError::Execution(e) => e.into(),
            other => Self::Baseline(other),
        }
    }
}

impl From<crate::execution::ExecutionError> for ProcessingError {
    fn from(error: crate::execution::ExecutionError) -> Self {
        match error {
            crate::execution::ExecutionError::Cancelled => Self::Cancelled,
            crate::execution::ExecutionError::WorkLimit => Self::WorkLimit,
            crate::execution::ExecutionError::SizeOverflow => Self::SizeOverflow,
        }
    }
}
