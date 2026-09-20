use crate::Complex64;
use std::num::NonZeroUsize;
use std::sync::Arc;
use thiserror::Error;

use super::{
    AssertionId, AxisIndex, EvidenceValidationError, NormalizationEvidence, NormalizationFact,
};

use super::registry;

const MIN_RELATIVE_SINGULAR_VALUE: f64 = 1.0e-12;

/// Index used to select a periodic lane multiplier.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModulationIndexDomain {
    /// Absolute coordinate in the full acquisition grid.
    AbsoluteGridCoordinate(AxisIndex),
    /// Unique acquisition-order observation ordinal.
    ObservationOrdinal,
}

/// Finite periodic multipliers stored phase-major, then lane-major.
#[derive(Clone, Debug, PartialEq)]
pub struct PeriodicLaneModulation {
    input_lanes: NonZeroUsize,
    period: NonZeroUsize,
    multipliers: Arc<[Complex64]>,
    domain: ModulationIndexDomain,
    origin: i64,
}

impl PeriodicLaneModulation {
    /// Creates a checked periodic lane modulation.
    pub fn try_new(
        input_lanes: usize,
        period: usize,
        multipliers: impl Into<Arc<[Complex64]>>,
        domain: ModulationIndexDomain,
        origin: i64,
    ) -> Result<Self, TransformValidationError> {
        let input_lanes =
            NonZeroUsize::new(input_lanes).ok_or(TransformValidationError::ZeroInputLanes)?;
        let period = NonZeroUsize::new(period).ok_or(TransformValidationError::ZeroPeriod)?;
        let expected = period
            .get()
            .checked_mul(input_lanes.get())
            .ok_or(TransformValidationError::SizeOverflow)?;
        let multipliers = multipliers.into();
        if multipliers.len() != expected {
            return Err(TransformValidationError::ModulationLength {
                expected,
                actual: multipliers.len(),
            });
        }
        if multipliers
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(TransformValidationError::NonFiniteMultiplier);
        }
        Ok(Self {
            input_lanes,
            period,
            multipliers,
            domain,
            origin,
        })
    }

    /// Creates a period-one identity modulation.
    /// Rejects unrepresentable storage sizes before allocating. The temporary
    /// vector uses fallible reservation; conversion to shared storage still
    /// follows Rust's allocator failure policy.
    pub fn identity(input_lanes: usize) -> Result<Self, TransformValidationError> {
        if input_lanes == 0 {
            return Err(TransformValidationError::ZeroInputLanes);
        }
        // Include the two reference counters in the eventual Arc allocation.
        let bytes = input_lanes
            .checked_mul(std::mem::size_of::<Complex64>())
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .ok_or(TransformValidationError::SizeOverflow)?;
        if bytes > isize::MAX as usize {
            return Err(TransformValidationError::SizeOverflow);
        }
        let mut multipliers = Vec::new();
        multipliers
            .try_reserve_exact(input_lanes)
            .map_err(|_| TransformValidationError::AllocationFailed)?;
        multipliers.resize(input_lanes, Complex64::new(1.0, 0.0));
        Self::try_new(
            input_lanes,
            1,
            multipliers,
            ModulationIndexDomain::ObservationOrdinal,
            0,
        )
    }

    /// Returns the number of input lanes.
    pub const fn input_lanes(&self) -> usize {
        self.input_lanes.get()
    }

    /// Returns the modulation period.
    pub const fn period(&self) -> usize {
        self.period.get()
    }

    /// Returns phase-major, lane-major multipliers.
    pub fn multipliers(&self) -> &[Complex64] {
        &self.multipliers
    }

    /// Returns the modulation index domain.
    pub const fn domain(&self) -> ModulationIndexDomain {
        self.domain
    }

    /// Returns the signed phase origin.
    pub const fn origin(&self) -> i64 {
        self.origin
    }

    fn multiplier(&self, index: i64, lane: usize) -> Complex64 {
        let phase = (i128::from(index) - i128::from(self.origin))
            .rem_euclid(self.period.get() as i128) as usize;
        self.multipliers[phase * self.input_lanes.get() + lane]
    }
}

/// A checked two-row complex component transform.
#[derive(Clone, Debug, PartialEq)]
pub struct LinearComponentTransform {
    input_lanes: NonZeroUsize,
    coefficients: Arc<[Complex64]>,
    modulation: PeriodicLaneModulation,
}

impl LinearComponentTransform {
    /// Creates a finite, complete, numerically full-row-rank transform.
    pub fn try_new(
        input_lanes: usize,
        coefficients: impl Into<Arc<[Complex64]>>,
        modulation: PeriodicLaneModulation,
    ) -> Result<Self, TransformValidationError> {
        let input_lanes =
            NonZeroUsize::new(input_lanes).ok_or(TransformValidationError::ZeroInputLanes)?;
        if modulation.input_lanes != input_lanes {
            return Err(TransformValidationError::ModulationLaneMismatch);
        }
        let expected = input_lanes
            .get()
            .checked_mul(2)
            .ok_or(TransformValidationError::SizeOverflow)?;
        let coefficients = coefficients.into();
        if coefficients.len() != expected {
            return Err(TransformValidationError::CoefficientLength {
                expected,
                actual: coefficients.len(),
            });
        }
        if coefficients
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(TransformValidationError::NonFiniteCoefficient);
        }
        validate_full_row_rank(&coefficients, input_lanes.get())?;
        Ok(Self {
            input_lanes,
            coefficients,
            modulation,
        })
    }

    /// Returns the number of raw input lanes.
    pub const fn input_lanes(&self) -> usize {
        self.input_lanes.get()
    }

    /// Returns row-major coefficients for two output rows.
    pub fn coefficients(&self) -> &[Complex64] {
        &self.coefficients
    }

    /// Returns the periodic lane modulation.
    pub fn modulation(&self) -> &PeriodicLaneModulation {
        &self.modulation
    }

    /// Applies the transform with a caller-resolved modulation index.
    ///
    /// Lanes are accumulated in strictly increasing order.
    pub fn apply(
        &self,
        input: &[Complex64],
        modulation_index: i64,
    ) -> Result<[Complex64; 2], TransformValidationError> {
        if input.len() != self.input_lanes.get() {
            return Err(TransformValidationError::InputLength {
                expected: self.input_lanes.get(),
                actual: input.len(),
            });
        }
        let mut output = [Complex64::new(0.0, 0.0); 2];
        for (row, result) in output.iter_mut().enumerate() {
            for (lane, value) in input.iter().copied().enumerate() {
                *result += self.coefficients[row * self.input_lanes.get() + lane]
                    * self.modulation.multiplier(modulation_index, lane)
                    * value;
            }
        }
        Ok(output)
    }
}

/// Evidence-bearing resolved transform with constant-cost cloning.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedComponentTransform(Arc<ResolvedTransformInner>);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedTransformInner {
    transform: LinearComponentTransform,
    evidence: NormalizationEvidence,
}

impl ResolvedComponentTransform {
    /// Shared wrapper only; coefficient, modulation and evidence arrays are separate.
    pub(crate) const fn wrapper_allocation_bytes() -> usize {
        std::mem::size_of::<ResolvedTransformInner>() + 3 * std::mem::size_of::<usize>()
    }

    /// Wraps a checked user transform and marks its authority as user constructed.
    pub fn user_constructed(
        transform: LinearComponentTransform,
    ) -> Result<Self, EvidenceValidationError> {
        let lanes = NonZeroUsize::new(transform.input_lanes())
            .expect("a checked transform has nonzero lanes");
        let evidence = NormalizationEvidence::user_constructed(vec![
            NormalizationFact::CanonicalLaneOrder { lanes },
            NormalizationFact::UserDefinedComponentTransform,
            NormalizationFact::PeriodicModulation {
                period: transform.modulation.period,
                domain: transform.modulation.domain,
            },
        ])?;
        Ok(Self(Arc::new(ResolvedTransformInner {
            transform,
            evidence,
        })))
    }

    pub(crate) fn resolved(
        transform: LinearComponentTransform,
        evidence: NormalizationEvidence,
    ) -> Self {
        Self(Arc::new(ResolvedTransformInner {
            transform,
            evidence,
        }))
    }

    pub(crate) fn from_assertion(
        transform: LinearComponentTransform,
        assertion: AssertionId,
    ) -> Result<Self, EvidenceValidationError> {
        let lanes = NonZeroUsize::new(transform.input_lanes()).expect("checked nonzero lanes");
        let evidence = NormalizationEvidence::from_assertion(
            assertion,
            vec![
                NormalizationFact::CanonicalLaneOrder { lanes },
                NormalizationFact::TraceMappingBijection,
                NormalizationFact::UserDefinedComponentTransform,
            ],
            vec![registry::CALLER_ASSERTION_V1],
        )?;
        Ok(Self::resolved(transform, evidence))
    }

    /// Returns the checked linear transform.
    pub fn transform(&self) -> &LinearComponentTransform {
        &self.0.transform
    }

    /// Returns its normalization evidence.
    pub fn evidence(&self) -> &NormalizationEvidence {
        &self.0.evidence
    }

    /// Returns the number of raw input lanes.
    pub fn input_lanes(&self) -> usize {
        self.0.transform.input_lanes()
    }
}

/// Invalid linear component transform or modulation.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TransformValidationError {
    /// The transform has no input lanes.
    #[error("component transform has zero input lanes")]
    ZeroInputLanes,
    /// The modulation period is zero.
    #[error("component modulation has zero period")]
    ZeroPeriod,
    /// A checked size computation overflowed.
    #[error("component transform size computation overflowed")]
    SizeOverflow,
    /// Temporary component storage could not be reserved.
    #[error("component storage allocation failed")]
    AllocationFailed,
    /// Coefficient storage is not exactly two complete rows.
    #[error("component coefficient length mismatch: expected {expected}, found {actual}")]
    CoefficientLength {
        /// Required coefficient count.
        expected: usize,
        /// Supplied coefficient count.
        actual: usize,
    },
    /// Modulation storage does not contain `period * lanes` values.
    #[error("component modulation length mismatch: expected {expected}, found {actual}")]
    ModulationLength {
        /// Required multiplier count.
        expected: usize,
        /// Supplied multiplier count.
        actual: usize,
    },
    /// Transform and modulation lane counts disagree.
    #[error("component transform and modulation lane counts disagree")]
    ModulationLaneMismatch,
    /// A coefficient contains a non-finite component.
    #[error("component transform contains a non-finite coefficient")]
    NonFiniteCoefficient,
    /// A modulation multiplier contains a non-finite component.
    #[error("component modulation contains a non-finite multiplier")]
    NonFiniteMultiplier,
    /// The two transform rows are numerically rank deficient.
    #[error("component transform is not full row rank")]
    RankDeficient,
    /// Input storage does not match the transform lane count.
    #[error("component transform input length mismatch: expected {expected}, found {actual}")]
    InputLength {
        /// Required lane count.
        expected: usize,
        /// Supplied lane count.
        actual: usize,
    },
}

fn validate_full_row_rank(
    coefficients: &[Complex64],
    lanes: usize,
) -> Result<(), TransformValidationError> {
    let scale = coefficients
        .iter()
        .flat_map(|value| [value.re.abs(), value.im.abs()])
        .fold(0.0_f64, f64::max);
    if scale == 0.0 {
        return Err(TransformValidationError::RankDeficient);
    }
    let mut row0 = 0.0;
    let mut row1 = 0.0;
    let mut cross = Complex64::new(0.0, 0.0);
    for lane in 0..lanes {
        let left = coefficients[lane] / scale;
        let right = coefficients[lanes + lane] / scale;
        row0 += left.norm_sqr();
        row1 += right.norm_sqr();
        cross += left * right.conj();
    }
    let trace = row0 + row1;
    let discriminant = (row0 - row1).hypot(2.0 * cross.norm());
    let lambda_max = 0.5 * (trace + discriminant);
    if !lambda_max.is_finite() || lambda_max <= 0.0 {
        return Err(TransformValidationError::RankDeficient);
    }

    // For two rows, det(M M^H) is the sum of squared 2x2 minors. Computing it
    // this way avoids cancellation in `trace - discriminant` near the 1e-12
    // singular-value threshold. Since det = sigma_max^2 * sigma_min^2,
    // sqrt(det) / lambda_max is sigma_min / sigma_max.
    let mut determinant = 0.0;
    let mut correction = 0.0;
    for left_lane in 0..lanes {
        let a_left = coefficients[left_lane] / scale;
        let b_left = coefficients[lanes + left_lane] / scale;
        for right_lane in left_lane + 1..lanes {
            let minor = a_left * (coefficients[lanes + right_lane] / scale)
                - (coefficients[right_lane] / scale) * b_left;
            let term = minor.norm_sqr();
            let sum = determinant + term;
            correction += if determinant.abs() >= term.abs() {
                (determinant - sum) + term
            } else {
                (term - sum) + determinant
            };
            determinant = sum;
        }
    }
    let ratio = (determinant + correction).max(0.0).sqrt() / lambda_max;
    if !ratio.is_finite() || ratio < MIN_RELATIVE_SINGULAR_VALUE {
        return Err(TransformValidationError::RankDeficient);
    }
    Ok(())
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl LinearComponentTransform {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (&NonZeroUsize, &Arc<[Complex64]>, &PeriodicLaneModulation) {
        (&self.input_lanes, &self.coefficients, &self.modulation)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (NonZeroUsize, Arc<[Complex64]>, PeriodicLaneModulation),
    ) -> Result<Self, crate::internal::ModelError> {
        let (input_lanes, coefficients, modulation) = parts;
        let value = Self {
            input_lanes,
            coefficients,
            modulation,
        };

        let value = Self::try_new(
            value.input_lanes.get(),
            value.coefficients,
            value.modulation,
        )
        .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl PeriodicLaneModulation {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &NonZeroUsize,
        &NonZeroUsize,
        &Arc<[Complex64]>,
        &ModulationIndexDomain,
        &i64,
    ) {
        (
            &self.input_lanes,
            &self.period,
            &self.multipliers,
            &self.domain,
            &self.origin,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            NonZeroUsize,
            NonZeroUsize,
            Arc<[Complex64]>,
            ModulationIndexDomain,
            i64,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (input_lanes, period, multipliers, domain, origin) = parts;
        let value = Self {
            input_lanes,
            period,
            multipliers,
            domain,
            origin,
        };

        let value = Self::try_new(
            value.input_lanes.get(),
            value.period.get(),
            value.multipliers,
            value.domain,
            value.origin,
        )
        .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ResolvedComponentTransform {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Arc<ResolvedTransformInner>,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Arc<ResolvedTransformInner>,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ResolvedTransformInner {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&LinearComponentTransform, &NormalizationEvidence) {
        (&self.transform, &self.evidence)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (LinearComponentTransform, NormalizationEvidence),
    ) -> Result<Self, crate::internal::ModelError> {
        let (transform, evidence) = parts;
        let value = Self {
            transform,
            evidence,
        };

        Ok(value)
    }
}
