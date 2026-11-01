use nmr::ExecutionContext;
use nmr::processing::background::{BackgroundReport, Representation, Status, diagnose};
use std::time::Instant;
fn normal(seed: &mut u64) -> f64 {
    let mut uniform = || {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((*seed >> 11) as f64 + 0.5) / ((1u64 << 53) as f64)
    };
    (-2.0 * uniform().ln()).sqrt() * (std::f64::consts::TAU * uniform()).cos()
}
fn noise(n: usize, seed: u64, a: f64, b: f64) -> Vec<f64> {
    let mut seed = seed;
    let mut v = vec![0.0; n * n];
    for r in 0..n {
        for c in 0..n {
            let z = normal(&mut seed);
            v[r * n + c] = if c == 0 {
                z
            } else {
                b * v[r * n + c - 1] + (1.0 - b * b).sqrt() * z
            };
        }
    }
    for c in 0..n {
        for r in 1..n {
            v[r * n + c] = a * v[(r - 1) * n + c] + (1.0 - a * a).sqrt() * v[r * n + c];
        }
    }
    v
}
fn report(v: &[f64], n: usize) -> BackgroundReport {
    diagnose(
        v,
        &[n, n],
        None,
        Representation::PhaseSensitive,
        &mut ExecutionContext::default(),
    )
    .unwrap()
}
#[test]
fn seeded_marginal_calibration_and_equivariance() {
    for (a, b) in [(0.0, 0.0), (0.65, 0.3)] {
        let mut errors = vec![];
        let start = Instant::now();
        for seed in 1..=12 {
            let v = noise(256, seed, a, b);
            let r = report(&v, 256);
            assert_eq!(r.status, Status::Estimated, "{r:?}");
            let s = r.sigma.unwrap();
            errors.push(s - 1.0);
            assert!((s - 1.0).abs() < 0.08);
            assert!(r.location.unwrap().abs() < 0.06);
            let transformed: Vec<_> = v.iter().map(|x| 7.0 - 3.0 * x).collect();
            let t = report(&transformed, 256);
            assert!((t.sigma.unwrap() - 3.0 * s).abs() < 1e-10);
            assert!((t.location.unwrap() - (7.0 - 3.0 * r.location.unwrap())).abs() < 1e-10);
        }
        let mean = errors.iter().sum::<f64>() / 12.0;
        let sd = (errors.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 12.0).sqrt();
        eprintln!(
            "a={a}, b={b}: bias={mean:.5}, seed sd={sd:.5}, 24 reports {:?}",
            start.elapsed()
        );
        assert!(mean.abs() < 0.04 && sd < 0.04);
    }
}
#[test]
fn peaks_contamination_and_failure_states() {
    let n = 256;
    let base = noise(n, 42, 0.3, 0.1);
    let reference = report(&base, n).sigma.unwrap();
    for amplitude in [20.0, 1e6, -1e6] {
        let mut v = base.clone();
        for r in 0..n {
            for c in 0..n {
                let d = ((r as f64 - 90.0) / 2.0).powi(2) + ((c as f64 - 145.0) / 2.0).powi(2);
                v[r * n + c] += amplitude * (-d).exp();
            }
        }
        let s = report(&v, n).sigma.unwrap();
        assert!((s / reference - 1.0).abs() < 0.05);
    }
    for kind in 0..6 {
        let mut v = base.clone();
        for r in 0..n {
            for c in 0..n {
                let i = r * n + c;
                match kind {
                    0 => v[i] += 20.0 * r as f64 / n as f64,
                    1 => v[i] += 8.0 * (r as f64 / 50.0).sin(),
                    2 => v[i] += if r % 3 == 0 { 20.0 } else { -10.0 },
                    3 => v[i] *= if c < n / 2 { 1.0 } else { 5.0 },
                    4 => v[i] = 0.0,
                    _ => {
                        v[i] += 30.0
                            / (1.0
                                + ((r as f64 - 128.0) / 100.0).powi(2)
                                + ((c as f64 - 128.0) / 100.0).powi(2))
                    }
                }
            }
        }
        if kind == 4 {
            for (x, y) in v[..n * n / 8].iter_mut().zip(&base) {
                *x = *y;
            }
        }
        assert_eq!(report(&v, n).status, Status::Unidentifiable, "kind={kind}");
    }
    let mut v = base.clone();
    v[0] = f64::NAN;
    v[1] = f64::INFINITY;
    v[2] = 1e100;
    assert_eq!(report(&v, n).status, Status::Estimated);
    let mask: Vec<_> = (0..n * n).map(|i| i < n * n / 8).collect();
    let r = diagnose(
        &base,
        &[n, n],
        Some(&mask),
        Representation::PhaseSensitive,
        &mut ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(r.status, Status::Unidentifiable);
    assert_eq!(report(&vec![0.0; n * n], n).status, Status::Constant);
    assert_eq!(report(&[0.0; 16], 4).status, Status::InsufficientSamples);
    assert!(
        diagnose(
            &[],
            &[0, 4],
            None,
            Representation::Unknown,
            &mut ExecutionContext::default()
        )
        .is_err()
    );
    let r = diagnose(
        &base,
        &[n, n],
        None,
        Representation::Magnitude,
        &mut ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(r.status, Status::Unsupported);
    assert_eq!(r.sigma, None);
    let token = nmr::CancellationToken::new();
    token.cancel();
    assert!(
        diagnose(
            &base,
            &[n, n],
            None,
            Representation::PhaseSensitive,
            &mut ExecutionContext::default().with_cancellation(token)
        )
        .is_err()
    );
}

#[test]
fn final_units_after_library_processing_chain() {
    use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
    use nmr::processed::{
        ComponentBasis, ProcessedAxis, ProcessedDataset, ProcessedDescriptor, ProcessedOrigin,
        ProcessedProvenance,
    };
    use nmr::processing::{
        FourierExponentSign, FourierTransform, PhaseCorrection, ProcessingOperation as Op,
        ProcessingPlan, Window, ZeroFill,
    };
    let n = 128;
    let axes = (0..2)
        .map(|_| {
            ProcessedAxis::new(
                AxisRole::Signal,
                AxisDomain::Time,
                Some(AxisUnit::Second),
                n,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 0.001,
                },
                ComponentBasis::Cartesian,
            )
            .unwrap()
        })
        .collect();
    let descriptor = ProcessedDescriptor::new(axes).unwrap();
    let mut seed = 93;
    let samples: Vec<_> = (0..4 * n * n).map(|_| normal(&mut seed)).collect();
    // Known hypercomplex decaying FID signal added independently of the noise.
    let mut combined = samples.clone();
    for r in 0..n {
        for c in 0..n {
            for a in 0..2 {
                for b in 0..2 {
                    let p = std::f64::consts::TAU * 13.0 * r as f64 / n as f64;
                    let q = std::f64::consts::TAU * 21.0 * c as f64 / n as f64;
                    combined[((r * 2 + a) * n + c) * 2 + b] += 0.03
                        * (-((r + c) as f64) / 35.0).exp()
                        * if a == 0 { p.cos() } else { p.sin() }
                        * if b == 0 { q.cos() } else { q.sin() };
                }
            }
        }
    }
    let make = |v| {
        ProcessedDataset::from_dense_samples(
            descriptor.clone(),
            v,
            ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
        )
        .unwrap()
    };
    let noise = make(samples);
    let signal = make(combined);
    for factor in [1, 2] {
        let mut ops = vec![];
        for axis in 0..2 {
            ops.push(Op::Window {
                axis,
                window: Window::exponential(1.0).unwrap(),
            });
            ops.push(Op::ZeroFill {
                axis,
                zero_fill: ZeroFill::new(n * factor).unwrap(),
            });
            ops.push(Op::FourierTransform {
                axis,
                transform: FourierTransform::new(FourierExponentSign::Negative),
            });
            ops.push(Op::PhaseCorrection {
                axis,
                correction: PhaseCorrection::new(23.0, 0.0, 0.0).unwrap(),
            });
        }
        let plan = ProcessingPlan::new(ops).unwrap();
        let reference = plan.apply_processed(&noise).unwrap();
        let observed = plan.apply_processed(&signal).unwrap();
        assert!(!observed.has_magnitude_history());
        let magnitude = ProcessingPlan::new(vec![
            Op::Spectrum {
                axis: 0,
                operation: nmr::processing::SpectrumOperation::Magnitude,
            },
            Op::Spectrum {
                axis: 1,
                operation: nmr::processing::SpectrumOperation::Magnitude,
            },
        ])
        .unwrap()
        .apply_processed(&observed)
        .unwrap();
        assert!(magnitude.has_magnitude_history());
        let unsupported = nmr::processing::background::diagnose_dataset(
            &magnitude,
            Representation::PhaseSensitive,
            &mut ExecutionContext::default(),
        )
        .unwrap();
        assert_eq!(unsupported.status, Status::Unsupported);
        assert_eq!(unsupported.representation, Representation::Magnitude);
        // A subsequent execution retains the applied magnitude evidence.
        let inherited = ProcessingPlan::new(vec![Op::Spectrum {
            axis: 0,
            operation: nmr::processing::SpectrumOperation::Invert,
        }])
        .unwrap()
        .apply_processed(&magnitude)
        .unwrap();
        assert!(inherited.has_magnitude_history());
        let reference: Vec<_> = reference
            .data()
            .component_plane(&[0, 0])
            .unwrap()
            .copied()
            .collect();
        let observed: Vec<_> = observed
            .data()
            .component_plane(&[0, 0])
            .unwrap()
            .copied()
            .collect();
        let mean = reference.iter().sum::<f64>() / reference.len() as f64;
        let truth = (reference.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / reference.len() as f64)
            .sqrt();
        let result = report(&observed, n * factor);
        assert_eq!(result.status, Status::Estimated, "{result:?}");
        let error = result.sigma.unwrap() / truth - 1.0;
        eprintln!("zero-fill {factor}: final sigma truth={truth}, relative error={error}");
        assert!(error.abs() < 0.08);
    }
}

fn mad(v: &[f64]) -> f64 {
    let mut x = v.to_vec();
    x.sort_unstable_by(f64::total_cmp);
    let m = x[x.len() / 2];
    for x in &mut x {
        *x = (*x - m).abs();
    }
    x.sort_unstable_by(f64::total_cmp);
    x[x.len() / 2] / 0.6744897501960817
}
#[test]
fn candidate_comparison_and_contamination_sweep() {
    let n = 256;
    let base = noise(n, 17, 0.65, 0.3);
    let differences: Vec<_> = (1..n)
        .flat_map(|r| {
            let base = &base;
            (0..n).map(move |c| base[r * n + c] - base[(r - 1) * n + c])
        })
        .collect();
    let difference = mad(&differences) / 2.0_f64.sqrt();
    eprintln!(
        "candidate correlated: difference={difference}, global={}, blocked={:?}",
        mad(&base),
        report(&base, n).sigma
    );
    assert!((difference - (1.0_f64 - 0.65).sqrt()).abs() < 0.04);
    for width in [2.0, 8.0, 32.0, 100.0] {
        for lorentzian in [false, true] {
            let mut v = base.clone();
            for r in 0..n {
                for c in 0..n {
                    let d =
                        ((r as f64 - 128.0) / width).powi(2) + ((c as f64 - 128.0) / width).powi(2);
                    v[r * n + c] += 100.0
                        * if lorentzian {
                            1.0 / (1.0 + d)
                        } else {
                            (-d).exp()
                        };
                }
            }
            let result = report(&v, n);
            eprintln!(
                "width={width}, Lorentzian={lorentzian}: {:?}, sigma={:?}, coverage={}",
                result.status, result.sigma, result.coverage
            );
            if width == 2.0 && !lorentzian {
                assert!((result.sigma.unwrap() - 1.0).abs() < 0.08);
            }
            if width == 100.0 {
                assert_eq!(result.status, Status::Unidentifiable);
            }
        }
    }
    let mut heavy = base.clone();
    let mut seed = 201;
    for (i, x) in heavy.iter_mut().enumerate() {
        if i % 5 == 0 {
            *x += 10.0 * normal(&mut seed);
        }
    }
    assert_eq!(report(&heavy, n).status, Status::Unidentifiable);
}

#[test]
fn f32_adapter_and_sampled_tail_are_equivariant() {
    let v = noise(256, 91, 0.3, 0.1);
    let floats: Vec<f32> = v.iter().map(|x| *x as f32).collect();
    let promoted: Vec<f64> = floats.iter().map(|x| f64::from(*x)).collect();
    let a = nmr::processing::background::diagnose_f32(
        &floats,
        &[256, 256],
        None,
        Representation::PhaseSensitive,
        &mut ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(a, report(&promoted, 256));
    let tail = a.background_tail.unwrap();
    assert!((2.3..2.9).contains(&tail), "{a:?}");
    let shifted: Vec<_> = promoted.iter().map(|x| 7.0 - 3.0 * x).collect();
    let b = report(&shifted, 256);
    assert!((b.background_tail.unwrap() - 3.0 * tail).abs() < 1e-10);
    let [lo, hi] = a.location_span.unwrap();
    let [blo, bhi] = b.location_span.unwrap();
    assert!((blo - (7.0 - 3.0 * hi)).abs() < 1e-10);
    assert!((bhi - (7.0 - 3.0 * lo)).abs() < 1e-10);
}

#[test]
fn stratified_sampling_observes_periodic_sparse_contamination() {
    let n = 1024;
    let base = noise(n, 38, 0.0, 0.0);
    let mut tails = Vec::new();
    for phase in [0, 1, 2, 3] {
        let mut v = base.clone();
        for r in 0..n {
            for c in 0..n {
                if (c + phase) % 16 == 0 {
                    v[r * n + c] += 12.0;
                }
            }
        }
        let r = report(&v, n);
        assert_eq!(r.status, Status::Estimated, "{r:?}");
        assert!((r.sigma.unwrap() - 1.0).abs() < 0.2, "{r:?}");
        let tail = r.background_tail.unwrap();
        assert!(tail > 11.0, "{r:?}");
        tails.push(tail);
    }
    let min = tails.iter().copied().fold(f64::INFINITY, f64::min);
    let max = tails.iter().copied().fold(0.0, f64::max);
    eprintln!("periodic contamination phase tails: {tails:?}");
    assert!(max / min < 1.1);
}
