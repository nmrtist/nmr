//! Analytic Gaussian-noise and retained Fourier-sum acceptance, no vendor files.
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::derivation::DerivationOperation;
use nmr::processed::ProcessedOrigin;
use nmr::processing::{ProcessingOperation as Op, *};
use nmr::raw::*;
use nmr::{Complex64, Dataset, ExecutionContext};

fn raw(n: usize, m: usize, f: usize, sigma: f64, seed: u64, signal: bool) -> Dataset {
    let axis = |kind, points| {
        RawAxis::new(
            kind,
            AxisDomain::Time,
            Some(AxisUnit::Second),
            points,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.001,
            },
        )
        .unwrap()
    };
    let axes = vec![
        axis(
            RawAxisKind::Indirect(IndirectComponents::Cartesian(
                ComponentEvidence::user_constructed(),
            )),
            n,
        ),
        axis(RawAxisKind::Direct(DirectSamples::Complex), f),
    ];
    let indices: Vec<_> = (0..m).map(|i| i * 73 % n).collect();
    let coordinates: Vec<_> = indices
        .iter()
        .map(|&i| SamplingCoordinate::new(vec![i]))
        .collect();
    let mut rng = seed;
    let mut uniform = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    let mut traces = Vec::new();
    for (ordinal, &i) in indices.iter().enumerate() {
        let mut samples = Vec::new();
        for lane in 0..2 {
            for j in 0..f {
                let mut z = Complex64::default();
                if signal {
                    for (a, f1, f2) in [(1.0, 7.0, 13.0), (0.05, 29.0, -19.0)] {
                        let angle = std::f64::consts::TAU * f1 * i as f64 / n as f64;
                        z += Complex64::from_polar(
                            a * if lane == 0 { angle.cos() } else { angle.sin() },
                            std::f64::consts::TAU * f2 * j as f64 / f as f64,
                        );
                    }
                }
                z += Complex64::from_polar(
                    sigma * (-2.0 * uniform().ln()).sqrt(),
                    std::f64::consts::TAU * uniform(),
                );
                samples.push(z);
            }
        }
        traces.push(SparseTrace::new(
            ObservationOrdinal::new(ordinal),
            coordinates[ordinal].clone(),
            samples,
        ));
    }
    RawDatasetBuilder::new(axes, RawMetadata::default())
        .unwrap()
        .sparse(traces, SamplingSchedule::new(vec![n], coordinates).unwrap())
        .unwrap()
        .into()
}
fn plan(mut ops: Vec<Op>) -> ProcessingPlan {
    ops.push(Op::FourierTransform {
        axis: 1,
        transform: FourierTransform::default(),
    });
    ProcessingPlan::new(ops).unwrap()
}
fn execute(input: &Dataset, plan: ProcessingPlan) -> Dataset {
    AutoNusSettings::default()
        .prepare(input, plan, ProcessingOptions::new())
        .unwrap()
        .execute()
        .unwrap()
}
fn report(data: &Dataset) -> &NusNoiseReport {
    let ProcessedOrigin::Library(origin) = data.as_processed().unwrap().provenance().origin()
    else {
        panic!()
    };
    let DerivationOperation::NusReconstruction { noise_report, .. } = origin.operation() else {
        panic!()
    };
    noise_report
}

#[test]
fn automatic_gaussian_scale_bias_and_dispersion_across_seeds_and_sampling_rates() {
    // 4 * 4 F2 bins * >=32 held-out rows * 4 components = >=2048 scalars.
    // IID Gaussian RMS has asymptotic relative SD <=1/sqrt(2*2048)=1.57%.
    // Limits allow selection-independent finite-sample fluctuations, not fits.
    let mut errors = Vec::new();
    for m in [96, 128, 192] {
        for seed in 1..=6 {
            let mut reference: Option<f64> = None;
            for sigma in [1e-4, 1.0, 1e4] {
                let data = raw(256, m, 128, sigma, seed, false);
                let out = execute(&data, plan(vec![]));
                let r = report(&out);
                let ratio = r.sigma / (sigma * 128.0_f64.sqrt());
                assert!(
                    (ratio - 1.0).abs() < 0.10,
                    "m={m} seed={seed} ratio={ratio}"
                );
                if let Some(v) = reference {
                    assert!((ratio - v).abs() < 1e-12);
                } else {
                    reference = Some(ratio);
                    errors.push(ratio - 1.0);
                }
                assert_eq!(r.effective_observations, m / 3);
                assert_eq!(r.source, NusNoiseSource::SplitObservationsV1);
            }
        }
    }
    let mean = errors.iter().sum::<f64>() / errors.len() as f64;
    let sd = (errors.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / errors.len() as f64).sqrt();
    println!("Gaussian relative bias={mean}, SD={sd}");
    assert!(mean.abs() < 0.03 && sd < 0.04);
}

#[test]
fn automatic_window_zero_fill_rotation_and_snapshot_replay() {
    let input = raw(128, 96, 128, 0.01, 7193, true);
    let base = execute(&input, plan(vec![]));
    let noise = raw(128, 96, 128, 0.01, 7193, false);
    let noise_base = execute(&noise, plan(vec![]));
    let filled = execute(
        &noise,
        plan(vec![Op::ZeroFill {
            axis: 1,
            zero_fill: ZeroFill::new(256).unwrap(),
        }]),
    );
    assert!((report(&filled).sigma / report(&noise_base).sigma - 1.0).abs() < 0.08);
    assert_eq!(
        report(&filled).effective_observations,
        report(&noise_base).effective_observations
    );
    // Rectangular truncation plus interpolation spreads this strong persistent
    // tone across all candidate regions: it must fail the contamination guard.
    assert!(
        AutoNusSettings::default()
            .prepare(
                &input,
                plan(vec![Op::ZeroFill {
                    axis: 1,
                    zero_fill: ZeroFill::new(256).unwrap()
                }]),
                ProcessingOptions::new()
            )
            .unwrap()
            .analyze()
            .is_err()
    );
    let mut rotated = plan(vec![]).operations().to_vec();
    rotated.push(Op::PhaseCorrection {
        axis: 1,
        correction: PhaseCorrection::new(37.0, 81.0, 0.5).unwrap(),
    });
    let rotated = execute(&input, ProcessingPlan::new(rotated).unwrap());
    assert!((report(&rotated).sigma / report(&base).sigma - 1.0).abs() < 1e-12);
    let windowed = execute(
        &noise,
        plan(vec![Op::Window {
            axis: 1,
            window: Window::exponential(3.0).unwrap(),
        }]),
    );
    let expected = 0.01
        * ((0..128)
            .map(|i| (-2.0 * std::f64::consts::PI * 3.0 * i as f64 * 0.001).exp())
            .sum::<f64>())
        .sqrt();
    assert!((report(&windowed).sigma / expected - 1.0).abs() < 0.10);
    let mut bytes = Vec::new();
    nmr::snapshot::write_snapshot(&base, &mut bytes, Default::default()).unwrap();
    let restored = nmr::snapshot::read_snapshot(&mut bytes.as_slice(), Default::default())
        .unwrap()
        .restore(nmr::snapshot::AcceptRecordedHistory);
    assert_eq!(report(&restored), report(&base));
    let ProcessedOrigin::Library(origin) = restored.as_processed().unwrap().provenance().origin()
    else {
        panic!()
    };
    let replay = origin
        .replay(
            &[&input],
            ProcessingOptions::new(),
            &mut ExecutionContext::default(),
        )
        .unwrap();
    assert_eq!(replay.canonical_digests(), base.canonical_digests());
    assert_eq!(report(&replay), report(&base));
    let f1 = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply(&restored)
    .unwrap();
    assert!(
        f1.as_processed()
            .unwrap()
            .descriptor()
            .axes()
            .iter()
            .all(|a| a.domain() == AxisDomain::Frequency)
    );
}

#[test]
fn automatic_failures_limits_cancellation_and_degenerate_data_are_explicit() {
    let input = raw(128, 96, 64, 1.0, 1, false);
    let options = ProcessingOptions::new();
    let prepared = AutoNusSettings::default()
        .prepare(&input, plan(vec![]), options)
        .unwrap();
    let work = prepared.estimated_work();
    let mut ledger = WorkLedger::new(work - 1);
    assert_eq!(
        prepared
            .execute_with_context(&mut ExecutionContext::new(&mut ledger))
            .unwrap_err()
            .code(),
        ProcessingErrorCode::ResourceLimit
    );
    assert_eq!(ledger.used(), 0);
    assert!(
        AutoNusSettings::default()
            .prepare(&input, plan(vec![]), options.max_working_bytes(0))
            .is_err()
    );
    let token = nmr::CancellationToken::new();
    let cancel = token.clone();
    let mut callback = |event: nmr::execution::ProgressEvent| {
        if event.stage == nmr::execution::ExecutionStage::NoiseEstimation {
            cancel.cancel();
        }
    };
    let mut ledger = WorkLedger::new(u128::MAX);
    let mut control = ExecutionContext::new(&mut ledger)
        .with_cancellation(token)
        .with_progress(&mut callback);
    let error = AutoNusSettings::default()
        .prepare(&input, plan(vec![]), options)
        .unwrap()
        .execute_with_context(&mut control)
        .unwrap_err();
    assert_eq!(error.code(), ProcessingErrorCode::Cancelled);
    let zero = raw(128, 96, 64, 0.0, 1, false);
    let error = AutoNusSettings::default()
        .prepare(&zero, plan(vec![]), options)
        .unwrap()
        .execute()
        .unwrap_err();
    assert_eq!(
        error.root_cause(),
        &ProcessingError::NoiseEstimation(NusNoiseError::DegenerateInput)
    );
    assert!(
        AutoNusSettings::default()
            .prepare(&raw(128, 12, 64, 1.0, 1, false), plan(vec![]), options)
            .is_err()
    );
    let mut ops = plan(vec![]).operations().to_vec();
    ops.push(Op::Spectrum {
        axis: 1,
        operation: SpectrumOperation::Magnitude,
    });
    let error = AutoNusSettings::default()
        .prepare(&input, ProcessingPlan::new(ops).unwrap(), options)
        .unwrap_err();
    assert!(matches!(
        error.root_cause(),
        ProcessingError::NoiseEstimation(NusNoiseError::UnsupportedOperation { .. })
    ));
}

#[test]
fn automatic_strong_and_weak_peaks_match_full_sampling_fourier_reference() {
    let n = 128;
    let f = 128;
    let input = raw(n, 96, f, 0.001, 7193, true);
    let mixed = execute(&input, plan(vec![]));
    let output = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply(&mixed)
    .unwrap();
    let data = output.as_processed().unwrap().data();
    // Independent direct-sum oracle: each bin-centered tone has amplitude N*F*A.
    // 5% complex amplitude error covers the known sigma=0.001, 20:1 peak ratio.
    for (amplitude, f1, f2) in [(1.0, 7usize, 13isize), (0.05, 29, -19)] {
        let row = (n / 2 + f1) % n;
        let col = (f as isize / 2 + f2) as usize;
        let expected = amplitude * (n * f) as f64;
        let real = data.get(&[row, col], &[0, 0]).unwrap();
        assert!(
            (real / expected - 1.0).abs() < 0.05,
            "amplitude={amplitude}, recovered={real}, expected={expected}"
        );
        let mut area = 0.0;
        for offset in -2isize..=2 {
            area += data
                .get(&[(row as isize + offset) as usize, col], &[0, 0])
                .unwrap();
        }
        assert!((area / expected - 1.0).abs() < 0.05);
        for offset in [-2isize, -1, 1, 2] {
            let flank = data
                .get(&[(row as isize + offset) as usize, col], &[0, 0])
                .unwrap();
            assert!(
                flank.abs() < 0.02 * expected,
                "unexpected line-shape flank {flank}"
            );
        }
    }
    let weak = 0.05 * (n * f) as f64;
    let mut false_peak = 0.0_f64;
    for row in 0..n {
        for col in 0..f {
            if (row.abs_diff(n / 2 + 7) <= 2 && col.abs_diff(f / 2 + 13) <= 2)
                || (row.abs_diff(n / 2 + 29) <= 2 && col.abs_diff(f / 2 - 19) <= 2)
            {
                continue;
            }
            false_peak = false_peak.max(data.get(&[row, col], &[0, 0]).unwrap().abs());
        }
    }
    assert!(
        false_peak < 0.02 * weak,
        "largest false peak={false_peak}; weak={weak}"
    );
}

#[test]
fn automatic_noise_tracks_shift_fold_delay_and_non_power_of_two_grid() {
    let input = raw(255, 96, 128, 0.01, 7193, false);
    for delay in [4.0, 4.25] {
        let out = execute(
            &input,
            plan(vec![Op::DigitalFilterCorrection {
                axis: 1,
                correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
                    source: DelaySource::Explicit(delay),
                    policy: TimeDomainResidualPolicy::CorrectFully,
                },
            }]),
        );
        // Unitary fractional shift leaves white complex noise white. V1 removes
        // six samples and folds two disjoint tail samples into the retained head:
        // total variance gain for unnormalised F2 FFT is 128-6+2 = 124.
        let expected = 0.01 * 124.0_f64.sqrt();
        assert!((report(&out).sigma / expected - 1.0).abs() < 0.10);
        assert_eq!(
            out.as_processed().unwrap().descriptor().logical_shape(),
            [255, 122]
        );
    }
}

#[test]
fn short_automatic_reconstruction_keeps_weak_peaks_and_replays_noise_policy() {
    let input = raw(128, 32, 256, 0.001, 7193, true);
    let mixed = execute(&input, plan(vec![]));
    assert_eq!(report(&mixed).source, NusNoiseSource::SplitHoldoutV1);
    assert_eq!(report(&mixed).effective_observations, 16);
    let output = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply(&mixed)
    .unwrap();
    for (amplitude, row, column) in [(1.0, 64 + 7, 128 + 13), (0.05, 64 + 29, 128 - 19)] {
        let actual = output
            .as_processed()
            .unwrap()
            .data()
            .get(&[row, column], &[0, 0])
            .unwrap();
        assert!(
            (actual / (128.0 * 256.0 * amplitude) - 1.0).abs() < 0.05,
            "peak={actual}"
        );
    }
    let mut bytes = Vec::new();
    nmr::snapshot::write_snapshot(&mixed, &mut bytes, Default::default()).unwrap();
    let restored = nmr::snapshot::read_snapshot(&mut bytes.as_slice(), Default::default())
        .unwrap()
        .restore(nmr::snapshot::AcceptRecordedHistory);
    assert_eq!(report(&restored), report(&mixed));
    let ProcessedOrigin::Library(origin) = restored.as_processed().unwrap().provenance().origin()
    else {
        panic!()
    };
    let replay = origin
        .replay(
            &[&input],
            ProcessingOptions::new(),
            &mut ExecutionContext::default(),
        )
        .unwrap();
    assert_eq!(replay.canonical_digests(), mixed.canonical_digests());
}
