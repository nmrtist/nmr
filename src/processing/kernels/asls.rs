//! Fixed asymmetric least-squares baseline correction.

use crate::execution::{ExecutionContext, ExecutionError};
use crate::internal::numeric::scaled_l2;
use crate::processing::contracts::profile::PositivePeaksV1;
use thiserror::Error;

const LAMBDA: f64 = 1_000_000.0;
const REFERENCE_INTERVALS: f64 = 2048.0;
const ASYMMETRY: f64 = 0.001;
const MAX_SOLVES: usize = 50;
const TOLERANCE: f64 = 1e-6;
const MIN_SCALED_PIVOT: f64 = 64.0 * f64::EPSILON;

impl PositivePeaksV1 {
    /// Smoothing strength at 2048 intervals across the normalized spectral window.
    ///
    /// The effective index-grid penalty is `LAMBDA * ((n - 1) / 2048)^4`.
    pub const LAMBDA: f64 = LAMBDA;

    /// Fixed positive-peak asymmetry weight.
    pub const ASYMMETRY: f64 = ASYMMETRY;

    /// Maximum number of banded solves, including the initial solve.
    pub const MAX_SOLVES: usize = MAX_SOLVES;

    /// Fixed weighted relative L2 convergence tolerance.
    pub const TOLERANCE: f64 = TOLERANCE;

    /// Subtracts the fitted baseline from one finite scalar trace.
    ///
    /// Smoothing is defined on the full normalized coordinate interval, so
    /// resampling the same window preserves the continuous smoothing scale.
    /// Cropping the window changes that scale. This experimental profile is
    /// validated for resolved, narrow positive peaks, not broad or negative peaks.
    /// The release gate covers 129â€“8193 samples over 1200 Hz, a Gaussian
    /// `20 exp(-((x-530)/22)^2)` above a slowly curved baseline: sampled peak
    /// height error <2%, peak-region area error <5%, and relative L2 error <5%.
    /// Doubling that width produces about 9% area bias and has no quantitative
    /// guarantee, even though its result is stable under resampling.
    pub fn subtract(self, coordinates_hz: &[f64], samples: &[f64]) -> Result<Vec<f64>, AslsError> {
        self.subtract_with_context(coordinates_hz, samples, &mut ExecutionContext::default())
    }
    /// Fits a baseline with cooperative cancellation and shared work accounting.
    pub fn subtract_with_context(
        self,
        coordinates_hz: &[f64],
        samples: &[f64],
        control: &mut ExecutionContext<'_>,
    ) -> Result<Vec<f64>, AslsError> {
        control.begin(crate::execution::ExecutionStage::Processing, None, None)?;
        self.subtract_controlled(coordinates_hz, samples, control)
    }
    pub(crate) fn subtract_controlled(
        self,
        coordinates_hz: &[f64],
        samples: &[f64],
        control: &mut ExecutionContext<'_>,
    ) -> Result<Vec<f64>, AslsError> {
        control.ensure_work((samples.len() as u128) * MAX_SOLVES as u128)?;

        validate_input(control, coordinates_hz, samples)?;
        let normalized = normalized_coordinates(control, coordinates_hz)?;
        let trapezoid = trapezoid_weights(control, &normalized)?;
        let penalty = penalty_bands(control, &normalized)?;
        let mut asymmetric = filled(samples.len(), 1.0)?;
        let mut previous: Option<Vec<f64>> = None;

        for _ in 0..MAX_SOLVES {
            control.charge(samples.len() as u128)?;
            let baseline = solve(control, samples, &trapezoid, &asymmetric, &penalty)?;
            if let Some(old) = previous.as_ref() {
                let change = scaled_l2((0..samples.len()).map(|index| {
                    (trapezoid[index] * asymmetric[index]).sqrt() * (baseline[index] - old[index])
                }));
                let reference = scaled_l2(
                    (0..samples.len())
                        .map(|index| (trapezoid[index] * asymmetric[index]).sqrt() * old[index]),
                );
                if !change.is_finite() || !reference.is_finite() {
                    return Err(AslsError::NumericalInvariantViolation);
                }
                if change <= TOLERANCE * reference.max(f64::MIN_POSITIVE) {
                    control.complete_work()?;
                    return subtract_baseline(control, samples, &baseline);
                }
            }
            for index in 0..samples.len() {
                if index % 4096 == 0 {
                    control.check_cancelled()?;
                }
                asymmetric[index] = if samples[index] > baseline[index] {
                    ASYMMETRY
                } else {
                    1.0 - ASYMMETRY
                };
            }
            previous = Some(baseline);
        }
        Err(AslsError::DidNotConverge)
    }
}

#[derive(Clone, Debug)]
struct PenaltyBands {
    diagonal: Vec<f64>,
    first: Vec<f64>,
    second: Vec<f64>,
}

fn validate_input(
    control: &mut ExecutionContext<'_>,
    coordinates: &[f64],
    samples: &[f64],
) -> Result<(), AslsError> {
    control.check_cancelled()?;
    if coordinates.len() != samples.len() || samples.len() < 3 {
        return Err(AslsError::InvalidLength);
    }
    if coordinates
        .iter()
        .chain(samples)
        .any(|value| !value.is_finite())
    {
        return Err(AslsError::NonFiniteInput);
    }
    if coordinates.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(AslsError::NonMonotonicCoordinates);
    }
    Ok(())
}

fn normalized_coordinates(
    control: &mut ExecutionContext<'_>,
    coordinates: &[f64],
) -> Result<Vec<f64>, AslsError> {
    control.check_cancelled()?;
    let span = coordinates[coordinates.len() - 1] - coordinates[0];
    if !span.is_finite() || span <= 0.0 {
        return Err(AslsError::NonMonotonicCoordinates);
    }
    let mut normalized = reserved(coordinates.len())?;
    for value in coordinates {
        let value = (*value - coordinates[0]) / span;
        if !value.is_finite() {
            return Err(AslsError::NumericalInvariantViolation);
        }
        normalized.push(value);
    }
    Ok(normalized)
}

fn trapezoid_weights(
    control: &mut ExecutionContext<'_>,
    coordinates: &[f64],
) -> Result<Vec<f64>, AslsError> {
    control.check_cancelled()?;
    let points = coordinates.len();
    let nominal = 1.0 / (points - 1) as f64;
    let mut weights = filled(points, 0.0)?;
    weights[0] = (coordinates[1] - coordinates[0]) / (2.0 * nominal);
    weights[points - 1] = (coordinates[points - 1] - coordinates[points - 2]) / (2.0 * nominal);
    for index in 1..points - 1 {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        weights[index] = (coordinates[index + 1] - coordinates[index - 1]) / (2.0 * nominal);
    }
    if weights
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(AslsError::NumericalInvariantViolation);
    }
    Ok(weights)
}

fn penalty_bands(
    control: &mut ExecutionContext<'_>,
    coordinates: &[f64],
) -> Result<PenaltyBands, AslsError> {
    control.check_cancelled()?;
    let points = coordinates.len();
    let nominal = 1.0 / (points - 1) as f64;
    let mut bands = PenaltyBands {
        diagonal: filled(points, 0.0)?,
        first: filled(points - 1, 0.0)?,
        second: filled(points - 2, 0.0)?,
    };
    for center in 1..points - 1 {
        if center % 4096 == 0 {
            control.check_cancelled()?;
        }
        let left = coordinates[center] - coordinates[center - 1];
        let right = coordinates[center + 1] - coordinates[center];
        let row_weight = (left + right) / (2.0 * nominal);
        let coefficients = [
            2.0 / (REFERENCE_INTERVALS.powi(2) * left * (left + right)),
            -2.0 / (REFERENCE_INTERVALS.powi(2) * left * right),
            2.0 / (REFERENCE_INTERVALS.powi(2) * right * (left + right)),
        ];
        if coefficients.iter().any(|value| !value.is_finite()) {
            return Err(AslsError::NumericalInvariantViolation);
        }
        let start = center - 1;
        for (offset, coefficient) in coefficients.iter().enumerate() {
            bands.diagonal[start + offset] += LAMBDA * row_weight * coefficient * coefficient;
        }
        bands.first[start] += LAMBDA * row_weight * coefficients[0] * coefficients[1];
        bands.first[start + 1] += LAMBDA * row_weight * coefficients[1] * coefficients[2];
        bands.second[start] += LAMBDA * row_weight * coefficients[0] * coefficients[2];
    }
    if bands
        .diagonal
        .iter()
        .chain(&bands.first)
        .chain(&bands.second)
        .any(|value| !value.is_finite())
    {
        return Err(AslsError::NumericalInvariantViolation);
    }
    Ok(bands)
}

fn solve(
    control: &mut ExecutionContext<'_>,
    samples: &[f64],
    trapezoid: &[f64],
    asymmetric: &[f64],
    penalty: &PenaltyBands,
) -> Result<Vec<f64>, AslsError> {
    control.check_cancelled()?;
    let points = samples.len();
    let mut diagonal = copied(&penalty.diagonal)?;
    let mut rhs = filled(points, 0.0)?;
    for index in 0..points {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        let weight = trapezoid[index] * asymmetric[index];
        diagonal[index] += weight;
        rhs[index] = weight * samples[index];
    }
    let scale = diagonal
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .fold(0.0, f64::max);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(AslsError::NonPositiveDefinite);
    }
    for value in &mut diagonal {
        *value /= scale;
    }
    let mut first = copied(&penalty.first)?;
    let mut second = copied(&penalty.second)?;
    for value in first.iter_mut().chain(&mut second).chain(&mut rhs) {
        *value /= scale;
        if !value.is_finite() {
            return Err(AslsError::NumericalInvariantViolation);
        }
    }
    cholesky_solve(control, diagonal, first, second, rhs)
}

fn cholesky_solve(
    control: &mut ExecutionContext<'_>,
    diagonal: Vec<f64>,
    first: Vec<f64>,
    second: Vec<f64>,
    rhs: Vec<f64>,
) -> Result<Vec<f64>, AslsError> {
    control.check_cancelled()?;
    let points = diagonal.len();
    let mut l0 = filled(points, 0.0)?;
    let mut l1 = filled(points - 1, 0.0)?;
    let mut l2 = filled(points - 2, 0.0)?;
    for index in 0..points {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        if index >= 2 {
            l2[index - 2] = second[index - 2] / l0[index - 2];
        }
        if index >= 1 {
            let shared = if index >= 2 {
                l2[index - 2] * l1[index - 2]
            } else {
                0.0
            };
            l1[index - 1] = (first[index - 1] - shared) / l0[index - 1];
        }
        let mut pivot = diagonal[index];
        if index >= 1 {
            pivot -= l1[index - 1] * l1[index - 1];
        }
        if index >= 2 {
            pivot -= l2[index - 2] * l2[index - 2];
        }
        if !pivot.is_finite() {
            return Err(AslsError::NumericalInvariantViolation);
        }
        if pivot <= MIN_SCALED_PIVOT {
            return Err(AslsError::NonPositiveDefinite);
        }
        l0[index] = pivot.sqrt();
    }

    let mut intermediate = filled(points, 0.0)?;
    for index in 0..points {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        let mut value = rhs[index];
        if index >= 1 {
            value -= l1[index - 1] * intermediate[index - 1];
        }
        if index >= 2 {
            value -= l2[index - 2] * intermediate[index - 2];
        }
        intermediate[index] = value / l0[index];
    }
    let mut solution = filled(points, 0.0)?;
    for index in (0..points).rev() {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        let mut value = intermediate[index];
        if index + 1 < points {
            value -= l1[index] * solution[index + 1];
        }
        if index + 2 < points {
            value -= l2[index] * solution[index + 2];
        }
        solution[index] = value / l0[index];
        if !solution[index].is_finite() {
            return Err(AslsError::NumericalInvariantViolation);
        }
    }
    Ok(solution)
}

fn subtract_baseline(
    control: &mut ExecutionContext<'_>,
    samples: &[f64],
    baseline: &[f64],
) -> Result<Vec<f64>, AslsError> {
    control.check_cancelled()?;
    let mut corrected = reserved(samples.len())?;
    for (&sample, &base) in samples.iter().zip(baseline) {
        let value = sample - base;
        if !value.is_finite() {
            return Err(AslsError::NumericalInvariantViolation);
        }
        corrected.push(value);
    }
    Ok(corrected)
}

fn reserved<T>(length: usize) -> Result<Vec<T>, AslsError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| AslsError::AllocationFailure)?;
    Ok(values)
}

fn filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, AslsError> {
    let mut values = reserved(length)?;
    values.resize(length, value);
    Ok(values)
}

fn copied<T: Copy>(values: &[T]) -> Result<Vec<T>, AslsError> {
    let mut output = reserved(values.len())?;
    output.extend_from_slice(values);
    Ok(output)
}

/// Fatal failures from the fixed AsLS profile.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AslsError {
    /// Cooperative execution control failed.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// Coordinates and samples differ in length or contain fewer than three points.
    #[error("AsLS requires matching coordinate and sample arrays with at least three points")]
    InvalidLength,
    /// Coordinates or samples contain NaN or infinity.
    #[error("AsLS input contains a non-finite value")]
    NonFiniteInput,
    /// Coordinates are repeated or do not increase strictly.
    #[error("AsLS requires strictly increasing coordinates")]
    NonMonotonicCoordinates,
    /// The scaled banded system has an invalid pivot.
    #[error("AsLS banded system is not positive definite")]
    NonPositiveDefinite,
    /// Finite input produced a non-finite intermediate result.
    #[error("AsLS numerical invariant was violated")]
    NumericalInvariantViolation,
    /// Fifty solves did not meet the weighted relative L2 tolerance.
    #[error("AsLS did not converge within 50 solves")]
    DidNotConverge,
    /// A scratch or output allocation failed.
    #[error("AsLS allocation failed")]
    AllocationFailure,
}
