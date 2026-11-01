//! Read-only, experimental diagnostics of final two-dimensional frequency spectra.
//!
//! Gaussian calibration applies to phase-sensitive real amplitudes, not magnitudes.
use crate::{
    ExecutionContext,
    execution::{ExecutionError, ExecutionStage},
    processed::ProcessedDataset,
};

/// Stable identity of the sampling, calibration and rejection policy.
pub const METHOD: &str = "spatial-amplitude-mad.v3-stratified-median-tail-cap1024";

/// Caller-declared statistical representation; never inferred from sample signs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Representation {
    /// Signed phase-sensitive real amplitudes with approximately Gaussian background.
    PhaseSensitive,
    /// Complex or hypercomplex modulus; Gaussian amplitude calibration is unsupported.
    Magnitude,
    /// Representation is not established.
    Unknown,
}
/// Evidence classification, not a confidence probability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Status {
    /// Distributed blocks are consistent with the assumed model.
    Estimated,
    /// Every valid point is exactly equal, with complete coverage.
    Constant,
    /// Too few finite samples or too small a spatial extent.
    InsufficientSamples,
    /// Background coverage or model checks failed; estimates are withheld.
    Unidentifiable,
    /// Statistical representation is unsupported.
    Unsupported,
}
/// Reproducible measurement and heuristic diagnostics in input amplitude units.
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundReport {
    /// Algorithm identity including fixed sampling and rejection policy.
    pub method: &'static str,
    /// Declared representation.
    pub representation: Representation,
    /// Measurement availability.
    pub status: Status,
    /// Median background location; absent when not identifiable.
    pub location: Option<f64>,
    /// Median block MAD / normal median absolute deviation; no difference calibration.
    pub sigma: Option<f64>,
    /// Interquartile range of accepted block locations in amplitude units.
    pub location_iqr: Option<f64>,
    /// Median across accepted blocks of their 99th absolute residual
    /// percentile. A sampled background tail amplitude, not a noise sigma,
    /// confidence bound, or recommended display threshold.
    pub background_tail: Option<f64>,
    /// 5th and 95th percentiles of accepted block locations. This describes
    /// accepted background only, not the full baseline excursion.
    pub location_span: Option<[f64; 2]>,
    /// Finite, unmasked points scanned (not independent observations).
    pub valid_points: usize,
    /// Points used in accepted blocks (not an effective independent sample size).
    pub background_samples: usize,
    /// Accepted blocks divided by all 64 spatial blocks.
    pub coverage: f64,
    /// Reasons requiring caller attention; no false-discovery guarantee.
    pub reasons: Vec<&'static str>,
}
/// Invalid request or shared execution failure. Scientific failure lives in the report.
#[derive(Debug, thiserror::Error)]
pub enum BackgroundError {
    /// Rank, extent, storage or mask is invalid.
    #[error("expected a nonempty two-dimensional scalar frequency plane and matching mask")]
    InvalidShape,
    /// Shared cancellation or resource budget failed.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
}
/// Diagnose checked scalar frequency data without copying the plane.
/// `representation` is an explicit caller assertion, including for imported data.
/// Cache with the dataset's canonical digests, representation and report method.
pub fn diagnose_dataset(
    data: &ProcessedDataset,
    representation: Representation,
    control: &mut ExecutionContext<'_>,
) -> Result<BackgroundReport, BackgroundError> {
    if data
        .descriptor()
        .axes()
        .iter()
        .any(|a| a.domain() != crate::axis::AxisDomain::Frequency)
        || data.data().component_counts().iter().any(|&c| c != 1)
    {
        return Err(BackgroundError::InvalidShape);
    }
    // A caller assertion must not override known nonlinear modulus evidence.
    let representation =
        if representation == Representation::PhaseSensitive && data.has_magnitude_history() {
            Representation::Magnitude
        } else {
            representation
        };
    diagnose(
        data.data().samples(),
        data.data().shape(),
        None,
        representation,
        control,
    )
}
/// Borrowed row-major scalar plane, second axis contiguous; no implicit rescaling.
/// `mask`, if supplied, has one boolean per stored value (`true` includes it).
/// Nonfinite values are excluded. Strided/component data should first select a
/// scalar processed product. Scratch is bounded by 1024 samples plus 64 summaries.
pub fn diagnose(
    values: &[f64],
    shape: &[usize],
    mask: Option<&[bool]>,
    representation: Representation,
    control: &mut ExecutionContext<'_>,
) -> Result<BackgroundReport, BackgroundError> {
    diagnose_values(values, shape, mask, representation, control)
}

/// Single-precision counterpart of [`diagnose`]. Samples are promoted only
/// while scanning or filling bounded scratch; no full-plane conversion occurs.
pub fn diagnose_f32(
    values: &[f32],
    shape: &[usize],
    mask: Option<&[bool]>,
    representation: Representation,
    control: &mut ExecutionContext<'_>,
) -> Result<BackgroundReport, BackgroundError> {
    diagnose_values(values, shape, mask, representation, control)
}

fn diagnose_values<T: Copy + Into<f64>>(
    values: &[T],
    shape: &[usize],
    mask: Option<&[bool]>,
    representation: Representation,
    control: &mut ExecutionContext<'_>,
) -> Result<BackgroundReport, BackgroundError> {
    if shape.len() != 2
        || shape.contains(&0)
        || shape[0].checked_mul(shape[1]) != Some(values.len())
        || mask.is_some_and(|m| m.len() != values.len())
    {
        return Err(BackgroundError::InvalidShape);
    }
    control.begin(ExecutionStage::NoiseEstimation, None, None)?;
    control.charge(values.len() as u128 * 32 + 100_000)?;
    control.observe_payload(32_768);
    let mut report = BackgroundReport {
        method: METHOD,
        representation,
        status: Status::Unsupported,
        location: None,
        sigma: None,
        location_iqr: None,
        background_tail: None,
        location_span: None,
        valid_points: 0,
        background_samples: 0,
        coverage: 0.0,
        reasons: vec![],
    };
    if representation != Representation::PhaseSensitive {
        report
            .reasons
            .push("Gaussian amplitude model does not support this representation");
        control.complete_work()?;
        return Ok(report);
    }
    let valid = |i: usize| values[i].into().is_finite() && mask.is_none_or(|m| m[i]);
    let mut first = None;
    let mut constant = true;
    for (i, &v) in values.iter().enumerate() {
        let v: f64 = v.into();
        if i % 4096 == 0 {
            control.check_cancelled()?;
        }
        if valid(i) {
            report.valid_points += 1;
            if let Some(f) = first {
                constant &= v == f;
            } else {
                first = Some(v);
            }
        }
    }
    if shape.iter().any(|&n| n < 32) || report.valid_points < 1024 {
        report.status = Status::InsufficientSamples;
        report
            .reasons
            .push("requires both extents >= 32 and at least 1024 finite unmasked points");
    } else if constant && report.valid_points == values.len() {
        report.status = Status::Constant;
        report.location = first;
        report.sigma = Some(0.0);
        report.location_iqr = Some(0.0);
        report.background_tail = Some(0.0);
        report.location_span = first.map(|v| [v, v]);
        report.coverage = 1.0;
        report.background_samples = values.len();
    } else {
        let mut blocks = Vec::with_capacity(64);
        let mut scratch = Vec::with_capacity(1024);
        for br in 0..8 {
            for bc in 0..8 {
                control.check_cancelled()?;
                scratch.clear();
                let (r0, r1, c0, c1) = (
                    br * shape[0] / 8,
                    (br + 1) * shape[0] / 8,
                    bc * shape[1] / 8,
                    (bc + 1) * shape[1] / 8,
                );
                // One amplitude-independent point per rectangular stratum.
                // Varying offsets breaks the regular-grid alias with periodic
                // stripes while retaining bounded, distributed sampling.
                let nr = (r1 - r0).min(32);
                let nc = (c1 - c0).min(32);
                for sr in 0..nr {
                    for sc in 0..nc {
                        let ra = r0 + sr * (r1 - r0) / nr;
                        let rb = r0 + (sr + 1) * (r1 - r0) / nr;
                        let ca = c0 + sc * (c1 - c0) / nc;
                        let cb = c0 + (sc + 1) * (c1 - c0) / nc;
                        let id = ((br * 8 + bc) * 1024 + sr * 32 + sc) as u64;
                        let r = ra + sample_hash(id * 2) as usize % (rb - ra);
                        let c = ca + sample_hash(id * 2 + 1) as usize % (cb - ca);
                        let i = r * shape[1] + c;
                        if valid(i) {
                            scratch.push(values[i].into());
                        }
                    }
                }
                if scratch.len() < 16 {
                    continue;
                }
                let n = scratch.len();
                let location = quantile(&mut scratch, 0.5);
                // A large atom (often zero padding or quantization) is not Gaussian
                // noise evidence. Test before absolute residuals merge signs.
                let mut run = 1;
                let mut largest_run = 1;
                for pair in scratch.windows(2) {
                    run = if pair[0] == pair[1] { run + 1 } else { 1 };
                    largest_run = largest_run.max(run);
                }
                if largest_run > 2 && largest_run * 10 > n {
                    continue;
                }

                for x in &mut scratch {
                    *x = (*x - location).abs();
                }
                let sigma = quantile(&mut scratch, 0.5) / 0.6744897501960817;
                let tail = quantile(&mut scratch, 0.9);
                if sigma > 0.0 && sigma.is_finite() && tail / sigma < 2.5 {
                    let tail99 = quantile(&mut scratch, 0.99);
                    blocks.push((br, bc, location, sigma, n, tail99));
                }
            }
        }
        let mut scales: Vec<_> = blocks.iter().map(|b| b.3).collect();
        let typical = quantile(&mut scales, 0.5);
        let mut centers: Vec<_> = blocks.iter().map(|b| b.2).collect();
        let center = quantile(&mut centers, 0.5);
        blocks.retain(|b| {
            b.3 / typical > 0.5 && b.3 / typical < 2.0 && (b.2 - center).abs() / typical < 2.0
        });
        report.coverage = blocks.len() as f64 / 64.0;
        report.background_samples = blocks.iter().map(|b| b.4).sum();
        let quadrants = (0..4).all(|q| {
            blocks
                .iter()
                .filter(|b| (b.0 / 4) * 2 + b.1 / 4 == q)
                .count()
                >= 6
        });
        report.status = Status::Unidentifiable;
        if blocks.len() >= 40 && quadrants {
            let mut s: Vec<_> = blocks.iter().map(|b| b.3).collect();
            let sigma = quantile(&mut s, 0.5);
            let spread = (quantile(&mut s, 0.75) - quantile(&mut s, 0.25)) / sigma;
            let mut l: Vec<_> = blocks.iter().map(|b| b.2).collect();
            let location = quantile(&mut l, 0.5);
            let iqr = quantile(&mut l, 0.75) - quantile(&mut l, 0.25);
            report.location_iqr = Some(iqr);
            if spread < 0.35 && iqr / sigma < 0.75 && location.is_finite() {
                report.status = Status::Estimated;
                report.location = Some(location);
                report.sigma = Some(sigma);
                let mut tails: Vec<_> = blocks.iter().map(|b| b.5).collect();
                report.background_tail = Some(quantile(&mut tails, 0.5));
                report.location_span = Some([quantile(&mut l, 0.05), quantile(&mut l, 0.95)]);
            } else {
                report.reasons.push("block locations or scales vary: baseline, correlation, or structured contamination");
            }
        } else {
            report
                .reasons
                .push("insufficient distributed nondegenerate background blocks");
        }
        if report.valid_points < values.len() {
            report.reasons.push("masked or nonfinite points excluded");
        }
        if report.status == Status::Estimated {
            report.reasons.push("conditional Gaussian marginal estimate; broad signals and long correlations may be indistinguishable from background; independent sample count unknown");
        }
    }
    control.complete_work()?;
    Ok(report)
}
// SplitMix64 finalizer; sampling depends on coordinates, never intensities.
fn sample_hash(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

fn quantile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_unstable_by(f64::total_cmp);
    let x = (v.len() - 1) as f64 * p;
    let i = x as usize;
    v[i] * (1.0 - (x - i as f64)) + v[(i + 1).min(v.len() - 1)] * (x - i as f64)
}
