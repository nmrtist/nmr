//! Split-observation noise evidence in the actual Cartesian IST input units.
use super::*;

/// Automatic noise analysis, without caller-supplied sigma or noise regions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoNusSettings {
    /// Hard iteration ceiling. Exhaustion remains an error.
    pub max_iterations: usize,
}
impl Default for AutoNusSettings {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
        }
    }
}
impl AutoNusSettings {
    /// Prepare acquired-row F2 processing and automatic NUS reconstruction.
    pub fn prepare(
        self,
        input: &crate::Dataset,
        direct: ProcessingPlan,
        options: ProcessingOptions,
    ) -> Result<PreparedNus<'_>, ProcessingError> {
        self.prepare_with_context(input, direct, options, &mut ExecutionContext::default())
    }
    /// Prepare with shared cancellation and capacity limits.
    pub fn prepare_with_context<'a>(
        self,
        input: &'a crate::Dataset,
        direct: ProcessingPlan,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<PreparedNus<'a>, ProcessingError> {
        NusSettings {
            max_iterations: self.max_iterations,
            noise_standard_deviation: None,
        }
        .prepare_internal(input, direct, options, control, true)
    }
}

/// Recorded scientific source of the sigma passed to IST.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NusNoiseSource {
    /// Caller assertion, including an explicitly asserted zero.
    Explicit,
    /// Three disjoint observation folds: selection, validation, estimation.
    SplitObservationsV1,
    /// Two disjoint row folds for short acquisitions: selection and held-out
    /// estimation/validation. Quality checks use the estimation fold, so this
    /// is conditional noise evidence, not a third independent validation.
    SplitHoldoutV1,
    /// JEOL digital-filter interior-band policy with three disjoint row folds.
    JeolInteriorV1,
    /// JEOL interior-band policy with selection and held-out row halves.
    JeolInteriorHoldoutV1,
}

/// Noise evidence attached to the enclosing NUS derivation. Input identity,
/// observation order, component interpretation and direct plan are referenced
/// through that derivation, not duplicated here.
#[derive(Clone, Debug, PartialEq)]
pub struct NusNoiseReport {
    /// Actual policy; automatic estimates are never independent measurements.
    pub source: NusNoiseSource,
    /// Standard deviation per real component after the direct plan.
    pub sigma: f64,
    /// Selected half-open F2-bin intervals in the processed direct grid.
    pub frequency_ranges: Vec<(usize, usize)>,
    /// Number of scalar values used for the final estimate (not independent count).
    pub scalar_samples: usize,
    /// Conservative number of independent observation clusters. Frequency bins
    /// and Cartesian components within a row are not counted as independent.
    pub effective_observations: usize,
    /// Maximum relative block RMS deviation in held-out validation.
    pub block_dispersion: f64,
    /// Relative departure of radial fourth moment from isotropic Gaussian noise.
    pub radial_moment_error: f64,
    /// Frobenius departure of component second moment from a scalar identity.
    pub isotropy_error: f64,
    /// Normalized correlation between successive held-out observation rows.
    pub observation_correlation: f64,
}
impl NusNoiseSource {
    pub(crate) fn holdout(self) -> bool {
        matches!(self, Self::SplitHoldoutV1 | Self::JeolInteriorHoldoutV1)
    }
    pub(crate) fn interior(self) -> bool {
        matches!(self, Self::JeolInteriorV1 | Self::JeolInteriorHoldoutV1)
    }
}
impl NusNoiseReport {
    /// Amplitude convention, referenced to the enclosing resolved direct plan.
    pub fn amplitude_convention(&self) -> &'static str {
        "standard deviation per real component; actual unnormalised F2 output units"
    }
    /// Version fixes selection rules, diagnostics and thresholds.
    pub fn method_version(&self) -> &'static str {
        match self.source {
            NusNoiseSource::Explicit => "caller-cartesian-sigma.v1",
            NusNoiseSource::SplitObservationsV1 => "split-observation-cartesian-rms.v1",
            NusNoiseSource::SplitHoldoutV1 => "split-holdout-component-rms.v1",
            NusNoiseSource::JeolInteriorV1 => "jeol-interior-split-rms.v1",
            NusNoiseSource::JeolInteriorHoldoutV1 => "jeol-interior-holdout-rms.v1",
        }
    }
    pub(super) fn explicit(sigma: f64) -> Self {
        Self {
            source: NusNoiseSource::Explicit,
            sigma,
            frequency_ranges: vec![],
            scalar_samples: 0,
            effective_observations: 0,
            block_dispersion: 0.0,
            radial_moment_error: 0.0,
            isotropy_error: 0.0,
            observation_correlation: 0.0,
        }
    }
    pub(crate) fn validate(&self, columns: usize, settings: NusSettings) -> bool {
        let finite = [
            self.sigma,
            self.block_dispersion,
            self.radial_moment_error,
            self.isotropy_error,
            self.observation_correlation,
        ]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0);
        finite
            && match self.source {
                NusNoiseSource::Explicit => {
                    *self == Self::explicit(self.sigma)
                        && settings.noise_standard_deviation == Some(self.sigma)
                }
                NusNoiseSource::SplitObservationsV1
                | NusNoiseSource::SplitHoldoutV1
                | NusNoiseSource::JeolInteriorV1
                | NusNoiseSource::JeolInteriorHoldoutV1 => {
                    settings.noise_standard_deviation.is_none()
                        && self.sigma > 0.0
                        && self.effective_observations >= 16
                        && (!self.source.holdout()
                            || (columns >= 128 && self.scalar_samples >= 512))
                        && self
                            .effective_observations
                            .checked_mul(2)
                            .is_some_and(|n| self.scalar_samples >= n)
                        && self.frequency_ranges.len() == 4
                        && self
                            .frequency_ranges
                            .iter()
                            .all(|&(a, b)| a < b && b <= columns)
                        && self.frequency_ranges.windows(2).all(|r| r[0].1 <= r[1].0)
                        && self.block_dispersion <= 0.3
                        && self.radial_moment_error <= 0.3
                        && self.isotropy_error <= 0.3
                        && self.observation_correlation <= 0.3
                }
            }
    }
}

/// Automatic noise evidence was insufficient. No reconstruction is returned.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum NusNoiseError {
    /// The direct operation has not been validated for this noise model.
    #[error("automatic NUS noise does not support direct operation {step}")]
    UnsupportedOperation {
        /// Zero-based direct-plan operation.
        step: usize,
    },
    /// Need 48 rows / 32 bins, or 32 rows / 128 bins for held-out estimation.
    #[error("automatic NUS noise requires 48 rows / 32 bins or 32 rows / 128 bins")]
    InsufficientSamples,
    /// Zero processed samples cannot establish thermal-noise evidence.
    #[error("automatic NUS received an all-zero processed input")]
    DegenerateInput,
    /// Held-out selected regions are not consistent with the model.
    #[error("automatic NUS noise validation failed: {0}")]
    UnreliableEvidence(&'static str),
    /// Held-out quality metrics exceeded the fixed V1 thresholds.
    #[error(
        "automatic NUS noise quality: block={block_dispersion}, radial={radial_moment_error}, isotropy={isotropy_error}, correlation={observation_correlation}, mean={mean_ratio}"
    )]
    Quality {
        /// Maximum relative block RMS deviation.
        block_dispersion: f64,
        /// Relative Gaussian radial fourth-moment error.
        radial_moment_error: f64,
        /// Relative isotropic covariance error.
        isotropy_error: f64,
        /// Adjacent observation cross-covariance norm.
        observation_correlation: f64,
        /// Squared mean relative to total component variance.
        mean_ratio: f64,
    },
}

pub(super) fn validate_plan(plan: &ProcessingPlan) -> Result<(), ProcessingError> {
    validate_operations(&plan.operations)
}
pub(super) fn validate_operations(
    operations: &[ProcessingOperation],
) -> Result<(), ProcessingError> {
    for (step, operation) in operations.iter().enumerate() {
        if !matches!(
            operation,
            ProcessingOperation::Window { .. }
                | ProcessingOperation::ZeroFill { .. }
                | ProcessingOperation::StandardZeroFill { .. }
                | ProcessingOperation::FourierTransform { .. }
                | ProcessingOperation::DigitalFilterCorrection { .. }
                | ProcessingOperation::PhaseCorrection { .. }
                | ProcessingOperation::ComponentTransform { .. }
                | ProcessingOperation::ResolveFrequencyFrame { .. }
                | ProcessingOperation::ReverseAxis { .. }
        ) {
            return Err(ProcessingError::NoiseEstimation(
                NusNoiseError::UnsupportedOperation { step },
            ));
        }
    }
    Ok(())
}

// Bounded scans (including covariance and lag diagnostics) plus a 32-element sort.
pub(super) fn work(scalars: usize) -> Result<u128, ProcessingError> {
    (scalars as u128)
        .checked_mul(96)
        .and_then(|n| n.checked_add(2048))
        .ok_or(ProcessingError::SizeOverflow)
}

pub(super) fn estimate(
    input: &IstInput,
    control: &mut ExecutionContext<'_>,
    interior: bool,
) -> Result<NusNoiseReport, ProcessingError> {
    let error =
        |reason| ProcessingError::NoiseEstimation(NusNoiseError::UnreliableEvidence(reason));
    control.begin(ExecutionStage::NoiseEstimation, None, None)?;
    control.charge(work(input.components().len())?)?;
    let (m, f, k) = (
        input.measured_indices().len(),
        input.f2_points(),
        input.cartesian_fields() * 2,
    );
    if !sufficient_samples(m, f) {
        return Err(ProcessingError::NoiseEstimation(
            NusNoiseError::InsufficientSamples,
        ));
    }
    // Keep the three-fold policy unchanged for larger acquisitions. With only
    // 32..47 rows, reserve half for selection and half for held-out RMS and
    // diagnostics, retaining >=16 independent row clusters in each half.
    // Require wider spectral blocks; do not count bins as independent rows.
    let folds = if m < 48 { 2 } else { 3 };
    let y = input.components();
    let mut scale = 0.0_f64;
    for chunk in y.chunks(4096) {
        control.check_cancelled()?;
        for &v in chunk {
            scale = scale.max(v.abs());
        }
    }
    if scale == 0.0 {
        return Err(ProcessingError::NoiseEstimation(
            NusNoiseError::DegenerateInput,
        ));
    }
    // Global scaling prevents overflow; cancels from every selection/quality ratio.
    let mut blocks = [(0usize, 0usize, 0.0_f64); 32];
    for (b, block) in blocks.iter_mut().enumerate() {
        let (start, end) = (b * f / 32, (b + 1) * f / 32);
        let mut energy = 0.0;
        for row in (0..m).step_by(folds) {
            control.check_cancelled()?;
            for &v in &y[(row * f + start) * k..(row * f + end) * k] {
                energy += (v / scale).powi(2);
            }
        }
        *block = (start, end, energy / (end - start) as f64);
    }
    // One candidate per spectral quarter prevents a coloured-noise trough in
    // one band from supplying all the evidence for a global scalar sigma.
    let ranges = blocks
        .chunks_exact(8)
        .enumerate()
        .map(|(index, quarter)| {
            // JEOL's enabled receiver filter suppresses spectral edges. Use a
            // fixed 1/8-grid guard at either edge, never tuned on validation.
            // Sigma describes the interior passband; it is conservative in the
            // suppressed edge bins. This is an assumption, not filter inversion.
            let quarter = if interior && index == 0 {
                &quarter[4..]
            } else if interior && index == 3 {
                &quarter[..4]
            } else {
                quarter
            };
            let b = quarter
                .iter()
                .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)))
                .unwrap();
            (b.0, b.1)
        })
        .collect::<Vec<_>>();
    let bins: usize = ranges.iter().map(|(a, b)| b - a).sum();
    // Fold 1 only validates; no retry or selection using its observed amplitudes.
    let mut cov = [[0.0; 4]; 4];
    let mut mean = [0.0; 4];
    let mut block_energy = [0.0; 4];
    let mut fourth = 0.0;
    let rows = (1..m).step_by(folds).count();
    for row in (1..m).step_by(folds) {
        control.check_cancelled()?;
        for (block, &(start, end)) in ranges.iter().enumerate() {
            for p in start..end {
                let mut v = [0.0; 4];
                for c in 0..k {
                    v[c] = y[(row * f + p) * k + c] / scale;
                }
                let r2: f64 = v[..k].iter().map(|v| v * v).sum();
                block_energy[block] += r2;
                fourth += r2 * r2;
                for a in 0..k {
                    mean[a] += v[a];
                    for b in 0..k {
                        cov[a][b] += v[a] * v[b];
                    }
                }
            }
        }
    }
    let count = (rows * bins) as f64;
    let energy: f64 = block_energy.iter().sum();
    let variance = energy / (count * k as f64);
    if variance <= 0.0 {
        return Err(error("selected regions have zero variance"));
    }
    let mut isotropy = 0.0;
    for (a, covariance_row) in cov.iter().enumerate().take(k) {
        for (b, &value) in covariance_row.iter().enumerate().take(k) {
            isotropy += (value / (count * variance) - if a == b { 1.0 } else { 0.0 }).powi(2);
        }
    }
    isotropy = (isotropy / k as f64).sqrt();
    let radial = (fourth / count / (variance * variance * (k * (k + 2)) as f64) - 1.0).abs();
    // Check adjacency in original acquisition order, including fold boundaries.
    let mut cross = [[0.0; 4]; 4];
    let mut adjacent_energy = 0.0;
    for row in 1..m {
        control.check_cancelled()?;
        for &(start, end) in &ranges {
            for p in start..end {
                for a in 0..k {
                    let v = y[(row * f + p) * k + a] / scale;
                    adjacent_energy += v * v;
                    for b in 0..k {
                        cross[a][b] += v * y[((row - 1) * f + p) * k + b] / scale;
                    }
                }
            }
        }
    }
    let correlation = cross.iter().flatten().map(|v| v * v).sum::<f64>().sqrt() * (k as f64).sqrt()
        / adjacent_energy;
    let mean_ratio =
        mean[..k].iter().map(|v| (v / count).powi(2)).sum::<f64>() / (variance * k as f64);
    let dispersion = ranges
        .iter()
        .zip(block_energy)
        .map(|(&(a, b), e)| ((e / ((b - a) * rows * k) as f64 / variance).sqrt() - 1.0).abs())
        .fold(0.0, f64::max);
    if radial > 0.3 || isotropy > 0.3 || dispersion > 0.3 || correlation > 0.3 || mean_ratio > 0.2 {
        return Err(ProcessingError::NoiseEstimation(NusNoiseError::Quality {
            block_dispersion: dispersion,
            radial_moment_error: radial,
            isotropy_error: isotropy,
            observation_correlation: correlation,
            mean_ratio,
        }));
    }
    // Three-fold estimates are independent of selection and validation.
    // Two-fold estimates remain independent of selection, but share validation.
    let mut estimate_energy = 0.0;
    for row in (folds - 1..m).step_by(folds) {
        control.check_cancelled()?;
        for &(start, end) in &ranges {
            for &v in &y[(row * f + start) * k..(row * f + end) * k] {
                estimate_energy += (v / scale).powi(2);
            }
        }
    }
    let effective_observations = (folds - 1..m).step_by(folds).count();
    let scalar_samples = effective_observations * bins * k;
    let sigma = (estimate_energy / scalar_samples as f64).sqrt() * scale;
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(error("nonpositive or nonfinite estimated sigma"));
    }
    let ratio = sigma / scale / variance.sqrt();
    if !(0.7..=1.3).contains(&ratio) {
        return Err(error("estimation and validation folds disagree"));
    }
    control.complete_work()?;
    Ok(NusNoiseReport {
        source: if interior && folds == 2 {
            NusNoiseSource::JeolInteriorHoldoutV1
        } else if interior {
            NusNoiseSource::JeolInteriorV1
        } else if folds == 3 {
            NusNoiseSource::SplitObservationsV1
        } else {
            NusNoiseSource::SplitHoldoutV1
        },
        sigma,
        frequency_ranges: ranges,
        scalar_samples,
        effective_observations,
        block_dispersion: dispersion,
        radial_moment_error: radial,
        isotropy_error: isotropy,
        observation_correlation: correlation,
    })
}

pub(super) fn sufficient_samples(rows: usize, columns: usize) -> bool {
    (rows >= 48 && columns >= 32) || (rows >= 32 && columns >= 128)
}

#[cfg(test)]
mod tests;
