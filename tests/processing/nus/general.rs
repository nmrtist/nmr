//! Public NUS reconstruction and retained observation contracts.
use nmr::processed::ProcessedOrigin;
use nmr::processing::{ProcessingOperation as Op, *};
use nmr::snapshot::{self, AcceptRecordedHistory, SnapshotLimits};
use nmr::{CancellationToken, ExecutionContext};

use crate::spectrum_support::*;

#[test]
fn grouped_columns_match_separate_reconstruction_including_retained_signed_zeros() {
    // Unequal dimensions, two grouped fields, unsorted observations and a zero
    // column exercise both transposes and freezing before neighbouring columns.
    for fields in [1, 2] {
        let n = 31;
        let coordinates: Vec<_> = (0..23).map(|i| i * 7 % n).collect();
        let mut columns = Vec::new();
        for column in 0..3 {
            let mut values = Vec::new();
            for &time in &coordinates {
                for field in 0..fields {
                    let z = if column == 1 {
                        nmr::Complex64::new(-0.0, 0.0)
                    } else {
                        nmr::Complex64::from_polar(
                            (column + field + 1) as f64,
                            std::f64::consts::TAU * (3 + column + field) as f64 * time as f64
                                / n as f64,
                        )
                    };
                    values.extend([z.re, z.im]);
                }
            }
            columns.push(values);
        }
        let reconstruct = |count, values| {
            let input = IstInput::with_grid(n, count, fields, coordinates.clone(), values).unwrap();
            let output = GeneralGridPhaseCovariantGroupRetainedIstV1
                .reconstruct_with_context(
                    &input,
                    IstOptions::new().noiseless(),
                    &mut ExecutionContext::default(),
                )
                .unwrap();
            for (m, &time) in coordinates.iter().enumerate() {
                for c in 0..count * fields * 2 {
                    assert_eq!(
                        input.components()[m * count * fields * 2 + c].to_bits(),
                        output.components()[time * count * fields * 2 + c].to_bits()
                    );
                }
            }
            output
        };
        let mut together = Vec::new();
        for m in 0..coordinates.len() {
            for column in &columns {
                together.extend_from_slice(&column[m * fields * 2..(m + 1) * fields * 2]);
            }
        }
        let combined = reconstruct(3, together);
        for (c, values) in columns.into_iter().enumerate() {
            let separate = reconstruct(1, values);
            assert_eq!(combined.column_iterations(c), separate.column_iterations(0));
            assert_eq!(
                combined.column_final_threshold(c),
                separate.column_final_threshold(0)
            );
            assert_eq!(
                combined.column_relative_change(c),
                separate.column_relative_change(0)
            );
            for time in 0..n {
                for k in 0..fields * 2 {
                    assert_eq!(
                        combined.components()[(time * 3 + c) * fields * 2 + k].to_bits(),
                        separate.components()[time * fields * 2 + k].to_bits()
                    );
                }
            }
        }
    }
}

#[test]
fn general_ist_accepts_low_signal_columns_without_discarding_observations() {
    // Deterministic impulse: every unnormalised Fourier bin has magnitude 0.01,
    // below the sigma=1 threshold. Soft thresholding gives zero; replacing the
    // measured values therefore makes the zero-filled observation a fixed point.
    for mixed in [false, true] {
        let indices = vec![7, 0, 1, 3, 4, 6];
        let mut samples = vec![0.0; indices.len() * 3 * 2];
        for (row, &coordinate) in indices.iter().enumerate() {
            if mixed {
                let phase = std::f64::consts::TAU * coordinate as f64 / 8.0;
                samples[row * 6] = 100.0 * phase.cos();
                samples[row * 6 + 1] = 100.0 * phase.sin();
            }
        }
        samples[2] = 0.01;
        let input = IstInput::with_grid(8, 3, 1, indices.clone(), samples.clone()).unwrap();
        let output = GeneralGridPhaseCovariantGroupRetainedIstV1
            .reconstruct_with_context(
                &input,
                IstOptions::new().noise_standard_deviation(1.0).unwrap(),
                &mut ExecutionContext::default(),
            )
            .unwrap();
        for row in 0..8 {
            for column in 1..3 {
                for component in 0..2 {
                    let expected = indices
                        .iter()
                        .position(|&i| i == row)
                        .map_or(0.0, |m| samples[m * 6 + column * 2 + component]);
                    assert_eq!(
                        output.components()[row * 6 + column * 2 + component],
                        expected
                    );
                }
            }
        }
        assert_eq!(output.column_iterations(1), Some(3));
        assert_eq!(output.column_iterations(2), Some(0));
        assert_eq!(output.measured_relative_residual(), 0.0);
    }
}

#[test]
fn general_ist_pure_gaussian_noise_has_retained_data_and_no_invented_missing_signal() {
    let indices = vec![14, 0, 1, 2, 4, 5, 7, 8, 10, 11, 13];
    for fields in [1, 2] {
        let mut state = 0x1234_5678_u64;
        let mut uniform = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            ((state >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let mut noise = Vec::new();
        for _ in 0..indices.len() * fields {
            let radius = (-2.0 * uniform().ln()).sqrt();
            let angle = std::f64::consts::TAU * uniform();
            noise.extend([radius * angle.cos(), radius * angle.sin()]);
        }
        for scale in [1e-6, 1.0, 1e6] {
            let samples: Vec<_> = noise.iter().map(|v| v * scale).collect();
            let input =
                IstInput::with_grid(15, 1, fields, indices.clone(), samples.clone()).unwrap();
            let output = GeneralGridPhaseCovariantGroupRetainedIstV1
                .reconstruct_with_context(
                    &input,
                    IstOptions::new().noise_standard_deviation(scale).unwrap(),
                    &mut ExecutionContext::default(),
                )
                .unwrap();
            for row in 0..15 {
                for component in 0..fields * 2 {
                    let expected = indices
                        .iter()
                        .position(|&i| i == row)
                        .map_or(0.0, |m| samples[m * fields * 2 + component]);
                    assert_eq!(output.components()[row * fields * 2 + component], expected);
                }
            }
            assert_eq!(output.iterations(), 3);
            let failure = GeneralGridPhaseCovariantGroupRetainedIstV1.reconstruct_with_context(
                &input,
                IstOptions::new()
                    .noise_standard_deviation(scale)
                    .unwrap()
                    .max_iterations(2)
                    .unwrap(),
                &mut ExecutionContext::default(),
            );
            assert_eq!(failure.unwrap_err(), IstError::DidNotConverge);
        }
    }
}

#[test]
fn general_nus_public_path_retains_observation_order_and_recovers_analytic_tone() {
    for (n, indices) in [
        (4, vec![3, 0, 1]),
        (8, vec![7, 0, 1, 3, 4, 6]),
        (15, vec![14, 0, 1, 2, 4, 5, 7, 8, 10, 11, 13]),
        (512, (0..128).map(|i| i * 73 % 512).collect()),
    ] {
        let input = nus_input(n, &indices);
        let settings = NusSettings {
            max_iterations: 1000,
            noise_standard_deviation: Some(0.0),
        };
        let direct = ProcessingPlan::new(vec![Op::FourierTransform {
            axis: 1,
            transform: FourierTransform::default(),
        }])
        .unwrap();
        let prepared = settings
            .prepare(&input, direct, ProcessingOptions::new())
            .unwrap();
        assert_eq!(prepared.measured_indices(), indices);
        let output = prepared.execute().unwrap();
        let view = output.as_dense_processed().unwrap();
        for row in 0..n {
            let phase = 2.0 * std::f64::consts::PI * row as f64 / n as f64;
            // Negative exponent direct FFT has the tone at centered bin +1, index 2.
            close(
                &[
                    view.samples()[(row * 2) * 6 + 4],
                    view.samples()[(row * 2 + 1) * 6 + 4],
                ],
                &[3.0 * phase.cos(), 3.0 * phase.sin()],
                1e-5,
            );
        }
        let f1 = ProcessingPlan::new(vec![Op::FourierTransform {
            axis: 0,
            transform: FourierTransform::default(),
        }])
        .unwrap()
        .apply(&output)
        .unwrap();
        let mut bytes = Vec::new();
        snapshot::write_snapshot(&f1, &mut bytes, SnapshotLimits::default()).unwrap();
        let restored = snapshot::read_snapshot(&mut bytes.as_slice(), SnapshotLimits::default())
            .unwrap()
            .restore(AcceptRecordedHistory);
        assert_eq!(restored.canonical_digests(), f1.canonical_digests());
        let ProcessedOrigin::Library(boundary) =
            output.as_processed().unwrap().provenance().origin()
        else {
            panic!("NUS evidence missing")
        };
        let replay = boundary
            .replay(
                &[&input],
                ProcessingOptions::new(),
                &mut ExecutionContext::default(),
            )
            .unwrap();
        close(values(&replay), values(&output), 0.0);
        println!(
            "general NUS grid={n}, M={} analytic missing-point tolerance=1e-5 passed",
            indices.len()
        );
    }
}

#[test]
fn nus_general_budget_full_sampling_duplicates_and_stage_cancellation() {
    let input = nus_input(8, &[7, 0, 1, 3, 4, 6]);
    let options = ProcessingOptions::new();
    let settings = NusSettings {
        max_iterations: 1000,
        noise_standard_deviation: Some(0.0),
    };
    let plan = || {
        ProcessingPlan::new(vec![Op::FourierTransform {
            axis: 1,
            transform: FourierTransform::default(),
        }])
        .unwrap()
    };
    let prepared = settings.prepare(&input, plan(), options).unwrap();
    let r = prepared.resources();
    let work = prepared.estimated_work();
    let exact = options
        .max_output_bytes(r.output_bytes())
        .max_metadata_bytes(r.metadata_bytes())
        .max_working_bytes(r.working_bytes());
    let mut ledger = WorkLedger::new(work);
    let mut context = ExecutionContext::new(&mut ledger);
    settings
        .prepare(&input, plan(), exact)
        .unwrap()
        .execute_with_context(&mut context)
        .unwrap();
    for options in [
        options.max_output_bytes(0),
        options.max_metadata_bytes(0),
        options.max_working_bytes(0),
    ] {
        assert!(settings.prepare(&input, plan(), options).is_err());
    }
    assert!(
        NusSettings::default()
            .prepare(&input, plan(), options)
            .is_err()
    );
    assert!(
        settings
            .prepare(&nus_input(8, &[1, 1]), plan(), options)
            .is_err()
    );
    for stage in [
        nmr::execution::ExecutionStage::Processing,
        nmr::execution::ExecutionStage::Reconstruction,
    ] {
        let token = CancellationToken::new();
        let cancel = token.clone();
        let mut saw = false;
        let mut callback = |event: nmr::execution::ProgressEvent| {
            if event.stage == stage {
                saw = true;
                cancel.cancel();
            }
        };
        let mut ledger = WorkLedger::new(u128::MAX);
        let mut control = ExecutionContext::new(&mut ledger)
            .with_cancellation(token)
            .with_progress(&mut callback);
        let error = settings
            .prepare(&input, plan(), options)
            .unwrap()
            .execute_with_context(&mut control)
            .unwrap_err();
        assert_eq!(error.code(), ProcessingErrorCode::Cancelled);
        assert!(saw);
    }
    for n in [4, 8, 15] {
        let full = nus_input(n, &(0..n).rev().collect::<Vec<_>>());
        let output = settings
            .prepare(&full, plan(), options)
            .unwrap()
            .execute()
            .unwrap();
        for row in 0..n {
            let phase = std::f64::consts::TAU * row as f64 / n as f64;
            close(
                &[
                    values(&output)[row * 12 + 4],
                    values(&output)[row * 12 + 10],
                ],
                &[3.0 * phase.cos(), 3.0 * phase.sin()],
                1e-12,
            );
        }
        let token = CancellationToken::new();
        token.cancel();
        let mut ledger = WorkLedger::new(u128::MAX);
        let mut control = ExecutionContext::new(&mut ledger).with_cancellation(token);
        assert_eq!(
            ProcessingPlan::new(vec![Op::FourierTransform {
                axis: 0,
                transform: FourierTransform::default()
            }])
            .unwrap()
            .apply_with_context(&output, options, &mut control)
            .unwrap_err()
            .code(),
            ProcessingErrorCode::Cancelled
        );
    }
    assert!(IstInput::with_grid(4, 1, 1, vec![4], vec![0.0; 2]).is_err());
    let zero = IstInput::with_grid(4, 1, 1, vec![3, 1], vec![0.0; 4]).unwrap();
    let result = GeneralGridPhaseCovariantGroupRetainedIstV1
        .reconstruct_with_context(
            &zero,
            IstOptions::new().noise_standard_deviation(0.0).unwrap(),
            &mut ExecutionContext::default(),
        )
        .unwrap();
    assert_eq!(result.components(), [0.0; 8]);
    assert_eq!(result.iterations(), 0);
}

#[test]
fn nus_preparation_shares_cancellation_and_preserves_the_default_contract() {
    let input = nus_input(8, &[7, 0, 1, 3, 4, 6]);
    let settings = NusSettings {
        max_iterations: 1000,
        noise_standard_deviation: Some(0.0),
    };
    let options = ProcessingOptions::new();
    let plan = || {
        ProcessingPlan::new(vec![Op::FourierTransform {
            axis: 1,
            transform: FourierTransform::default(),
        }])
        .unwrap()
    };
    let ordinary = settings.prepare(&input, plan(), options).unwrap();
    let controlled = {
        // Returning the preparation outlives both the context and its ledger.
        let mut ledger = WorkLedger::new(0);
        let mut context = ExecutionContext::new(&mut ledger);
        let prepared = settings
            .prepare_with_context(&input, plan(), options, &mut context)
            .unwrap();
        assert_eq!(context.ledger().used(), 0);
        prepared
    };
    assert_eq!(ordinary.resources(), controlled.resources());
    assert_eq!(ordinary.estimated_work(), controlled.estimated_work());
    assert_eq!(ordinary.measured_indices(), &[7, 0, 1, 3, 4, 6]);
    assert_eq!(ordinary.measured_indices(), controlled.measured_indices());
    assert_eq!(ordinary.output_descriptor(), controlled.output_descriptor());
    for (old, new) in ordinary.direct_steps().zip(controlled.direct_steps()) {
        assert_eq!(old.requested(), new.requested());
        assert_eq!(old.resolved(), new.resolved());
        assert_eq!(old.algorithm_version(), new.algorithm_version());
    }
    let old = ordinary.execute().unwrap();
    let new = controlled.execute().unwrap();
    assert_eq!(old, new);

    let token = CancellationToken::new();
    token.cancel();
    let mut ledger = WorkLedger::new(0);
    let mut context = ExecutionContext::new(&mut ledger).with_cancellation(token);
    let error = settings
        .prepare_with_context(&input, plan(), options.max_metadata_bytes(0), &mut context)
        .unwrap_err();
    assert_eq!(error.code(), ProcessingErrorCode::Cancelled);
    assert_eq!(context.ledger().used(), 0);

    let large = nus_input(8193, &(0..8193).rev().collect::<Vec<_>>());
    let token = CancellationToken::new();
    let cancel = token.clone();
    let mut observed = 0;
    let mut progress = |event: nmr::execution::ProgressEvent| {
        if event.stage == nmr::execution::ExecutionStage::Preflight && event.completed > 0 {
            observed = event.completed;
            cancel.cancel();
        }
    };
    let mut ledger = WorkLedger::new(0);
    let mut context = ExecutionContext::new(&mut ledger)
        .with_cancellation(token)
        .with_progress(&mut progress);
    let error = settings
        .prepare_with_context(&large, plan(), options, &mut context)
        .unwrap_err();
    assert_eq!(error.code(), ProcessingErrorCode::Cancelled);
    assert_eq!(context.ledger().used(), 0);
    assert!(observed > 0 && observed < 8193);
}

#[test]
fn accelerated_general_ist_matches_independent_fixed_threshold_iteration() {
    use nmr::Complex64;
    let n = 31;
    let indices: Vec<_> = (0..20).map(|i| i * 7 % n).collect();
    let measured: Vec<_> = indices
        .iter()
        .map(|&i| {
            Complex64::from_polar(
                (-(i as f64) / 12.0).exp(),
                std::f64::consts::TAU * 7.3 * i as f64 / n as f64,
            )
        })
        .collect();
    let components = measured.iter().flat_map(|z| [z.re, z.im]).collect();
    let input = IstInput::with_grid(n, 1, 1, indices.clone(), components).unwrap();
    let sigma = 0.01;
    let output = GeneralGridPhaseCovariantGroupRetainedIstV1
        .reconstruct_with_context(
            &input,
            IstOptions::new()
                .noise_standard_deviation(sigma)
                .unwrap()
                .max_iterations(1000)
                .unwrap(),
            &mut ExecutionContext::default(),
        )
        .unwrap();
    let log = (n as f64).ln();
    let lambda =
        sigma * (indices.len() as f64).sqrt() * (2.0 + 2.0 * (2.0 * log).sqrt() + 2.0 * log).sqrt();
    assert!((output.final_threshold() - lambda).abs() < 1e-12);
    // Independent O(N^2) Fourier sums, not the library FFT or shrinkage code.
    let matrix: Vec<_> = (0..n)
        .flat_map(|k| {
            (0..n).map(move |t| {
                Complex64::from_polar(1.0, -std::f64::consts::TAU * (k * t) as f64 / n as f64)
            })
        })
        .collect();
    let map = |x: &[Complex64]| {
        let spectrum: Vec<_> = (0..n)
            .map(|k| {
                let z: Complex64 = (0..n).map(|t| x[t] * matrix[k * n + t]).sum();
                if z.norm() <= lambda {
                    Complex64::default()
                } else {
                    z * (1.0 - lambda / z.norm())
                }
            })
            .collect();
        let mut next: Vec<_> = (0..n)
            .map(|t| {
                (0..n)
                    .map(|k| spectrum[k] * matrix[k * n + t].conj() / n as f64)
                    .sum()
            })
            .collect();
        for (&i, &z) in indices.iter().zip(&measured) {
            next[i] = z;
        }
        next
    };
    let norm = |x: &[Complex64]| x.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt();
    let difference = |a: &[Complex64], b: &[Complex64]| {
        a.iter()
            .zip(b)
            .map(|(a, b)| (*a - *b).norm_sqr())
            .sum::<f64>()
            .sqrt()
    };
    let actual: Vec<_> = output
        .components()
        .chunks_exact(2)
        .map(|v| Complex64::new(v[0], v[1]))
        .collect();
    assert!(difference(&map(&actual), &actual) / norm(&actual) < 1.01e-6);
    let mut reference = vec![Complex64::default(); n];
    for (&i, &z) in indices.iter().zip(&measured) {
        reference[i] = z;
    }
    let mut converged = false;
    for _ in 0..10000 {
        let next = map(&reference);
        let residual = difference(&next, &reference) / norm(&reference);
        reference = next;
        if residual < 1e-10 {
            converged = true;
            break;
        }
    }
    assert!(converged, "independent slow reference must converge");
    assert!(difference(&actual, &reference) / norm(&reference) < 1e-3);
}
