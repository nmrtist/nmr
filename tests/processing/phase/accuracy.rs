//! Independent analytic release gates. Expected signals use closed-form
//! physical line shapes or tones, never the processing kernels under test.

use nmr::Complex64;
use nmr::processing::{NormalizedAcmeV1, PhaseOptimizationError, PolarityState, WorkLedger};

fn lorentzian(points: usize, width: f64, amplitude: f64) -> Vec<Complex64> {
    (0..points)
        .map(|index| {
            let u = (index as f64 - 103.0 * points as f64 / 256.0) / width;
            Complex64::new(1.0, -u) * (amplitude / (1.0 + u * u))
        })
        .collect()
}

#[test]
fn acme_preserves_physical_absorption_across_amplitudes_and_sampling_densities() {
    for points in [128, 256, 1024] {
        for amplitude in [1e-150, 1.0, 1e150] {
            let trace = lorentzian(points, points as f64 / 64.0, amplitude);
            let mut work = WorkLedger::new(u128::MAX);
            let solution = NormalizedAcmeV1
                .optimize(&trace, PolarityState::UserAssertedPositive, &mut work)
                .unwrap();
            assert_eq!(solution.correction().p0_degrees(), 0.0);
            assert_eq!(solution.correction().p1_degrees(), 0.0);
            assert_eq!(solution.evaluations(), 0);
            assert_eq!(work.used(), 4 * points as u128);
        }
    }
}

#[test]
fn acme_rejects_known_bad_phase_and_unvalidated_line_shapes() {
    let canonical = lorentzian(256, 4.0, 1.0);
    for (p0, p1) in [(-37.0, 60.0), (-37.0, 240.0)] {
        let trace: Vec<_> = canonical
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    * Complex64::from_polar(
                        1.0,
                        (p0 + p1 * (index as f64 / 256.0 - 1.0)).to_radians(),
                    )
            })
            .collect();
        assert_eq!(
            NormalizedAcmeV1.optimize(
                &trace,
                PolarityState::UserAssertedPositive,
                &mut WorkLedger::new(u128::MAX)
            ),
            Err(PhaseOptimizationError::QualityUnverified)
        );
    }
    let noisy: Vec<_> = canonical
        .iter()
        .enumerate()
        .map(|(index, value)| value + Complex64::new(0.001 * (index as f64).sin(), 0.0))
        .collect();
    let negative: Vec<_> = canonical.iter().map(|value| -value).collect();
    for (trace, polarity) in [
        (noisy, PolarityState::UserAssertedPositive),
        (
            lorentzian(256, 40.0, 1.0),
            PolarityState::UserAssertedPositive,
        ),
        (negative, PolarityState::Ambiguous180),
    ] {
        assert_eq!(
            NormalizedAcmeV1.optimize(&trace, polarity, &mut WorkLedger::new(u128::MAX)),
            Err(PhaseOptimizationError::QualityUnverified)
        );
    }
}
