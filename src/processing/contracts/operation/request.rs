use crate::processing::contracts::polarity::PolarityState;
use crate::processing::contracts::profile::NormalizedAcmeV1;

use super::{
    BaselineProfile, DigitalFilterCorrection, FourierTransform, FrequencyFrame, PhaseCorrection,
    PhaseFailurePolicy, PhaseMethod, Projection, RealBaseline, SpectrumOperation, Window, ZeroFill,
};

/// A deterministic request executable by [`crate::processing::ProcessingPlan`].
///
/// Axis numbers always denote zero-based positions in the current descriptor,
/// immediately before this operation. They are not persistent dataset identities
/// or source-lineage references. Existing request fields remain constructible;
/// new settings requiring a different contract use new variants or parameter types.
///
/// Data-dependent automatic phasing is applied through
/// [`NormalizedAcmeV1::apply`] and described by [`ProcessingRequest`] in history.
/// Baseline correction retains experimental numerical quality and follows the
/// same [compatibility policy](crate#experimental-algorithms)
/// as all public operations.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessingOperation {
    /// Explicit spectrum manipulation with descriptor and history transitions.
    Spectrum {
        /// Axis in the descriptor immediately before this step.
        axis: usize,
        /// Mathematical operation; no implicit preprocessing is added.
        operation: SpectrumOperation,
    },
    /// Applies a window to a time-domain signal axis.
    Window {
        /// Zero-based processed axis.
        axis: usize,
        /// Window parameters.
        window: Window,
    },
    /// Appends zeros to a time-domain signal axis.
    ZeroFill {
        /// Zero-based processed axis.
        axis: usize,
        /// Zero-fill parameters.
        zero_fill: ZeroFill,
    },
    /// Zero-fills to `next_power_of_two(2 * logical_length)`.
    StandardZeroFill {
        /// Zero-based processed axis.
        axis: usize,
    },
    /// Fourier transforms a time-domain signal axis.
    FourierTransform {
        /// Zero-based processed axis.
        axis: usize,
        /// Transform parameters.
        transform: FourierTransform,
    },
    /// Corrects a digital-filter delay.
    DigitalFilterCorrection {
        /// Zero-based processed axis.
        axis: usize,
        /// Correction profile.
        correction: DigitalFilterCorrection,
    },
    /// Applies phase correction to a Cartesian frequency axis.
    PhaseCorrection {
        /// Zero-based processed axis.
        axis: usize,
        /// Phase parameters.
        correction: PhaseCorrection,
    },
    /// Experimental: subtracts a scalar frequency-domain baseline with fixed AsLS V1.
    BaselineCorrection {
        /// Zero-based processed axis.
        axis: usize,
        /// Versioned baseline profile.
        profile: BaselineProfile,
    },
    /// Applies the resolved indirect component transform.
    ComponentTransform {
        /// Zero-based processed axis.
        axis: usize,
    },
    /// Reduces all signal-axis Cartesian fields to scalar samples.
    Projection {
        /// Requested scalar projection.
        projection: Projection,
        /// Polarity authorization retained with the request.
        polarity: PolarityState,
    },
    /// Resolves one calibrated frequency axis into the requested frame.
    ResolveFrequencyFrame {
        /// Zero-based processed axis.
        axis: usize,
        /// Requested output frame.
        frame: FrequencyFrame,
    },
    /// Reverses samples and coordinates together on one logical axis.
    /// Supports either direction with known coordinates; a singleton is unchanged.
    /// Multi-point reversal invalidates the FFT bin mapping, including when reversed twice.
    /// History uses `reverse-axis.v1`.
    ReverseAxis {
        /// Zero-based processed axis.
        axis: usize,
    },
}

impl ProcessingOperation {
    /// Returns the actual scope of the operation.
    ///
    /// Projection reduces Cartesian fields across all signal axes and therefore
    /// targets the dataset, without designating an unrelated bookkeeping axis.
    pub fn target(&self) -> OperationTarget {
        match self.axis() {
            Some(axis) => OperationTarget::Axis(axis),
            None => OperationTarget::Dataset,
        }
    }

    pub(crate) fn axis(&self) -> Option<usize> {
        match self {
            Self::Spectrum { axis, .. }
            | Self::Window { axis, .. }
            | Self::ZeroFill { axis, .. }
            | Self::StandardZeroFill { axis }
            | Self::FourierTransform { axis, .. }
            | Self::DigitalFilterCorrection { axis, .. }
            | Self::PhaseCorrection { axis, .. }
            | Self::BaselineCorrection { axis, .. }
            | Self::ComponentTransform { axis }
            | Self::ResolveFrequencyFrame { axis, .. }
            | Self::ReverseAxis { axis } => Some(*axis),
            Self::Projection { .. } => None,
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Spectrum { .. } => "spectrum operation",
            Self::Window { .. } => "window",
            Self::ZeroFill { .. } => "zero fill",
            Self::StandardZeroFill { .. } => "standard zero fill",
            Self::FourierTransform { .. } => "Fourier transform",
            Self::DigitalFilterCorrection {
                correction: DigitalFilterCorrection::AcknowledgeZeroDelayV1,
                ..
            } => "zero-delay acknowledgement",
            Self::DigitalFilterCorrection {
                correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(_),
                ..
            } => "delay phase ramp",
            Self::DigitalFilterCorrection {
                correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 { .. },
                ..
            } => "time-domain shift/fold correction",
            Self::PhaseCorrection { .. } => "phase correction",
            Self::BaselineCorrection { .. } => "baseline correction",
            Self::ComponentTransform { .. } => "component transform",
            Self::Projection { .. } => "scalar projection",
            Self::ResolveFrequencyFrame { .. } => "frequency-frame resolution",
            Self::ReverseAxis { .. } => "axis reversal",
        }
    }
}

/// The logical scope on which an operation acts.
///
/// Axis indices refer to the descriptor immediately before the operation.
/// Dataset-wide operations may inspect or reduce fields across multiple axes;
/// their scope does not imply that every descriptor field changes.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationTarget {
    /// One zero-based logical axis.
    Axis(usize),
    /// An ordered collection of logical axes.
    Axes(Vec<usize>),
    /// The dataset as a whole, with no distinguished target axis.
    Dataset,
}

/// The original request retained by a processing-history record.
///
/// Explicit requests can be submitted to a plan. Automatic requests describe
/// calls to data-dependent algorithms and are never disguised as plan steps.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessingRequest {
    /// Analyze a real baseline on a selected one-dimensional spectrum.
    BaselineEstimate {
        /// Zero-based frequency axis.
        axis: usize,
        /// Explicit baseline method and settings.
        method: RealBaseline,
    },
    /// Explicit selection of one versioned automatic phase method.
    PhaseMethod {
        /// Selected frequency axis.
        axis: usize,
        /// Actual estimator; no silent fallback is permitted.
        method: PhaseMethod,
    },
    /// An explicit deterministic plan operation.
    Explicit(ProcessingOperation),
    /// Automatic phase optimization through [`NormalizedAcmeV1::apply`].
    #[non_exhaustive]
    AutoPhase {
        /// Zero-based processed axis.
        axis: usize,
        /// Immutable optimization profile.
        profile: NormalizedAcmeV1,
        /// Existing polarity authorization.
        polarity: PolarityState,
        /// Requested behavior for recoverable quality failure.
        failure_policy: PhaseFailurePolicy,
    },
}

impl ProcessingRequest {
    /// Returns the actual scope of the original request.
    pub fn target(&self) -> OperationTarget {
        match self {
            Self::Explicit(operation) => operation.target(),
            Self::AutoPhase { axis, .. }
            | Self::PhaseMethod { axis, .. }
            | Self::BaselineEstimate { axis, .. } => OperationTarget::Axis(*axis),
        }
    }

    /// Returns the executable plan request, when this record came from a plan.
    pub fn explicit(&self) -> Option<&ProcessingOperation> {
        match self {
            Self::Explicit(operation) => Some(operation),
            Self::AutoPhase { .. } | Self::PhaseMethod { .. } | Self::BaselineEstimate { .. } => {
                None
            }
        }
    }
}

impl From<ProcessingOperation> for ProcessingRequest {
    fn from(value: ProcessingOperation) -> Self {
        Self::Explicit(value)
    }
}
