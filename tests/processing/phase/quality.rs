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
use nmr::processing::{PhaseMethod, ProcessingOptions};
fn correct_phase(spec: &mut Spectrum) {
    let input = checked(spec);
    let estimate = PhaseMethod::RobustConsensus
        .prepare(&input, 0, ProcessingOptions::new())
        .unwrap()
        .estimate()
        .unwrap();
    let output = estimate.apply(&input, ProcessingOptions::new()).unwrap();
    replace(spec, &output);
}
const PHASE_POINTS: usize = 512;
/// A correctly phased spectrum containing resolved and overlapping lines.
/// Each line has absorptive real and dispersive imaginary components, matching the quadrature structure of a frequency-domain NMR
/// resonance rather than using an unrealistically real-only test signal.
fn ideal_phase_spectrum(scale: f64) -> Vec<Complex64> {
    let peaks = [
        (0.24, 0.010, 0.72),
        (0.455, 0.012, 1.00),
        (0.477, 0.017, 0.64),
        (0.78, 0.014, 0.48),
    ];
    (0..PHASE_POINTS)
        .map(|i| {
            let frac = i as f64 / (PHASE_POINTS - 1) as f64;
            let mut value = Complex64::new(0.0, 0.0);
            for &(center, width, height) in &peaks {
                let d = (frac - center) / width;
                value += Complex64::new(1.0 / (1.0 + d * d), -d / (1.0 + d * d)) * height;
            }
            (value + deterministic_complex_noise(i, 0.0015)) * scale
        })
        .collect()
}

fn inject_phase(values: &[Complex64], phase0: f64, phase1: f64) -> Vec<Complex64> {
    let denom = (values.len() - 1).max(1) as f64;
    values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let frac = i as f64 / denom;
            value * Complex64::from_polar(1.0, phase0 + phase1 * frac)
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct PhaseQuality {
    negative_energy_fraction: f64,
    complex_nrms_error: f64,
    imaginary_energy_fraction_error: f64,
    peak_height_ratio: f64,
    signed_area_ratio: f64,
}

fn energy(values: &[Complex64]) -> f64 {
    values.iter().map(Complex64::norm_sqr).sum()
}

fn imaginary_energy_fraction(values: &[Complex64]) -> f64 {
    let total = energy(values);
    values.iter().map(|value| value.im * value.im).sum::<f64>() / total
}

fn assess_phase_quality(corrected: &[Complex64], reference: &[Complex64]) -> PhaseQuality {
    let real_energy: f64 = corrected.iter().map(|value| value.re * value.re).sum();
    let negative_energy: f64 = corrected
        .iter()
        .filter(|value| value.re < 0.0)
        .map(|value| value.re * value.re)
        .sum();
    let error_energy: f64 = corrected
        .iter()
        .zip(reference)
        .map(|(actual, expected)| (*actual - *expected).norm_sqr())
        .sum();
    let corrected_peak = corrected
        .iter()
        .map(|value| value.re)
        .fold(f64::NEG_INFINITY, f64::max);
    let reference_peak = reference
        .iter()
        .map(|value| value.re)
        .fold(f64::NEG_INFINITY, f64::max);
    let corrected_area: f64 = corrected.iter().map(|value| value.re).sum();
    let reference_area: f64 = reference.iter().map(|value| value.re).sum();
    PhaseQuality {
        negative_energy_fraction: negative_energy / real_energy,
        complex_nrms_error: (error_energy / energy(reference)).sqrt(),
        imaginary_energy_fraction_error: (imaginary_energy_fraction(corrected)
            - imaginary_energy_fraction(reference))
        .abs(),
        peak_height_ratio: corrected_peak / reference_peak,
        signed_area_ratio: corrected_area / reference_area,
    }
}

fn robust_phase_quality(phase0: f64, phase1: f64, scale: f64) -> PhaseQuality {
    let reference = ideal_phase_spectrum(scale);
    let mut observed = spectrum(inject_phase(&reference, phase0, phase1));
    correct_phase(&mut observed);
    assess_phase_quality(&observed.values, &reference)
}

fn assert_phase_quality(label: &str, quality: PhaseQuality) {
    println!("phase {label}: {quality:?}");
    assert!(
        quality.negative_energy_fraction < 0.05,
        "{label}: negative-energy fraction is {}",
        quality.negative_energy_fraction
    );
    assert!(
        quality.complex_nrms_error < 0.32,
        "{label}: complex normalized RMS error is {}",
        quality.complex_nrms_error
    );
    assert!(
        quality.imaginary_energy_fraction_error < 0.08,
        "{label}: imaginary-energy fraction differs by {}",
        quality.imaginary_energy_fraction_error
    );
    assert!(
        (0.80..=1.10).contains(&quality.peak_height_ratio),
        "{label}: peak-height retention is {}",
        quality.peak_height_ratio
    );
    assert!(
        (0.75..=1.20).contains(&quality.signed_area_ratio),
        "{label}: signed-area retention is {}",
        quality.signed_area_ratio
    );
}

#[test]
fn robust_consensus_corrects_phase0_phase1_overlap_and_noise() {
    let cases = [
        ("zero-order", 0.85, 0.0),
        ("positive-ramp", -0.65, 0.90),
        ("negative-ramp", 1.05, -1.15),
    ];
    for (label, phase0, phase1) in cases {
        assert_phase_quality(label, robust_phase_quality(phase0, phase1, 1.0));
    }
}

#[test]
fn robust_consensus_quality_is_stable_across_intensity_scales() {
    let low = robust_phase_quality(-0.72, 1.05, 1.0e-5);
    let nominal = robust_phase_quality(-0.72, 1.05, 1.0);
    let high = robust_phase_quality(-0.72, 1.05, 1.0e5);
    for (label, quality) in [("low", low), ("nominal", nominal), ("high", high)] {
        assert_phase_quality(label, quality);
    }

    let normalized_metrics = |quality: PhaseQuality| {
        [
            quality.negative_energy_fraction,
            quality.complex_nrms_error,
            quality.imaginary_energy_fraction_error,
            quality.peak_height_ratio,
            quality.signed_area_ratio,
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
