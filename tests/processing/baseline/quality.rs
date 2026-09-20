//! Analytic signals and independent thresholds for processing quality.
//! Original tests are synthetic, distributed under the source repository license.
//! Deterministic, black-box quality contracts for automatic spectrum correction.
//!
//! These tests intentionally assert signal-level outcomes rather than recovered
//! parameters. A different optimizer or baseline solver is therefore free to
//! replace the current implementation as long as the user-visible quality is
//! preserved.

use crate::quality_support::*;
use nmr::Complex64;
use nmr::processing::{ProcessingOperation, ProcessingPlan, RealBaseline, SpectrumOperation};
fn correct_baseline(spec: &mut Spectrum) {
    let input = checked(spec);
    let output = ProcessingPlan::new(vec![ProcessingOperation::Spectrum {
        axis: 0,
        operation: SpectrumOperation::Baseline(RealBaseline::Asls {
            lambda: 50000.0,
            asymmetry: 0.001,
            iterations: 20,
        }),
    }])
    .unwrap()
    .apply(&input)
    .unwrap();
    replace(spec, &output);
}

const BASELINE_POINTS: usize = 640;
#[derive(Clone, Copy, Debug)]
enum BaselineShape {
    Linear,
    Quadratic,
    SlowlyVarying,
}

fn baseline_value(shape: BaselineShape, x: f64) -> f64 {
    match shape {
        BaselineShape::Linear => 0.16 + 0.075 * x,
        BaselineShape::Quadratic => 0.13 + 0.035 * x + 0.085 * x * x,
        BaselineShape::SlowlyVarying => {
            0.17 + 0.045 * (1.35 * std::f64::consts::PI * (x + 0.17)).sin()
        }
    }
}

fn gaussian(i: usize, center: usize, width: f64, height: f64) -> f64 {
    let distance = (i as f64 - center as f64) / width;
    height * (-0.5 * distance * distance).exp()
}

fn supported_peak_signal(i: usize) -> f64 {
    gaussian(i, 175, 5.0, 1.0) + gaussian(i, 405, 22.0, 0.62)
}

#[derive(Clone, Copy, Debug)]
struct BaselineQuality {
    normalized_baseline_rmse: f64,
    narrow_height_ratio: f64,
    broad_height_ratio: f64,
    narrow_area_ratio: f64,
    broad_area_ratio: f64,
}

fn area(values: &[f64], center: usize, radius: usize) -> f64 {
    let start = center.saturating_sub(radius);
    let end = (center + radius + 1).min(values.len());
    values[start..end].iter().sum()
}

fn asls_quality(shape: BaselineShape, scale: f64) -> BaselineQuality {
    let mut known_baseline = Vec::with_capacity(BASELINE_POINTS);
    let mut known_signal = Vec::with_capacity(BASELINE_POINTS);
    let mut values = Vec::with_capacity(BASELINE_POINTS);
    for i in 0..BASELINE_POINTS {
        let x = 2.0 * i as f64 / (BASELINE_POINTS - 1) as f64 - 1.0;
        let base = baseline_value(shape, x) * scale;
        let signal = supported_peak_signal(i) * scale;
        let noise = deterministic_complex_noise(i, 0.0015 * scale);
        known_baseline.push(base);
        known_signal.push(signal);
        values.push(Complex64::new(base + signal + noise.re, noise.im));
    }
    let observed = values.clone();
    let mut corrected = spectrum(values);
    correct_baseline(&mut corrected);

    // Baseline correction is a real-channel operation. Treating the imaginary
    // channel as immutable is part of its public signal-preservation contract.
    for (before, after) in observed.iter().zip(&corrected.values) {
        assert_eq!(before.im, after.im);
    }

    let estimated_baseline: Vec<f64> = observed
        .iter()
        .zip(&corrected.values)
        .map(|(before, after)| before.re - after.re)
        .collect();
    let baseline_error_rms = (estimated_baseline
        .iter()
        .zip(&known_baseline)
        .map(|(actual, expected)| (actual - expected).powi(2))
        .sum::<f64>()
        / BASELINE_POINTS as f64)
        .sqrt();
    let baseline_rms = (known_baseline
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        / BASELINE_POINTS as f64)
        .sqrt();
    let corrected_real: Vec<f64> = corrected.values.iter().map(|value| value.re).collect();

    BaselineQuality {
        normalized_baseline_rmse: baseline_error_rms / baseline_rms,
        narrow_height_ratio: corrected_real[175] / known_signal[175],
        broad_height_ratio: corrected_real[405] / known_signal[405],
        narrow_area_ratio: area(&corrected_real, 175, 20) / area(&known_signal, 175, 20),
        broad_area_ratio: area(&corrected_real, 405, 88) / area(&known_signal, 405, 88),
    }
}

fn assert_baseline_quality(label: &str, quality: BaselineQuality) {
    println!("baseline {label}: {quality:?}");
    assert!(
        quality.normalized_baseline_rmse < 0.12,
        "{label}: normalized baseline RMSE is {}",
        quality.normalized_baseline_rmse
    );
    assert!(
        (0.88..=1.08).contains(&quality.narrow_height_ratio),
        "{label}: narrow-peak height retention is {}",
        quality.narrow_height_ratio
    );
    assert!(
        (0.78..=1.10).contains(&quality.broad_height_ratio),
        "{label}: broad-peak height retention is {}",
        quality.broad_height_ratio
    );
    assert!(
        (0.86..=1.12).contains(&quality.narrow_area_ratio),
        "{label}: narrow-peak area retention is {}",
        quality.narrow_area_ratio
    );
    assert!(
        (0.65..=1.15).contains(&quality.broad_area_ratio),
        "{label}: broad-peak area retention is {}",
        quality.broad_area_ratio
    );
}

#[test]
fn asls_recovers_supported_baselines_and_preserves_narrow_and_broad_peaks() {
    for shape in [
        BaselineShape::Linear,
        BaselineShape::Quadratic,
        BaselineShape::SlowlyVarying,
    ] {
        assert_baseline_quality(&format!("{shape:?}"), asls_quality(shape, 1.0));
    }
}

#[test]
fn asls_quality_is_stable_across_intensity_scales() {
    let low = asls_quality(BaselineShape::Quadratic, 1.0e-4);
    let nominal = asls_quality(BaselineShape::Quadratic, 1.0);
    let high = asls_quality(BaselineShape::Quadratic, 1.0e4);
    for (label, quality) in [("low", low), ("nominal", nominal), ("high", high)] {
        assert_baseline_quality(label, quality);
    }

    let normalized_metrics = |quality: BaselineQuality| {
        [
            quality.normalized_baseline_rmse,
            quality.narrow_height_ratio,
            quality.broad_height_ratio,
            quality.narrow_area_ratio,
            quality.broad_area_ratio,
        ]
    };
    let nominal = normalized_metrics(nominal);
    for (label, scaled) in [("low", low), ("high", high)] {
        for (index, (actual, expected)) in normalized_metrics(scaled)
            .into_iter()
            .zip(nominal)
            .enumerate()
        {
            assert!(
                (actual - expected).abs() < 0.01,
                "{label}: normalized metric {index} changed from {expected} to {actual}"
            );
        }
    }
}

#[test]
fn asls_non_target_peak_shapes_remain_numerically_safe() {
    // AsLS with a small upper-envelope weight assumes peaks are positive and
    // appreciably narrower than the baseline. Negative peaks can anchor the fit,
    // while a peak spanning a substantial fraction of the spectrum is
    // indistinguishable from baseline curvature. Fidelity for either case is an
    // explicit non-goal of the automatic preset until polarity-aware or
    // peak-masked estimation is introduced. The boundary contract here is only
    // numerical safety and preservation of the untouched imaginary channel.
    for signal in [
        (0..BASELINE_POINTS)
            .map(|i| -gaussian(i, 300, 11.0, 0.8))
            .collect::<Vec<_>>(),
        (0..BASELINE_POINTS)
            .map(|i| gaussian(i, 320, BASELINE_POINTS as f64 / 5.0, 0.8))
            .collect::<Vec<_>>(),
    ] {
        let values: Vec<Complex64> = signal
            .iter()
            .enumerate()
            .map(|(i, peak)| {
                let x = 2.0 * i as f64 / (BASELINE_POINTS - 1) as f64 - 1.0;
                Complex64::new(
                    baseline_value(BaselineShape::Quadratic, x) + peak,
                    0.01 * (0.31 * i as f64).sin(),
                )
            })
            .collect();
        let imaginary_before: Vec<f64> = values.iter().map(|value| value.im).collect();
        let input_abs_max = values
            .iter()
            .map(|value| value.re.abs())
            .fold(0.0_f64, f64::max);
        let mut corrected = spectrum(values);
        correct_baseline(&mut corrected);

        assert!(corrected.values.iter().all(|value| value.re.is_finite()));
        for (value, expected_imaginary) in corrected.values.iter().zip(&imaginary_before) {
            assert_eq!(value.im, *expected_imaginary);
        }
        let output_abs_max = corrected
            .values
            .iter()
            .map(|value| value.re.abs())
            .fold(0.0_f64, f64::max);
        assert!(
            output_abs_max / input_abs_max < 4.0,
            "unsupported peak shape was numerically amplified by {}x",
            output_abs_max / input_abs_max
        );
    }
}
