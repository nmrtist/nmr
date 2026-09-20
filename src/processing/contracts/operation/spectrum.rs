/// Versioned automatic phasing methods, independent of NormalizedAcmeV1.
/// These infer positive absorption and remain experimental outside tested signals.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseMethod {
    /// Rotate the maximum-magnitude point to the positive real axis; zero order only.
    AbsorptivePeak,
    /// Unit-maximum normalized derivative entropy plus 1000 times negative energy.
    /// Full-resolution search includes endpoint first-order phase in ±720 degrees.
    Entropy,
    /// Minimize negative real energy divided by all real energy.
    NegativeMinimization,
    /// Height-weighted regression of unwrapped phases at resolved magnitude maxima.
    PeakRegression,
    /// Compare distinct candidates and refine a dominant-peak-constrained objective.
    RobustConsensus,
}
impl PhaseMethod {
    /// Stable mathematical and search-profile identifier.
    /// Entropy, NegativeMinimization and RobustConsensus search endpoint
    /// first-order phase in ±720 degrees with the original 15-degree grid spacing.
    pub fn algorithm_version(self) -> &'static str {
        match self {
            Self::AbsorptivePeak => "phase-absorptive-peak.v1",
            Self::Entropy => "phase-derivative-entropy.v1",
            Self::NegativeMinimization => "phase-negative-energy.v1",
            Self::PeakRegression => "phase-peak-regression.v1",
            Self::RobustConsensus => "phase-robust-consensus.v1",
        }
    }
}

/// Explicit real-channel baseline definitions. These do not replace PositivePeaksV1.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RealBaseline {
    /// Median/MAD clipped mean; MAD=0 uses the median exactly.
    Offset,
    /// Fit a polynomial in normalized index [-1,1] to points at or below the upper median.
    Polynomial {
        /// Polynomial degree, at most 12; insufficient anchors are errors.
        order: usize,
    },
    /// Index-space Eilers asymmetric least squares with an unscaled D2 penalty.
    Asls {
        /// Positive index-difference penalty, supported range `[1,1e12]`.
        lambda: f64,
        /// Positive-peak weight, supported range `[1e-6,0.5]`.
        asymmetry: f64,
        /// Exact number of reweighted solves, supported range `[1,100]`.
        iterations: usize,
    },
}

/// Complex normalization divisor, shared across fields of each selected-axis trace.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Normalization {
    /// Maximum Cartesian norm; a zero trace returns an explicit no-usable-signal error.
    MaxPeak,
    /// Sum of absolute real values times absolute uniform spacing.
    TotalArea {
        /// Explicit effective width for a singleton, in the axis's unit.
        singleton_width: Option<f64>,
    },
    /// Divide by this finite, nonzero value, including negative values.
    Constant(f64),
}

/// Value aggregation for contiguous bins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinAggregation {
    /// Sum all samples, including a shorter final bin.
    Sum,
    /// Divide each bin by its actual sample count.
    Mean,
}

/// General mathematical spectrum operations. All coordinates remain authoritative.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum SpectrumOperation {
    /// Retain start..end along an axis, preserving physical coordinates and time origin.
    RetainRange {
        /// Inclusive first logical point.
        start: usize,
        /// Exclusive end, greater than start and no larger than the axis length.
        end: usize,
    },
    /// Translate established ppm coordinates and the retained reference by delta.
    Reference {
        /// Signed coordinate shift in ppm.
        delta_ppm: f64,
    },
    /// Reverse intensities on this axis while keeping coordinates fixed.
    Reverse,
    /// Negate every Cartesian field.
    Invert,
    /// Scale every field, then add offset to the all-real field only.
    Affine {
        /// Finite scale.
        scale: f64,
        /// Finite real-channel offset.
        real_offset: f64,
    },
    /// Odd window, truncated at boundaries and divided by actual count.
    MovingAverage {
        /// Requested width; preflight resolves min(max(width,3)|1, largest odd N).
        window: usize,
    },
    /// Local polynomial fit, evaluating full nearest-edge windows at actual offsets.
    SavitzkyGolay {
        /// Requested odd window, normalized as for MovingAverage.
        window: usize,
        /// Requested degree, normalized to [1,window-1], supported up to 12.
        order: usize,
    },
    /// Subtract only the selected-axis real field, preserving independent other fields.
    Baseline(RealBaseline),
    /// Divide corresponding real/imaginary fields by a shared divisor.
    Normalize(Normalization),
    /// Contiguous bins on a uniform axis; output coordinates are exact group means.
    Bin {
        /// Positive width in the current axis unit.
        width: f64,
        /// Sum or mean intensity.
        aggregation: BinAggregation,
    },
    /// Cartesian norm along this axis only; retains all other component fields.
    Magnitude,
    /// Fix a logical position and explicitly select a component of the removed axis.
    /// For a shared complex pair, `component` must be zero: both fields survive
    /// as Cartesian components in the remaining axis's imaginary orientation.
    /// The resulting 1D spectrum can be passed directly to [`PhaseMethod::prepare`].
    Slice {
        /// Checked logical index.
        index: usize,
        /// Checked component on the removed axis; zero for a shared complex pair.
        component: usize,
    },
    /// Sum the removed dimension, selecting its explicit component.
    Sum {
        /// Component of the removed axis; select each to retain separate products.
        component: usize,
    },
    /// Select the original remaining-axis complex value with greatest magnitude.
    /// Equal norms retain the first logical sample in descriptor order.
    Skyline {
        /// Component of the removed axis.
        component: usize,
    },
}

impl SpectrumOperation {
    pub(crate) fn removes_axis(&self) -> bool {
        matches!(
            self,
            Self::Slice { .. } | Self::Sum { .. } | Self::Skyline { .. }
        )
    }
    /// Stable algorithm rule; each name has one mathematical meaning.
    pub fn algorithm_version(&self) -> &'static str {
        match self {
            Self::RetainRange { .. } => "retain-range.v1",
            Self::Reference { .. } => "reference-shift.v1",
            Self::Reverse => "reverse-intensities.v1",
            Self::Invert => "invert.v1",
            Self::Affine { .. } => "affine-spectrum.v1",
            Self::MovingAverage { .. } => "moving-average.v1",
            Self::SavitzkyGolay { .. } => "savitzky-golay.v1",
            Self::Baseline(RealBaseline::Offset) => "real-baseline-offset.v1",
            Self::Baseline(RealBaseline::Polynomial { .. }) => "real-baseline-polynomial.v1",
            Self::Baseline(RealBaseline::Asls { .. }) => "real-baseline-index-asls.v1",
            Self::Normalize(_) => "normalize-spectrum.v1",
            Self::Bin { .. } => "contiguous-bin.v1",
            Self::Magnitude => "axis-magnitude.v1",
            Self::Slice { .. } => "slice-component.v1",
            Self::Sum { .. } => "sum-dimension.v1",
            Self::Skyline { .. } => "skyline-dimension.v1",
        }
    }
}
