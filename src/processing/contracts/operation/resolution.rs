use crate::acquisition::ChemicalShiftReference;
use crate::acquisition::ResolvedComponentTransform;
use crate::processing::contracts::polarity::PolarityState;
use crate::processing::contracts::profile::PositivePeaksV1;
use std::sync::Arc;

use super::{
    DelaySource, FourierExponentSign, PhaseCorrection, SpectrumOperation, TimeDomainResidualPolicy,
    Window,
};

/// A versioned baseline algorithm selected by an explicit processing request.
///
/// New profiles can be added without changing the baseline operation's field
/// type. A fixed profile retains its original numerical and history meaning.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaselineProfile {
    /// Fixed positive-peak asymmetric least-squares profile.
    PositivePeaksV1(PositivePeaksV1),
}

impl From<PositivePeaksV1> for BaselineProfile {
    fn from(value: PositivePeaksV1) -> Self {
        Self::PositivePeaksV1(value)
    }
}

/// Recoverable quality failure from an attempted processing operation.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptFailure {
    /// The selected trace contains no usable finite nonzero signal.
    NoUsableSignal,
    /// The requested numerical objective is undefined on the selected input.
    ObjectiveUndefined,
    /// A bounded optimizer did not meet its convergence contract.
    OptimizationDidNotConverge,
}

/// Typed diagnostic retained alongside a processing record.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingDiagnostic {
    /// A configured phase-failure policy continued with unphased real data.
    ContinuedAfterPhaseFailure,
    /// The scalar output retains a possible global 180-degree inversion.
    AmbiguousPolarity,
    /// Coordinate source quality is unknown even though its representation is valid.
    UnknownCoordinateQuality,
}

/// Final scalar projection of Cartesian frequency-domain fields.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Projection {
    /// Selects the real Cartesian field on every signal axis without asserting
    /// absorptive phase quality or positive polarity. Requires
    /// [`PolarityState::Ambiguous180`], retains existing phase state, and does not
    /// require phase correction or a previous optimization failure.
    Real,
    /// RR output with established positive polarity.
    RealAbsorptive,
    /// RR output while retaining possible 180-degree ambiguity.
    RealSigned,
    /// Stable L2 norm across all Cartesian fields.
    Magnitude,
    /// RR output permitted only after an explicit recoverable phase failure.
    UnphasedReal,
}

/// Policy for recoverable automatic-phase quality failures.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseFailurePolicy {
    /// Return the typed phase optimization failure.
    Fail,
    /// Record the failed attempt and continue directly with `UnphasedReal`.
    ContinueUnphasedReal,
}

/// Checked frequency frame used to resolve one signal axis.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum FrequencyFrame {
    /// Retains calibrated centered-bin Hz coordinates.
    Hertz,
    /// Converts signed Hz offsets with a canonical chemical-shift reference.
    Ppm(ReferenceSource),
}

/// Source of a canonical chemical-shift reference.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ReferenceSource {
    /// Uses reference evidence carried by the canonical raw axis.
    AxisEvidence,
    /// Uses an explicit evidence-bearing caller reference.
    Explicit(ChemicalShiftReference),
}

/// Direct-axis delay placement in the fixed dense pipeline.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DirectDelayMode {
    /// Selects FrequencyDomainPhaseRampV1 for positive pending delay evidence.
    /// Pending zero delay is explicitly acknowledged; NotApplicable needs no record. Unknown
    /// evidence is rejected. Explicit correction modes retain their own validation.
    Automatic,
    /// Time-domain correction before windowing.
    TimeDomainShiftFoldV1(DelaySource, TimeDomainResidualPolicy),
    /// Frequency-domain phase ramp after the direct FFT.
    FrequencyDomainPhaseRampV1(DelaySource),
    /// No direct-axis delay correction.
    NoDelay,
}

/// Parameters after all state-dependent canonical values are resolved.
///
/// These values do not independently identify a target. Read them together with
/// [`crate::processing::ProcessingRecord::target`] on their owning history record.
/// Struct variants are non-exhaustive because later releases may retain more
/// execution facts. External matches must use `..`.
///
/// ```compile_fail,E0638
/// use nmr::processing::ResolvedOperation;
/// fn inspect(resolved: &ResolvedOperation) {
///     if let ResolvedOperation::ZeroFill { target_points } = resolved {}
/// }
/// ```
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedOperation {
    /// Actual fitted real baseline; replay subtracts the recorded values.
    EstimatedBaseline {
        /// Baseline at every input coordinate, in input intensity units.
        values: Vec<f64>,
        /// Ascending monomial coefficients in t=2*i/(N-1)-1, empty for AsLS.
        coefficients: Vec<f64>,
    },
    /// Actual result of a named automatic estimator; replay applies this correction.
    AutomaticPhase {
        /// Phase in the library's degrees, positive exponent and full-width convention.
        correction: PhaseCorrection,
        /// Final dimensionless method objective.
        objective: f64,
        /// Number of evaluated candidates.
        evaluations: usize,
    },
    /// Checked spectrum parameters, including normalized windows and bin sizes.
    Spectrum(SpectrumOperation),
    /// A state-only acknowledgement of established zero-delay evidence.
    #[non_exhaustive]
    AcknowledgeZeroDelayV1 {
        /// Original evidence retained without inventing a numeric correction.
        evidence: crate::acquisition::NormalizationEvidence,
    },
    /// A checked window with no inferred parameters.
    Window(Window),
    /// End-only zero filling to the resolved logical point count.
    #[non_exhaustive]
    ZeroFill {
        /// Resulting logical point count.
        target_points: usize,
    },
    /// A Fourier transform with the resolved exponent sign.
    FourierTransform(FourierExponentSign),
    /// A frequency-domain delay correction with resolved delay and FFT sign.
    #[non_exhaustive]
    FrequencyDomainPhaseRampV1 {
        /// Delay in logical points.
        delay: f64,
        /// Sign of the transform that established the frequency-bin layout.
        sign: FourierExponentSign,
    },
    /// The fully resolved Bruker time-domain correction.
    #[non_exhaustive]
    TimeDomainShiftFoldV1 {
        /// Delay applied in logical points.
        applied_delay: f64,
        /// Number of corrected leading points removed.
        skip: usize,
        /// Number of folded tail points.
        fold: usize,
        /// Fractional delay retained for a later operation.
        residual: f64,
    },
    /// Explicit phase parameters applied to the entire selected axis.
    PhaseCorrection(PhaseCorrection),
    /// Resolved component transform applied in canonical lane order.
    #[non_exhaustive]
    ComponentTransform {
        /// Exact evidence-bearing transform used for the operation.
        transform: ResolvedComponentTransform,
        /// Absolute grid point to acquisition ordinal mapping when required.
        observation_ordinals: Option<Arc<[usize]>>,
        /// Absolute origin of the axis named by `AbsoluteGridCoordinate`, added
        /// to that axis's current local index. Zero and unused for observation
        /// ordinal modulation. This resolution uses `linear-component-transform.v1`.
        grid_origin: i64,
    },
    /// Scalar reduction with retained polarity authorization.
    #[non_exhaustive]
    Projection {
        /// Applied scalar projection.
        projection: Projection,
        /// Polarity state used by the projection.
        polarity: PolarityState,
    },
    /// Frequency-frame conversion after validation of its strict profile.
    #[non_exhaustive]
    ResolveFrequencyFrame {
        /// Resolved output frame.
        frame: FrequencyFrame,
        /// Exact reference used for ppm conversion, when applicable.
        reference: Option<ChemicalShiftReference>,
    },
    /// Synchronized sample and coordinate reversal.
    ReverseAxis,
    /// Fixed positive-peak asymmetric least-squares subtraction.
    BaselineCorrection(BaselineProfile),
}
