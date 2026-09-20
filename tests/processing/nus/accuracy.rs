//! Independent analytic release gates. Expected signals use closed-form
//! physical line shapes or tones, never the processing kernels under test.

use nmr::Complex64;
use nmr::processing::{
    IstError, IstInput, IstOptions, PhaseCovariantGroupRetainedIstV1, WorkLedger,
};

#[test]
fn general_ist_recovers_weak_and_strong_tone_amplitude_area_and_missing_values() {
    use nmr::processing::GeneralGridPhaseCovariantGroupRetainedIstV1;
    for (n, indices) in [
        (4, vec![3, 0, 1]),
        (8, vec![7, 0, 1, 3, 4, 6]),
        (15, vec![14, 0, 1, 2, 4, 5, 7, 8, 10, 11, 13]),
        (512, (0..128).map(|i| i * 73 % 512).collect()),
    ] {
        let truth = |time: usize, column: usize| {
            Complex64::from_polar(
                if column == 0 { 1.0 } else { 100.0 },
                std::f64::consts::TAU * (column + 1) as f64 * time as f64 / n as f64,
            )
        };
        let mut measured = vec![];
        for &i in &indices {
            for column in 0..2 {
                let z = truth(i, column);
                measured.extend([z.re, z.im]);
            }
        }
        let input = IstInput::with_grid(n, 2, 1, indices.clone(), measured).unwrap();
        let output = GeneralGridPhaseCovariantGroupRetainedIstV1
            .reconstruct_with_context(
                &input,
                IstOptions::new()
                    .max_iterations(1000)
                    .unwrap()
                    .noise_standard_deviation(0.0)
                    .unwrap(),
                &mut nmr::ExecutionContext::default(),
            )
            .unwrap();
        for column in 0..2 {
            let amplitude = if column == 0 { 1.0 } else { 100.0 };
            let mut missing_error = 0.0;
            let mut missing = 0;
            let mut coherent = Complex64::default();
            let mut area = 0.0;
            for time in 0..n {
                let z = Complex64::new(
                    output.components()[time * 4 + column * 2],
                    output.components()[time * 4 + column * 2 + 1],
                );
                let expected = truth(time, column);
                coherent += z * expected.conj() / amplitude;
                area += z.norm() / amplitude;
                if !indices.contains(&time) {
                    missing_error += (z - expected).norm_sqr() / amplitude.powi(2);
                    missing += 1;
                }
            }
            let missing_nrms = (missing_error / missing as f64).sqrt();
            let peak_ratio = coherent.norm() / (n as f64 * amplitude);
            let area_ratio = area / n as f64;
            println!(
                "general IST N={n} M={} column={column} missing_NRMS={missing_nrms:.9} peak={peak_ratio:.9} normalized_time_area={area_ratio:.9}",
                indices.len()
            );
            assert!(missing_nrms < 1e-5);
            assert!((peak_ratio - 1.0).abs() < 1e-5);
            assert!((area_ratio - 1.0).abs() < 1e-5);
        }
    }
}

fn true_tone(time: usize) -> Complex64 {
    Complex64::from_polar(1.0, std::f64::consts::TAU * 73.0 * time as f64 / 512.0)
}

fn tone_input(f2: usize, stride: usize, occupied: usize) -> IstInput {
    let measured: Vec<_> = (0..128).map(|index| index * stride % 512).collect();
    let mut components = vec![0.0; 128 * f2 * 2];
    for (row, &time) in measured.iter().enumerate() {
        let z = true_tone(time);
        for column in 0..occupied {
            let offset = (row * f2 + column) * 2;
            components[offset] = z.re;
            components[offset + 1] = z.im;
        }
    }
    IstInput::new(f2, 1, measured, components).unwrap()
}

#[test]
fn ist_retains_absolute_tone_amplitude_independently_of_zero_columns_and_occupancy() {
    for stride in [73, 137] {
        let mut reference: Option<Vec<Complex64>> = None;
        for (f2, occupied) in [(1, 1), (8, 1), (8, 8)] {
            let output = PhaseCovariantGroupRetainedIstV1
                .reconstruct(
                    &tone_input(f2, stride, occupied),
                    &mut WorkLedger::new(u128::MAX),
                    IstOptions::new().noiseless(),
                )
                .unwrap();
            let reconstructed: Vec<_> = (0..512)
                .map(|time| {
                    Complex64::new(
                        output.components()[time * f2 * 2],
                        output.components()[time * f2 * 2 + 1],
                    )
                })
                .collect();
            let error = (reconstructed
                .iter()
                .enumerate()
                .map(|(time, value)| (value - true_tone(time)).norm_sqr())
                .sum::<f64>()
                / 512.0)
                .sqrt();
            let peak_ratio = reconstructed
                .iter()
                .enumerate()
                .map(|(time, value)| (value * true_tone(time).conj()).re)
                .sum::<f64>()
                / 512.0;
            assert!(error < 1e-5, "stride={stride} F2={f2} L2={error}");
            assert!((peak_ratio - 1.0).abs() < 1e-5, "peak ratio={peak_ratio}");
            assert_eq!(output.noise_standard_deviation(), 0.0);
            assert_eq!(output.measured_relative_residual(), 0.0);
            assert!(output.relative_change() <= 1e-6);
            if let Some(reference) = &reference {
                for (actual, expected) in reconstructed.iter().zip(reference) {
                    assert!((actual - expected).norm() < 1e-12);
                }
            } else {
                reference = Some(reconstructed);
            }
            println!(
                "IST stride={stride} F2={f2} occupied={occupied}: peak={peak_ratio:.9}, L2={error:.9}"
            );
        }
    }
}

#[test]
fn ist_requires_independent_noise_evidence_before_work() {
    let input = tone_input(1, 73, 1);
    let mut work = WorkLedger::new(u128::MAX);
    assert_eq!(
        PhaseCovariantGroupRetainedIstV1
            .reconstruct(&input, &mut work, IstOptions::new())
            .unwrap_err(),
        IstError::NoiseEstimateRequired
    );
    assert_eq!(work.used(), 0);
    for sigma in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            IstOptions::new()
                .noise_standard_deviation(sigma)
                .unwrap_err(),
            IstError::InvalidOptions
        );
    }
}

#[test]
fn ist_supplied_noise_threshold_is_invariant_to_appended_zero_columns() {
    let mut reference = None;
    for f2 in [1, 8] {
        let original = tone_input(f2, 73, 1);
        let mut samples = original.components().to_vec();
        let mut state = 0x65abc012_u64;
        for row in 0..128 {
            for component in 0..2 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let uniform = (state >> 11) as f64 / (1_u64 << 53) as f64 * 2.0 - 1.0;
                samples[row * f2 * 2 + component] += 0.002 * uniform;
            }
        }
        let sigma = 0.002 / 3.0_f64.sqrt();
        let input = IstInput::new(f2, 1, original.measured_indices().to_vec(), samples).unwrap();
        let output = PhaseCovariantGroupRetainedIstV1
            .reconstruct(
                &input,
                &mut WorkLedger::new(u128::MAX),
                IstOptions::new().noise_standard_deviation(sigma).unwrap(),
            )
            .unwrap();
        assert_eq!(output.noise_standard_deviation(), sigma);
        let trace: Vec<_> = (0..512)
            .map(|time| {
                Complex64::new(
                    output.components()[time * f2 * 2],
                    output.components()[time * f2 * 2 + 1],
                )
            })
            .collect();
        let error = (trace
            .iter()
            .enumerate()
            .map(|(time, actual)| (actual - true_tone(time)).norm_sqr())
            .sum::<f64>()
            / 512.0)
            .sqrt();
        assert!(error < 0.002, "noisy tone relative L2={error}");
        if let Some((threshold, previous)) = &reference {
            assert_eq!(output.final_threshold(), *threshold);
            assert_eq!(&trace, previous);
        } else {
            reference = Some((output.final_threshold(), trace));
        }
    }
}

#[test]
fn ist_weak_column_is_unchanged_by_independent_thousandfold_and_millionfold_peaks() {
    for sigma in [0.0, 1e-4] {
        let mut reference = None;
        for strong in [None, Some(1e3), Some(1e6)] {
            let f2 = if strong.is_some() { 2 } else { 1 };
            let measured: Vec<_> = (0..128).map(|index| index * 73 % 512).collect();
            let mut samples = vec![0.0; 128 * f2 * 2];
            let mut state = 0x65abc012_u64;
            for (row, &time) in measured.iter().enumerate() {
                let signal = true_tone(time);
                for (component, value) in [signal.re, signal.im].into_iter().enumerate() {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1);
                    let uniform = (state >> 11) as f64 / (1_u64 << 53) as f64 * 2.0 - 1.0;
                    samples[row * f2 * 2 + component] = value + sigma * 3.0_f64.sqrt() * uniform;
                }
                if let Some(amplitude) = strong {
                    let value = Complex64::from_polar(
                        amplitude,
                        std::f64::consts::TAU * 97.0 * time as f64 / 512.0,
                    );
                    samples[(row * f2 + 1) * 2] = value.re;
                    samples[(row * f2 + 1) * 2 + 1] = value.im;
                }
            }
            let input = IstInput::new(f2, 1, measured, samples).unwrap();
            let output = PhaseCovariantGroupRetainedIstV1
                .reconstruct(
                    &input,
                    &mut WorkLedger::new(u128::MAX),
                    IstOptions::new().noise_standard_deviation(sigma).unwrap(),
                )
                .unwrap();
            let weak: Vec<_> = (0..512)
                .map(|time| {
                    Complex64::new(
                        output.components()[time * f2 * 2],
                        output.components()[time * f2 * 2 + 1],
                    )
                })
                .collect();
            let error = (weak
                .iter()
                .enumerate()
                .map(|(time, value)| (value - true_tone(time)).norm_sqr())
                .sum::<f64>()
                / 512.0)
                .sqrt();
            let peak_ratio = weak
                .iter()
                .enumerate()
                .map(|(time, value)| (value * true_tone(time).conj()).re)
                .sum::<f64>()
                / 512.0;
            let tolerance = if sigma == 0.0 { 1e-5 } else { 2e-4 };
            assert!(
                error < tolerance,
                "sigma={sigma} strong={strong:?} L2={error}"
            );
            assert!((peak_ratio - 1.0).abs() < tolerance);
            if let Some((previous, threshold, iterations)) = &reference {
                assert_eq!(&weak, previous);
                assert_eq!(output.column_final_threshold(0), *threshold);
                assert_eq!(output.column_iterations(0), *iterations);
            } else {
                reference = Some((
                    weak,
                    output.column_final_threshold(0),
                    output.column_iterations(0),
                ));
            }
            assert!(output.relative_change() <= 1e-6);
            assert_eq!(output.column_final_threshold(f2), None);
            if sigma > 0.0 && strong.is_some() {
                assert!(
                    output.column_iterations(0).unwrap() < output.column_iterations(1).unwrap(),
                    "independent stopping must not hide a weak column in global energy"
                );
            }
            println!(
                "IST dynamic sigma={sigma} strong={strong:?}: weak peak={peak_ratio:.9}, L2={error:.9}, iterations={:?}",
                output.column_iterations(0)
            );
        }
    }
}
