use super::ReportError;
use crate::processed::ProcessedDataset;
use crate::{
    acquisition::ComponentBasis,
    axis::{AxisCoordinates, AxisDomain},
};

/// Explicit per-sample tolerances and comparison-output allocation bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComparisonOptions {
    pub(super) absolute: f64,
    pub(super) relative: f64,
    pub(super) max_difference_bytes: usize,
}
impl ComparisonOptions {
    /// Creates finite nonnegative tolerances. These are caller-selected scientific criteria.
    pub fn new(absolute: f64, relative: f64) -> Result<Self, ReportError> {
        if !absolute.is_finite() || !relative.is_finite() || absolute < 0.0 || relative < 0.0 {
            return Err(ReportError::Invalid(
                "tolerances must be finite and nonnegative",
            ));
        }
        Ok(Self {
            absolute,
            relative,
            max_difference_bytes: 64 * 1024 * 1024,
        })
    }
    /// Bounds the returned per-sample difference payload, excluding borrowed inputs.
    pub fn max_difference_bytes(mut self, value: usize) -> Self {
        self.max_difference_bytes = value;
        self
    }
}
/// One difference in canonical storage order, without fitting a global scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleDifference {
    pub(super) absolute: f64,
    pub(super) relative: Option<f64>,
    pub(super) within_tolerance: bool,
}
impl SampleDifference {
    /// Absolute error `abs(actual-reference)`.
    pub fn absolute(&self) -> f64 {
        self.absolute
    }
    /// Absolute error divided by absolute reference; None for a nonzero error at zero reference.
    pub fn relative(&self) -> Option<f64> {
        self.relative
    }
    /// Whether `absolute <= atol + rtol * abs(reference)`.
    pub fn within_tolerance(&self) -> bool {
        self.within_tolerance
    }
}
/// Numerical agreement, separate from canonical identity or history replay authorization.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub(super) differences: Vec<SampleDifference>,
    pub(super) max_absolute: f64,
    pub(super) relative_l2: Option<f64>,
    pub(super) reference_l2: f64,
    pub(super) error_l2: f64,
    pub(super) within_tolerance: bool,
}
impl Comparison {
    /// Per-sample errors in input storage order.
    pub fn differences(&self) -> &[SampleDifference] {
        &self.differences
    }
    /// Largest absolute error.
    pub fn max_absolute(&self) -> f64 {
        self.max_absolute
    }
    /// Error L2 divided by reference L2; None for nonzero error against all-zero reference.
    pub fn relative_l2(&self) -> Option<f64> {
        self.relative_l2
    }
    /// Stable unnormalized L2 norm of the reference.
    pub fn reference_l2(&self) -> f64 {
        self.reference_l2
    }
    /// Stable unnormalized L2 norm of the difference.
    pub fn error_l2(&self) -> f64 {
        self.error_l2
    }
    /// Whether every sample passed the specified combined tolerance.
    pub fn within_tolerance(&self) -> bool {
        self.within_tolerance
    }
}

/// Compares equal, nonempty finite scalar sequences without axis claims.
///
/// Relative error is undefined for a nonzero error at zero reference. Overflow
/// in a reported norm/error is an error, never silently replaced by zero/null.
/// Caller inputs are borrowed; the only variable allocation is the bounded
/// per-sample result. Prefer [`compare_processed`] for scientific dataset checks.
pub fn compare_samples(
    reference: &[f64],
    actual: &[f64],
    options: ComparisonOptions,
) -> Result<Comparison, ReportError> {
    if reference.len() != actual.len() {
        return Err(ReportError::IncompatibleInputs);
    }
    if reference.is_empty() {
        return Err(ReportError::Invalid("empty comparison"));
    }
    if reference
        .iter()
        .chain(actual)
        .any(|value| !value.is_finite())
    {
        return Err(ReportError::NonFinite);
    }
    let bytes = reference
        .len()
        .checked_mul(std::mem::size_of::<SampleDifference>())
        .ok_or(ReportError::Invalid("comparison allocation size overflow"))?;
    if bytes > options.max_difference_bytes {
        return Err(ReportError::LimitExceeded {
            required: bytes,
            limit: options.max_difference_bytes,
        });
    }
    let mut differences = Vec::new();
    differences
        .try_reserve_exact(reference.len())
        .map_err(|_| ReportError::Invalid("comparison allocation failed"))?;
    let mut max_absolute: f64 = 0.0;
    let mut within_tolerance = true;
    for (&expected, &value) in reference.iter().zip(actual) {
        let absolute = (value - expected).abs();
        if !absolute.is_finite() {
            return Err(ReportError::NonFinite);
        }
        let relative = ratio(absolute, expected.abs())?;
        // Division keeps a huge valid tolerance from overflowing its product.
        let within = absolute <= options.absolute
            || expected != 0.0
                && (absolute - options.absolute) / expected.abs() <= options.relative;
        max_absolute = max_absolute.max(absolute);
        within_tolerance &= within;
        differences.push(SampleDifference {
            absolute,
            relative,
            within_tolerance: within,
        });
    }
    let reference_l2 = crate::internal::numeric::scaled_l2(reference.iter().copied());
    let error_l2 =
        crate::internal::numeric::scaled_l2(differences.iter().map(|value| value.absolute));
    if !reference_l2.is_finite() || !error_l2.is_finite() {
        return Err(ReportError::NonFinite);
    }
    Ok(Comparison {
        differences,
        max_absolute,
        relative_l2: ratio(error_l2, reference_l2)?,
        reference_l2,
        error_l2,
        within_tolerance,
    })
}
pub(super) fn ratio(numerator: f64, denominator: f64) -> Result<Option<f64>, ReportError> {
    if denominator == 0.0 {
        return Ok((numerator == 0.0).then_some(0.0));
    }
    let result = numerator / denominator;
    if !result.is_finite() {
        return Err(ReportError::NonFinite);
    }
    Ok(Some(result))
}

/// Compares processed datasets only when complete descriptors match exactly.
///
/// This conservative check includes axes, coordinates, units, components and
/// annotations. It never aligns axes, interpolates, fits phase, or normalizes
/// amplitude. Processing histories may differ; this is a tolerance comparison,
/// not an identity check or authorization to replay against a different source.
pub fn compare_processed(
    reference: &ProcessedDataset,
    actual: &ProcessedDataset,
    options: ComparisonOptions,
) -> Result<Comparison, ReportError> {
    if reference.descriptor() != actual.descriptor()
        || reference.data().shape() != actual.data().shape()
    {
        return Err(ReportError::IncompatibleInputs);
    }
    compare_samples(reference.data().samples(), actual.data().samples(), options)
}

/// Metrics for one caller-specified peak region in a nonnegative scalar 1D frequency spectrum.
#[derive(Clone, Debug, PartialEq)]
pub struct SpectrumComparison {
    pub(super) samples: Comparison,
    pub(super) peak_shift: f64,
    pub(super) peak_area_ratio: f64,
    pub(super) outside_roi_error_relative_l2: f64,
}
impl SpectrumComparison {
    /// Ordinary unscaled per-sample comparison over the complete spectrum.
    pub fn samples(&self) -> &Comparison {
        &self.samples
    }
    /// Candidate maximum coordinate minus reference maximum coordinate inside the ROI, in axis units.
    pub fn peak_shift(&self) -> f64 {
        self.peak_shift
    }
    /// Candidate/reference trapezoidal area within the ROI; absolute coordinate spacing is used.
    pub fn peak_area_ratio(&self) -> f64 {
        self.peak_area_ratio
    }
    /// L2 error outside the ROI divided by the complete reference norm; this measures error leakage.
    pub fn outside_roi_error_relative_l2(&self) -> f64 {
        self.outside_roi_error_relative_l2
    }
}

/// Compares an isolated nonnegative peak with a caller-supplied half-open index ROI.
///
/// Requires matching descriptors, scalar frequency data, known monotone coordinates,
/// at least two ROI points, and positive reference ROI area. Signed/complex spectra,
/// overlapping-peak interpretation, integration baselines and automatic peak finding
/// are outside this metric's domain. Ties select the lowest array index.
pub fn compare_positive_scalar_spectra(
    reference: &ProcessedDataset,
    actual: &ProcessedDataset,
    roi: std::ops::Range<usize>,
    options: ComparisonOptions,
) -> Result<SpectrumComparison, ReportError> {
    let axes = reference.descriptor().axes();
    if axes.len() != 1
        || axes[0].domain() != AxisDomain::Frequency
        || axes[0].component_basis() != &ComponentBasis::Scalar
        || axes[0].unit().is_none()
    {
        return Err(ReportError::Invalid(
            "peak metrics require calibrated scalar 1D frequency data",
        ));
    }
    let a = reference.data().samples();
    let b = actual.data().samples();
    if roi.end > a.len() || roi.start >= roi.end || roi.len() < 2 {
        return Err(ReportError::Invalid(
            "peak ROI needs at least two valid points",
        ));
    }
    if a.iter().chain(b).any(|value| *value < 0.0) {
        return Err(ReportError::Invalid(
            "peak metrics require nonnegative spectra",
        ));
    }
    let samples = compare_processed(reference, actual, options)?;
    let coordinate = |index: usize| -> Result<f64, ReportError> {
        match axes[0].coordinates() {
            AxisCoordinates::Uniform { start, step } => Ok(step.mul_add(index as f64, *start)),
            AxisCoordinates::Explicit(values) => Ok(values[index]),
            AxisCoordinates::Unknown => Err(ReportError::Invalid("unknown peak coordinates")),
        }
    };
    let mut direction: f64 = 0.0;
    for i in 1..a.len() {
        let step = coordinate(i)? - coordinate(i - 1)?;
        if !step.is_finite() || step == 0.0 || direction != 0.0 && step.signum() != direction {
            return Err(ReportError::Invalid(
                "peak coordinates must be finite and strictly monotone",
            ));
        }
        direction = step.signum();
    }
    let peak = |values: &[f64]| {
        let mut best = roi.start;
        for index in roi.clone() {
            if values[index] > values[best] {
                best = index;
            }
        }
        best
    };
    let area = |values: &[f64]| -> Result<f64, ReportError> {
        let mut result = 0.0;
        for i in roi.start + 1..roi.end {
            result += (0.5 * values[i - 1] + 0.5 * values[i])
                * (coordinate(i)? - coordinate(i - 1)?).abs();
        }
        if result.is_finite() {
            Ok(result)
        } else {
            Err(ReportError::NonFinite)
        }
    };
    let reference_area = area(a)?;
    if reference_area == 0.0 {
        return Err(ReportError::Invalid("reference peak area is zero"));
    }
    let peak_area_ratio = ratio(area(b)?, reference_area)?
        .ok_or(ReportError::Invalid("undefined peak area ratio"))?;
    let leakage = crate::internal::numeric::scaled_l2(
        samples
            .differences
            .iter()
            .enumerate()
            .filter(|(i, _)| !roi.contains(i))
            .map(|(_, v)| v.absolute),
    );
    let outside_roi_error_relative_l2 =
        ratio(leakage, samples.reference_l2)?.ok_or(ReportError::Invalid("zero reference norm"))?;
    let peak_shift = coordinate(peak(b))? - coordinate(peak(a))?;
    if !peak_shift.is_finite() {
        return Err(ReportError::NonFinite);
    }
    Ok(SpectrumComparison {
        samples,
        peak_shift,
        peak_area_ratio,
        outside_roi_error_relative_l2,
    })
}
