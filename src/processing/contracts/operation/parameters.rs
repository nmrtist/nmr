use crate::processing::contracts::error::ProcessingError;

/// Sign of the exponential in an unnormalized discrete Fourier transform.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FourierExponentSign {
    /// Uses `exp(-i 2 pi n q / N)`.
    #[default]
    Negative,
    /// Uses `exp(+i 2 pi n q / N)`, matching the NMRPipe exponent convention.
    Positive,
}

/// A window request, validated authoritatively during plan preflight.
///
/// Convenience constructors check intrinsic parameter values. Public variants
/// also allow describing requests before validation, so this enum alone is not
/// proof that parameters are valid. Existing variant fields are an intentional
/// construction contract; new configurable profiles use new variants.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Window {
    /// A powered sine bell with an optional first-point multiplier.
    SineBell {
        /// Fraction of pi at the first point.
        offset: f64,
        /// Fraction of pi at the right edge.
        end: f64,
        /// Positive sine-bell power.
        power: f64,
        /// Non-negative multiplier applied only to the first point.
        first_point_scale: f64,
    },
    /// Lorentz-to-Gauss: exp(pi*lb*t - (pi*gb)^2*t^2/(4*ln(2))).
    /// Time is elapsed from the first sample, in seconds. Positive lb narrows
    /// a Lorentzian; gb is the applied Gaussian frequency FWHM in Hz.
    LorentzToGauss {
        /// Signed Lorentzian narrowing in Hz.
        lb_hz: f64,
        /// Nonnegative Gaussian FWHM in Hz.
        gb_hz: f64,
    },
    /// Exponential line broadening in Hz.
    Exponential {
        /// Signed line broadening in Hz.
        lb_hz: f64,
    },
}

/// End-only zero filling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZeroFill {
    pub(in crate::processing) target_points: usize,
}

/// Unnormalized forward time-to-frequency transform parameters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FourierTransform {
    pub(in crate::processing) sign: FourierExponentSign,
}

/// Source of a delay-phase-ramp value.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DelaySource {
    /// Uses the checked delay evidence attached to the selected axis.
    AxisEvidence,
    /// Uses an explicit delay in points.
    Explicit(f64),
}

/// Residual behavior for the versioned time-domain shift/fold algorithm.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeDomainResidualPolicy {
    /// Applies the complete fractional delay and retains its evidence as corrected.
    CorrectFully,
    /// Applies the integer part and retains a nonzero fractional remainder.
    IntegerOnlyRetainResidual,
}

/// Digital-filter correction profiles.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DigitalFilterCorrection {
    /// Records that checked pending zero-delay evidence needs no numeric correction.
    AcknowledgeZeroDelayV1,
    /// Corrects a delay on a canonical frequency axis produced by this crate.
    /// Must precede axis reversal, which invalidates the FFT bin mapping.
    FrequencyDomainPhaseRampV1(DelaySource),
    /// Applies the versioned time-domain shift, fold, and skip algorithm.
    TimeDomainShiftFoldV1 {
        /// Source of the delay value.
        source: DelaySource,
        /// Treatment of a fractional residual.
        policy: TimeDomainResidualPolicy,
    },
}

/// Zero- and first-order phase correction in degrees.
///
/// For the current axis length `N` and zero-based array index `k`, samples are
/// multiplied by `exp(i * pi/180 * (p0 + p1 * (k/N - pivot)))`. The denominator
/// is `N`, not `N - 1`; zero filling therefore changes the index-to-phase mapping.
/// Angles are in degrees and the pivot is a fraction of the full array width.
/// After reversing an axis, the same physical phase function at the same pivot
/// uses `p1_new = -p1` and
/// `p0_new = p0 + p1 * ((N - 1)/N - 2*pivot)`.
/// Applying a phase rotation records the mathematical operation; it does not
/// establish that the result is a correctly phased absorption spectrum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseCorrection {
    pub(in crate::processing) p0_degrees: f64,
    pub(in crate::processing) p1_degrees: f64,
    pub(in crate::processing) pivot_fraction: f64,
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl FourierTransform {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&FourierExponentSign,) {
        (&self.sign,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (FourierExponentSign,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (sign,) = parts;
        let value = Self { sign };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl PhaseCorrection {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&f64, &f64, &f64) {
        (&self.p0_degrees, &self.p1_degrees, &self.pivot_fraction)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (f64, f64, f64),
    ) -> Result<Self, crate::internal::ModelError> {
        let (p0_degrees, p1_degrees, pivot_fraction) = parts;
        let value = Self {
            p0_degrees,
            p1_degrees,
            pivot_fraction,
        };

        let value = Self::new(value.p0_degrees, value.p1_degrees, value.pivot_fraction)
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ZeroFill {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&usize,) {
        (&self.target_points,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(parts: (usize,)) -> Result<Self, crate::internal::ModelError> {
        let (target_points,) = parts;
        let value = Self { target_points };

        Ok(value)
    }
}

impl Window {
    /// Creates the explicit Lorentz-to-Gauss window, with widths in Hz.
    pub fn lorentz_to_gauss(lb_hz: f64, gb_hz: f64) -> Result<Self, ProcessingError> {
        let window = Self::LorentzToGauss { lb_hz, gb_hz };
        window
            .validate()
            .map_err(ProcessingError::InvalidParameter)?;
        Ok(window)
    }

    /// Creates a checked sine-bell window.
    pub fn sine_bell(
        offset: f64,
        end: f64,
        power: f64,
        first_point_scale: f64,
    ) -> Result<Self, ProcessingError> {
        let window = Self::SineBell {
            offset,
            end,
            power,
            first_point_scale,
        };
        window
            .validate()
            .map_err(ProcessingError::InvalidParameter)?;
        Ok(window)
    }

    /// Creates an exponential window with signed line broadening in Hz.
    ///
    /// Nonzero broadening requires positive uniform time coordinates. With
    /// dwell `dt`, the effective width `sw` is the supplied spectral width
    /// or `1/dt` when absent. Point `k` is multiplied by
    /// `exp(-pi * lb_hz * k / sw)`, relative to the first retained sample,
    /// irrespective of its absolute time origin. If spectral width is present,
    /// `abs(sw * dt - 1) <= 1e-9` must hold. Explicit or unknown coordinates
    /// are rejected for nonzero broadening. Zero broadening is an identity,
    /// requires no time calibration, and remains recorded in history.
    pub fn exponential(lb_hz: f64) -> Result<Self, ProcessingError> {
        let window = Self::Exponential { lb_hz };
        window
            .validate()
            .map_err(ProcessingError::InvalidParameter)?;
        Ok(window)
    }

    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match *self {
            Self::SineBell {
                offset,
                end,
                power,
                first_point_scale,
            } if !offset.is_finite()
                || !end.is_finite()
                || !(0.0..=1.0).contains(&offset)
                || !(0.0..=1.0).contains(&end)
                || !power.is_finite()
                || power <= 0.0
                || !first_point_scale.is_finite()
                || first_point_scale < 0.0 =>
            {
                Err("sine-bell window")
            }
            Self::LorentzToGauss { lb_hz, gb_hz }
                if !lb_hz.is_finite() || !gb_hz.is_finite() || gb_hz < 0.0 =>
            {
                Err("Lorentz-to-Gauss widths")
            }
            Self::Exponential { lb_hz } if !lb_hz.is_finite() => Err("exponential line broadening"),
            _ => Ok(()),
        }
    }
}

impl ZeroFill {
    /// Creates a target point count. A plan rejects shrinking an axis.
    /// An unchanged count is an identity retained in history; growth requires
    /// positive uniform time coordinates to extend the acquisition grid.
    pub fn new(target_points: usize) -> Result<Self, ProcessingError> {
        if target_points == 0 {
            return Err(ProcessingError::InvalidParameter("zero-fill target"));
        }
        Ok(Self { target_points })
    }

    /// Returns the requested point count.
    pub fn target_points(self) -> usize {
        self.target_points
    }
}

impl FourierTransform {
    /// Creates an unnormalized FFT with the requested exponential sign.
    ///
    /// For `sigma = -1` (negative) or `+1` (positive), centered output bin
    /// `q = k - floor(N/2)` is `sum_n x[n] * exp(sigma * i * 2*pi*n*q/N)`.
    /// `N` is the current length, including any prior zero filling. A nonzero
    /// input origin `t0` additionally multiplies that bin by
    /// `exp(sigma * i * 2*pi*(q*sw/N)*t0)`. Coordinates are `q*sw/N` Hz.
    /// For even `N`, the retained Nyquist bin is `-sw/2`. No normalization or
    /// implicit first-point weighting is applied; request window scaling explicitly.
    pub fn new(sign: FourierExponentSign) -> Self {
        Self { sign }
    }

    /// Returns the exponential sign.
    pub fn sign(self) -> FourierExponentSign {
        self.sign
    }
}

impl Default for FourierTransform {
    fn default() -> Self {
        Self::new(FourierExponentSign::Negative)
    }
}

impl PhaseCorrection {
    /// Creates a zero-order rotation explicitly measured in degrees.
    /// The first-order term and array-width pivot initially equal zero.
    pub fn zero_order_degrees(p0_degrees: f64) -> Result<Self, ProcessingError> {
        Self::new(p0_degrees, 0.0, 0.0)
    }

    /// Sets the first-order phase in degrees over the full array width.
    pub fn with_first_order_degrees(self, p1_degrees: f64) -> Result<Self, ProcessingError> {
        Self::new(self.p0_degrees, p1_degrees, self.pivot_fraction)
    }

    /// Sets the pivot as a fraction of the array width, checked in `[0, 1]`.
    pub fn with_pivot_fraction(self, pivot_fraction: f64) -> Result<Self, ProcessingError> {
        Self::new(self.p0_degrees, self.p1_degrees, pivot_fraction)
    }

    /// Creates checked phase parameters for a Cartesian frequency axis.
    ///
    /// Current array point `k` is multiplied by
    /// `exp(i*pi/180 * (p0 + p1*(k/N - pivot)))`. The denominator is the
    /// current length `N`, not `N-1`, including prior zero filling. The pivot
    /// is an array-width fraction, not a physical Hz or ppm coordinate.
    /// Applying a rotation records that operation, not proof of correct phase.
    ///
    /// After reversing an axis, the same physical phase function uses
    /// `p1' = -p1` and `p0' = p0 + p1*((N-1)/N - 2*pivot)` at the same pivot.
    pub fn new(
        p0_degrees: f64,
        p1_degrees: f64,
        pivot_fraction: f64,
    ) -> Result<Self, ProcessingError> {
        if !p0_degrees.is_finite()
            || !p1_degrees.is_finite()
            || !pivot_fraction.is_finite()
            || !(0.0..=1.0).contains(&pivot_fraction)
        {
            return Err(ProcessingError::InvalidParameter("phase correction"));
        }
        Ok(Self {
            p0_degrees,
            p1_degrees,
            pivot_fraction,
        })
    }

    /// Returns the zero-order phase in degrees.
    pub fn p0_degrees(self) -> f64 {
        self.p0_degrees
    }

    /// Returns the first-order phase in degrees.
    pub fn p1_degrees(self) -> f64 {
        self.p1_degrees
    }

    /// Returns the pivot as a fraction of the full width.
    pub fn pivot_fraction(self) -> f64 {
        self.pivot_fraction
    }
}
