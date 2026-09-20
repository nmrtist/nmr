//! Phase and baseline estimation contracts.
use nmr::ExecutionContext;
use nmr::axis::AxisUnit;
use nmr::processing::{SpectrumOperation as S, *};
use nmr::snapshot::{self, AcceptRecordedHistory};

use crate::spectrum_support::*;

#[test]
fn real_baselines_preserve_imaginary_and_ppm_equivalence() {
    for unit in [AxisUnit::Hertz, AxisUnit::Ppm] {
        let input = spectrum(
            vec![4.0, 3.0, 2.0, 1.0, 0.0],
            &[[5.0, 1.0], [5.0, 2.0], [105.0, 3.0], [5.0, 4.0], [5.0, 5.0]],
            unit,
        );
        close(
            values(&apply(&input, 0, S::Baseline(RealBaseline::Offset))),
            &[0.0, 1.0, 0.0, 2.0, 100.0, 3.0, 0.0, 4.0, 0.0, 5.0],
            1e-12,
        );
        for method in [
            RealBaseline::Polynomial { order: 0 },
            RealBaseline::Asls {
                lambda: 50000.0,
                asymmetry: 0.001,
                iterations: 20,
            },
        ] {
            let output = apply(&input, 0, S::Baseline(method));
            for (a, b) in values(&output)
                .chunks_exact(2)
                .zip(values(&input).chunks_exact(2))
            {
                assert_eq!(a[1], b[1]);
            }
        }
    }
}

#[test]
fn segmented_baseline_retains_fitted_values_coefficients_and_replays() {
    let z = (0..21)
        .map(|i| {
            let x = 2.0 * i as f64 / 20.0 - 1.0;
            [
                5.0 + 2.0 * x + x * x + if i == 10 { 100.0 } else { 0.0 },
                i as f64 + 1.0,
            ]
        })
        .collect::<Vec<_>>();
    let input = spectrum((0..21).map(f64::from).collect(), &z, AxisUnit::Ppm);
    for method in [
        RealBaseline::Offset,
        RealBaseline::Polynomial { order: 2 },
        RealBaseline::Asls {
            lambda: 50000.0,
            asymmetry: 0.001,
            iterations: 20,
        },
    ] {
        let prepared = method.prepare(&input, ProcessingOptions::new()).unwrap();
        let r = prepared.resources();
        let options = ProcessingOptions::new()
            .max_output_bytes(r.output_bytes())
            .max_metadata_bytes(r.metadata_bytes())
            .max_working_bytes(r.working_bytes());
        let mut ledger = WorkLedger::new(prepared.estimated_work() + values(&input).len() as u128);
        let mut context = ExecutionContext::new(&mut ledger);
        let estimate = method
            .prepare(&input, options)
            .unwrap()
            .estimate_with_context(&mut context)
            .unwrap();
        if matches!(method, RealBaseline::Polynomial { .. }) {
            close(estimate.coefficients(), &[5.0, 2.0, 1.0], 1e-10);
        }
        let output = estimate
            .apply_with_context(&input, options, &mut context)
            .unwrap();
        for (i, pair) in values(&output).chunks_exact(2).enumerate() {
            assert_eq!(pair[1], z[i][1]);
            assert_eq!(pair[0], z[i][0] - estimate.values()[i]);
        }
        let history = output
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        assert!(matches!(
            history.records()[0].resolved(),
            Some(ResolvedOperation::EstimatedBaseline { .. })
        ));
        let replay = history.replay(&[&input], ProcessingOptions::new()).unwrap();
        close(values(&replay), values(&output), 0.0);
        let mut bytes = vec![];
        snapshot::write_snapshot(&output, &mut bytes, Default::default()).unwrap();
        let restored = snapshot::read_snapshot(&mut bytes.as_slice(), Default::default())
            .unwrap()
            .restore(AcceptRecordedHistory);
        apply(&restored, 0, S::Invert);
        let mut json = vec![];
        nmr::execution_report::write_json(output.as_processed().unwrap(), &[], &mut json, 1 << 20)
            .unwrap();
        assert!(
            String::from_utf8(json)
                .unwrap()
                .contains("estimated-real-baseline")
        );
    }
}
