use super::*;
fn input(fields: usize, mode: usize, seed: u64) -> IstInput {
    input_shape(fields, mode, seed, 96, 128)
}
fn input_shape(fields: usize, mode: usize, seed: u64, m: usize, f: usize) -> IstInput {
    let mut rng = seed;
    let mut uniform = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    let mut y = vec![0.0; m * f * fields * 2];
    for row in 0..m {
        for p in 0..f {
            for field in 0..fields {
                let radius = (-2.0 * uniform().ln()).sqrt();
                let angle = std::f64::consts::TAU * uniform();
                for (c, noise) in [radius * angle.cos(), radius * angle.sin()]
                    .into_iter()
                    .enumerate()
                {
                    let index = ((row * f + p) * fields + field) * 2 + c;
                    y[index] = match mode {
                        1 => {
                            noise
                                + if row > 0 {
                                    0.95 * y[index - f * fields * 2]
                                } else {
                                    0.0
                                }
                        }
                        2 => {
                            noise
                                * (1.0
                                    + 8.0
                                        * (std::f64::consts::PI * p as f64 / f as f64)
                                            .sin()
                                            .powi(2))
                        }
                        3 => noise * if c == 0 { 10.0 } else { 1.0 },
                        4 => noise + 100.0 * (row as f64 * 0.2).cos(),
                        // Sparse narrow peaks and a solvent peak with a 10^6 dynamic range.
                        5 => {
                            noise
                                + if matches!(p, 13 | 49 | 73 | 111) {
                                    1e6 * (row as f64 * 0.2).cos()
                                } else {
                                    0.0
                                }
                        }
                        6 => {
                            noise
                                + (0..16)
                                    .map(|peak| {
                                        let u = (p as f64 - (peak as f64 * 8.0 + 3.0)) / 4.0;
                                        100.0 / (1.0 + u * u)
                                            * (row as f64 * (0.1 + peak as f64 * 0.03)).cos()
                                    })
                                    .sum::<f64>()
                        }
                        7 => {
                            noise
                                + 100.0 / (1.0 + ((p as f64 - 64.0) / 20.0).powi(2))
                                    * (row as f64 * 0.2).cos()
                        }
                        _ => noise,
                    };
                }
            }
        }
    }
    IstInput::with_grid(192, f, fields, (0..m).map(|i| i * 73 % 192).collect(), y).unwrap()
}
#[test]
fn guards_reject_correlated_coloured_anisotropic_and_crowded_noise_candidates() {
    for fields in [1, 2] {
        for mode in [1, 2, 3, 4, 6, 7] {
            let error = estimate(
                &input(fields, mode, 7193),
                &mut ExecutionContext::default(),
                false,
            )
            .unwrap_err();
            assert!(
                matches!(
                    error,
                    ProcessingError::NoiseEstimation(NusNoiseError::Quality { .. })
                ),
                "{error:?}"
            );
        }
        let out = estimate(
            &input(fields, 5, 7193),
            &mut ExecutionContext::default(),
            false,
        )
        .unwrap();
        assert!((out.sigma - 1.0).abs() < 0.08);
    }
}
#[test]
fn grouping_is_rotation_covariant_and_nonfinite_inputs_are_rejected() {
    for fields in [1, 2] {
        let original = input(fields, 0, 7193);
        let a = estimate(&original, &mut ExecutionContext::default(), false).unwrap();
        let mut y = original.components().to_vec();
        let (s, c) = 0.7193_f64.sin_cos();
        for pair in y.chunks_exact_mut(2) {
            let (x, z) = (pair[0], pair[1]);
            pair[0] = c * x - s * z;
            pair[1] = s * x + c * z;
        }
        let rotated =
            IstInput::with_grid(192, 128, fields, original.measured_indices().to_vec(), y).unwrap();
        let b = estimate(&rotated, &mut ExecutionContext::default(), false).unwrap();
        assert!((a.sigma - b.sigma).abs() < 1e-12);
        assert_eq!(a.frequency_ranges, b.frequency_ranges);
    }
    assert!(
        IstInput::with_grid(48, 32, 1, (0..48).collect(), vec![f64::NAN; 48 * 32 * 2]).is_err()
    );
}

#[test]
fn report_validation_rejects_nonfinite_and_overflowing_archived_counts() {
    let report = estimate(&input(2, 0, 7193), &mut ExecutionContext::default(), false).unwrap();
    let settings = NusSettings {
        max_iterations: 1000,
        noise_standard_deviation: None,
    };
    assert!(report.validate(128, settings));
    let mut forged = report.clone();
    forged.effective_observations = usize::MAX;
    assert!(!forged.validate(128, settings));
    let mut forged = report.clone();
    forged.sigma = f64::NAN;
    assert!(!forged.validate(128, settings));
    let mut forged = report;
    forged.frequency_ranges[0] = (0, 129);
    assert!(!forged.validate(128, settings));
}

#[test]
fn short_holdout_sigma_bias_dispersion_rotation_and_failure_guards() {
    for fields in [1, 2] {
        let mut errors = Vec::new();
        for m in [32, 33, 40, 47] {
            for seed in 1..=24 {
                let data = input_shape(fields, 0, seed, m, 256);
                let report = estimate(&data, &mut ExecutionContext::default(), false).unwrap();
                assert_eq!(report.source, NusNoiseSource::SplitHoldoutV1);
                assert_eq!(report.effective_observations, m / 2);
                assert!(report.validate(
                    256,
                    NusSettings {
                        max_iterations: 1000,
                        noise_standard_deviation: None
                    }
                ));
                errors.push(report.sigma - 1.0);
            }
        }
        let bias = errors.iter().sum::<f64>() / errors.len() as f64;
        let sd =
            (errors.iter().map(|v| (v - bias).powi(2)).sum::<f64>() / errors.len() as f64).sqrt();
        assert!(bias.abs() < 0.02 && sd < 0.04, "bias={bias} sd={sd}");
        for mode in [1, 2, 3, 4, 6, 7] {
            assert!(
                estimate(
                    &input_shape(fields, mode, 7193, 32, 128),
                    &mut ExecutionContext::default(),
                    false
                )
                .is_err(),
                "mode={mode}"
            );
        }
        let data = input_shape(fields, 5, 7193, 32, 128);
        let a = estimate(&data, &mut ExecutionContext::default(), false).unwrap();
        let mut y = data.components().to_vec();
        for z in y.chunks_exact_mut(2) {
            let (x, y) = (z[0], z[1]);
            z[0] = (x - y) / 2.0_f64.sqrt();
            z[1] = (x + y) / 2.0_f64.sqrt();
        }
        let rotated =
            IstInput::with_grid(192, 128, fields, data.measured_indices().to_vec(), y).unwrap();
        let b = estimate(&rotated, &mut ExecutionContext::default(), false).unwrap();
        assert!((a.sigma - b.sigma).abs() < 1e-12);
        assert_eq!(a.frequency_ranges, b.frequency_ranges);
    }
    for (m, f) in [(31, 256), (32, 127)] {
        assert!(matches!(
            estimate(
                &input_shape(2, 0, 1, m, f),
                &mut ExecutionContext::default(),
                false
            ),
            Err(ProcessingError::NoiseEstimation(
                NusNoiseError::InsufficientSamples
            ))
        ));
    }
}

#[test]
fn jeol_interior_noise_ignores_filter_stopband_but_keeps_quality_guards() {
    for fields in [1, 2] {
        for m in [32, 64] {
            let base = input_shape(fields, 0, 7193, m, 256);
            let mut y = base.components().to_vec();
            for row in y.chunks_exact_mut(256 * fields * 2) {
                for (p, bin) in row.chunks_exact_mut(fields * 2).enumerate() {
                    if !(32..224).contains(&p) {
                        for v in bin {
                            *v *= 0.01;
                        }
                    }
                }
            }
            let filtered =
                IstInput::with_grid(192, 256, fields, base.measured_indices().to_vec(), y).unwrap();
            assert!(estimate(&filtered, &mut ExecutionContext::default(), false).is_err());
            let r = estimate(&filtered, &mut ExecutionContext::default(), true).unwrap();
            assert!(r.source.interior());
            assert!((r.sigma - 1.0).abs() < 0.08);
            for mode in [1, 3, 4, 6, 7] {
                assert!(
                    estimate(
                        &input_shape(fields, mode, 7193, m, 256),
                        &mut ExecutionContext::default(),
                        true
                    )
                    .is_err(),
                    "mode={mode}"
                );
            }
        }
    }
}
