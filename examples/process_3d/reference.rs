//! Independent indirect DFT over NMRPipe's direct-axis transform for ubiquitin samples 2, 14 and 16.
use nmr::Complex64;
use nmr::processed::ProcessedDataset;
use std::{f64::consts::TAU, path::Path};

const DELAY: f64 = 67.9858856201172;

pub(super) fn compare(
    spectrum: &ProcessedDataset,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let shape = spectrum.descriptor().logical_shape();
    let (n2, echo_anti_echo) = match shape.as_slice() {
        [64, 20, 1024] => (20, false),
        [64, 32, 1024] => (32, true),
        _ => return Err("reference is for ubiquitin samples 2, 14 and 16".into()),
    };
    let (n1, n3) = (64, 1024);
    let bytes = std::fs::read(path)?;
    if bytes.len() != 2048 + 4 * n1 * n2 * n3 * 8 {
        return Err("unexpected NMRPipe row stream size".into());
    }
    let value = |row, sample| {
        let start = 2048 + (row * 2 * n3 + sample) * 4;
        f32::from_le_bytes(
            bytes[start..start + 4]
                .try_into()
                .expect("four-byte sample"),
        ) as f64
    };
    for f3 in [500, 512, 530, 550, 580, 760, 850, 915] {
        let k3 = f3 as f64 - n3 as f64 / 2.0;
        let ramp = Complex64::from_polar(1.0, TAU * k3 * DELAY / n3 as f64);
        // NMRPipe FT has the positive exponent; reverse its shifted bins.
        let pipe_bin = (n3 - f3) % n3;
        let direct = |row| Complex64::new(value(row, pipe_bin), value(row, pipe_bin + n3)) * ramp;
        let rows: Vec<_> = (0..4 * n1 * n2)
            .map(|row| {
                if echo_anti_echo {
                    let echo = direct(row & !1);
                    let anti = direct(row | 1);
                    // For F2, C = i(E + A) and S = A - E. Rotation on
                    // the direct axis commutes with this complex-linear map.
                    if row % 2 == 0 {
                        -(echo + anti).im
                    } else {
                        (anti - echo).re
                    }
                } else {
                    direct(row).re
                }
            })
            .collect();
        let w1 = weights(n1, true);
        let w2 = weights(n2, !echo_anti_echo);
        let mut worst_error: f64 = 0.0;
        let mut maximum: f64 = 0.0;
        let mut reference_peak = (0.0_f64, [0, 0]);
        let mut actual_peak = (0.0_f64, [0, 0]);
        for (f1, first) in w1.iter().enumerate() {
            // Transform every stored F2 row along F1 before summing along F2.
            let intermediate: Vec<f64> = (0..2 * n2)
                .map(|row2| {
                    first
                        .iter()
                        .enumerate()
                        .map(|(row1, weight)| rows[row1 * 2 * n2 + row2] * weight)
                        .sum()
                })
                .collect();
            for (f2, second) in w2.iter().enumerate() {
                let expected: f64 = intermediate
                    .iter()
                    .zip(second)
                    .map(|(sample, weight)| sample * weight)
                    .sum();
                let actual = spectrum.data().get(&[f1, f2, f3], &[0, 0, 0])?;
                if expected.abs() > reference_peak.0.abs() {
                    reference_peak = (expected, [f1, f2]);
                }
                if actual.abs() > actual_peak.0.abs() {
                    actual_peak = (actual, [f1, f2]);
                }
                worst_error = worst_error.max((expected - actual).abs());
                maximum = maximum.max(expected.abs());
            }
        }
        let normalized_error = worst_error / maximum;
        println!(
            "  F3 bin {f3}: max error / reference max = {normalized_error:.3e}; strongest F1/F2 bin {:?}, sign {:+}",
            reference_peak.1,
            reference_peak.0.signum()
        );
        if !normalized_error.is_finite()
            || normalized_error > 1e-5
            || reference_peak.1 != actual_peak.1
            || reference_peak.0.signum() != actual_peak.0.signum()
        {
            return Err(format!("independent comparison failed for F3 bin {f3}").into());
        }
    }
    Ok(())
}

fn weights(points: usize, tppi: bool) -> Vec<Vec<f64>> {
    (0..points)
        .map(|bin| {
            let k = bin as f64 - points as f64 / 2.0;
            (0..2 * points)
                .map(|row| {
                    let time = row / 2;
                    // States-TPPI parity on each logical time increment. Taking
                    // the real part of (C + iS) exp(-i theta) gives C cos + S sin.
                    let phase = TAU * k * time as f64 / points as f64;
                    let weight = if row % 2 == 0 {
                        phase.cos()
                    } else {
                        phase.sin()
                    };
                    if !tppi || time % 2 == 0 {
                        weight
                    } else {
                        -weight
                    }
                })
                .collect()
        })
        .collect()
}
