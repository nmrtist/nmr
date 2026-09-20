//! Phase and baseline estimation contracts.
use nmr::axis::AxisUnit;
use nmr::processing::{ProcessingOperation as Op, SpectrumOperation as S, *};
use nmr::{CancellationToken, ExecutionContext};

use crate::spectrum_support::*;

#[test]
fn named_phase_methods_record_actual_solution_and_replay_without_reestimation() {
    for method in [
        PhaseMethod::AbsorptivePeak,
        PhaseMethod::Entropy,
        PhaseMethod::NegativeMinimization,
        PhaseMethod::PeakRegression,
        PhaseMethod::RobustConsensus,
    ] {
        let samples: Vec<_> = (0..512)
            .map(|i| {
                let z = [(123.0, 4.0, 1.0), (370.0, 5.0, 0.7)].into_iter().fold(
                    nmr::Complex64::default(),
                    |s, (center, width, height)| {
                        let u = (i as f64 - center) / width;
                        s + nmr::Complex64::new(1.0, -u) * (height / (1.0 + u * u))
                    },
                ) * nmr::Complex64::from_polar(1.0, 0.7 + 0.5 * i as f64 / 511.0);
                [z.re, z.im]
            })
            .collect();
        let input = spectrum((0..512).map(f64::from).collect(), &samples, AxisUnit::Ppm);
        let estimate = method
            .prepare(&input, 0, ProcessingOptions::new())
            .unwrap()
            .estimate()
            .unwrap();
        assert_eq!(estimate.method(), method);
        if method == PhaseMethod::AbsorptivePeak {
            assert_eq!(estimate.correction().p1_degrees(), 0.0);
        }
        let output = estimate.apply(&input, ProcessingOptions::new()).unwrap();
        let history = output
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        assert_eq!(
            history.records().last().unwrap().algorithm_version(),
            Some(method.algorithm_version())
        );
        let replay = history.replay(&[&input], ProcessingOptions::new()).unwrap();
        close(values(&replay), values(&output), 0.0);
        apply(&output, 0, S::Invert);
    }
}

#[test]
fn phase_methods_and_baselines_enforce_cancellation_limits_and_units() {
    for method in [
        PhaseMethod::AbsorptivePeak,
        PhaseMethod::Entropy,
        PhaseMethod::NegativeMinimization,
        PhaseMethod::PeakRegression,
        PhaseMethod::RobustConsensus,
    ] {
        let z = (0..256)
            .map(|i| {
                let mut z = nmr::Complex64::default();
                for (center, amp) in [(60.0, 1.0), (170.0, 0.7)] {
                    let u = (i as f64 - center) / 3.0;
                    z += nmr::Complex64::new(1.0, -u) * (amp / (1.0 + u * u));
                }
                z *= nmr::Complex64::from_polar(1.0, 0.4);
                [z.re, z.im]
            })
            .collect::<Vec<_>>();
        let mut corrections = vec![];
        for unit in [AxisUnit::Ppm, AxisUnit::Hertz] {
            let input = spectrum((0..256).map(f64::from).collect(), &z, unit);
            let prepared = method.prepare(&input, 0, ProcessingOptions::new()).unwrap();
            let r = prepared.resources();
            let options = ProcessingOptions::new()
                .max_output_bytes(r.output_bytes())
                .max_metadata_bytes(r.metadata_bytes())
                .max_working_bytes(r.working_bytes());
            let mut work = WorkLedger::new(prepared.estimated_work());
            let mut control = ExecutionContext::new(&mut work);
            corrections.push(
                method
                    .prepare(&input, 0, options)
                    .unwrap()
                    .estimate_with_context(&mut control)
                    .unwrap()
                    .correction(),
            );
            let token = CancellationToken::new();
            token.cancel();
            let mut work = WorkLedger::new(u128::MAX);
            let mut control = ExecutionContext::new(&mut work).with_cancellation(token);
            assert_eq!(
                method
                    .prepare(&input, 0, options)
                    .unwrap()
                    .estimate_with_context(&mut control)
                    .unwrap_err()
                    .code(),
                ProcessingErrorCode::Cancelled
            );
            let mut work = WorkLedger::new(0);
            let mut control = ExecutionContext::new(&mut work);
            assert_eq!(
                method
                    .prepare(&input, 0, options)
                    .unwrap()
                    .estimate_with_context(&mut control)
                    .unwrap_err()
                    .code(),
                ProcessingErrorCode::ResourceLimit
            );
        }
        assert_eq!(corrections[0], corrections[1]);
    }
    let short = spectrum(vec![0.0, 1.0], &[[0.0, 1.0]; 2], AxisUnit::Ppm);
    assert!(
        RealBaseline::Asls {
            lambda: 50000.0,
            asymmetry: 0.001,
            iterations: 20
        }
        .prepare(&short, ProcessingOptions::new())
        .is_err()
    );
    assert!(
        PhaseMethod::PeakRegression
            .prepare(&short, 0, ProcessingOptions::new())
            .unwrap()
            .estimate()
            .is_err()
    );
    for divisor in [0.0, f64::NAN, f64::INFINITY] {
        assert!(
            ProcessingPlan::new(vec![Op::Spectrum {
                axis: 0,
                operation: S::Normalize(Normalization::Constant(divisor))
            }])
            .unwrap()
            .apply(&short)
            .is_err()
        );
    }
}
