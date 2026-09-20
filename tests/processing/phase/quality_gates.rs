//! Independent absorptive Lorentzian truth; public estimation, control and replay.
use super::reference_signals as phase_readiness;

use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::execution::{ExecutionStage, ProgressEvent, ProgressTotal};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedDataset, ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    OperationTarget, PhaseMethod, ProcessingError, ProcessingErrorCode, ProcessingOptions,
    ProcessingPhase, ProcessingRequest, ResolvedOperation, WorkLedger,
};
use nmr::snapshot::{self, AcceptRecordedHistory};
use nmr::{CancellationToken, Complex64, Dataset, ExecutionContext};
use phase_readiness::groundtruth::{clean, many, residual, scramble};

fn noisy_input(p1: f64, scale: f64) -> (Dataset, Vec<Complex64>) {
    let n = 1024;
    let truth = clean(n, &many());
    let observed = scramble(&truth, 2.0, p1.to_radians(), 0.01)
        .into_iter()
        .map(|z| z * scale)
        .collect();
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Ppm),
        n,
        AxisCoordinates::Uniform {
            start: 10.0,
            step: -0.01,
        },
        ComponentBasis::Cartesian,
    )
    .unwrap();
    let input = ProcessedDataset::from_complex_trace(
        axis,
        observed,
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
    .into();
    (input, truth.into_iter().map(|z| z * scale).collect())
}

#[test]
fn eight_retained_phase_quality_gates() {
    assert_eq!(phase_readiness::run(), std::process::ExitCode::SUCCESS);
}

#[test]
fn entropy_large_ramps_preserve_quality_across_sign_and_scale() {
    for ramp in [-650.0, -500.0, 500.0, 650.0] {
        let mut nominal = None::<f64>;
        for scale in [1.0, 1e-5, 1e5] {
            let (input, truth) = noisy_input(ramp, scale);
            let estimate = PhaseMethod::Entropy
                .prepare(&input, 0, ProcessingOptions::new())
                .unwrap()
                .estimate()
                .unwrap();
            let output = estimate.apply(&input, ProcessingOptions::new()).unwrap();
            let values: Vec<_> = output
                .as_dense_processed()
                .unwrap()
                .samples()
                .chunks_exact(2)
                .map(|z| Complex64::new(z[0], z[1]))
                .collect();
            let error = residual(&values, &truth);
            println!("Entropy ramp={ramp}, scale={scale}: real NRMS={error:.12}");
            assert!(error < 0.35, "ramp={ramp}, scale={scale}: {error}");
            if let Some(reference) = nominal {
                assert!((error - reference).abs() < 1e-10);
            } else {
                nominal = Some(error);
            }
        }
    }
}

#[test]
fn wide_phase_search_accounts_for_work_records_diagnostics_and_replays() {
    for method in [
        PhaseMethod::Entropy,
        PhaseMethod::NegativeMinimization,
        PhaseMethod::RobustConsensus,
    ] {
        let (input, _) = noisy_input(500.0, 1.0);
        let identity = input.canonical_digests();
        let options = ProcessingOptions::new();
        let prepared = method.prepare(&input, 0, options).unwrap();
        let bound = prepared.estimated_work();
        let resources = prepared.resources();
        let options = options
            .max_output_bytes(resources.output_bytes())
            .max_metadata_bytes(resources.metadata_bytes())
            .max_working_bytes(resources.working_bytes());

        let mut short = WorkLedger::new(bound - 1);
        let mut context = ExecutionContext::new(&mut short);
        assert_eq!(
            method
                .prepare(&input, 0, options)
                .unwrap()
                .estimate_with_context(&mut context)
                .unwrap_err()
                .code(),
            ProcessingErrorCode::ResourceLimit
        );
        assert_eq!(context.ledger().used(), 0);

        let mut events = Vec::new();
        let mut callback = |event| events.push(event);
        // One advertised budget covers estimation and its application together.
        let mut work = WorkLedger::new(bound);
        let mut context = ExecutionContext::new(&mut work).with_progress(&mut callback);
        let estimate = method
            .prepare(&input, 0, options)
            .unwrap()
            .estimate_with_context(&mut context)
            .unwrap();
        let analysis_work = context.ledger().used();
        let n = input.as_dense_processed().unwrap().samples().len() / 2;
        assert_eq!(
            analysis_work,
            n as u128 * (8 + 4 * estimate.evaluations() as u128)
        );
        assert!(estimate.evaluations() <= 5708);
        let output = estimate
            .apply_with_context(&input, options, &mut context)
            .unwrap();
        assert!(context.ledger().used() <= bound);
        assert!(context.peak_payload_bytes() <= resources.working_bytes());
        assert_eq!(input.canonical_digests(), identity);
        let auto_events: Vec<_> = events
            .iter()
            .filter(|e| e.stage == ExecutionStage::AutoPhase)
            .collect();
        assert_eq!(auto_events.last().unwrap().completed, analysis_work);
        assert!(
            auto_events
                .iter()
                .all(|e| e.total == Some(ProgressTotal::UpperBound(bound)))
        );

        let history = output
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        assert_eq!(history.records().len(), 1);
        let record = &history.records()[0];
        assert_eq!(record.algorithm_version(), Some(method.algorithm_version()));
        assert!(method.algorithm_version().ends_with(".v1"));
        assert_eq!(
            record.requested(),
            &ProcessingRequest::PhaseMethod { axis: 0, method }
        );
        assert_eq!(
            record.resolved(),
            Some(&ResolvedOperation::AutomaticPhase {
                correction: estimate.correction(),
                objective: estimate.objective(),
                evaluations: estimate.evaluations(),
            })
        );

        let mut bytes = Vec::new();
        snapshot::write_snapshot(&output, &mut bytes, Default::default()).unwrap();
        let restored = snapshot::decode_snapshot(&bytes, Default::default())
            .unwrap()
            .restore(AcceptRecordedHistory);
        let history = restored
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        // Replay reserves its own history payload and needs only linear work;
        // this budget cannot admit another full-resolution phase search.
        let mut replay_work = WorkLedger::new(n as u128 * 100);
        let mut replay_context = ExecutionContext::new(&mut replay_work);
        let replay = history
            .replay_with_context(&[&input], ProcessingOptions::new(), &mut replay_context)
            .unwrap();
        assert_eq!(replay.canonical_digests(), output.canonical_digests());
        let mut report = Vec::new();
        nmr::execution_report::write_json(
            output.as_processed().unwrap(),
            &[],
            &mut report,
            1 << 20,
        )
        .unwrap();
        assert!(
            String::from_utf8(report)
                .unwrap()
                .contains(method.algorithm_version())
        );
        println!(
            "{method:?}: evaluations={}, analysis_work={analysis_work}, preflight={bound}",
            estimate.evaluations()
        );
    }
}

#[test]
fn wide_phase_search_can_cancel_after_evaluating_candidates() {
    for method in [
        PhaseMethod::Entropy,
        PhaseMethod::NegativeMinimization,
        PhaseMethod::RobustConsensus,
    ] {
        let (input, _) = noisy_input(500.0, 1.0);
        let identity = input.canonical_digests();
        let prepared = method.prepare(&input, 0, ProcessingOptions::new()).unwrap();
        let bound = prepared.estimated_work();
        let token = CancellationToken::new();
        let mut last_completed = 0;
        let mut callback = |event: ProgressEvent| {
            if event.stage == ExecutionStage::AutoPhase && event.completed >= 1024 * (8 + 4 * 10) {
                last_completed = event.completed;
                token.cancel();
            }
        };
        let mut work = WorkLedger::new(bound);
        let mut context = ExecutionContext::new(&mut work)
            .with_cancellation(token.clone())
            .with_progress(&mut callback);
        let error = prepared.estimate_with_context(&mut context).unwrap_err();
        assert_eq!(error.code(), ProcessingErrorCode::Cancelled);
        assert!(matches!(
            error,
            ProcessingError::Step {
                step_index: Some(0),
                target: Some(OperationTarget::Axis(0)),
                phase: ProcessingPhase::Execution,
                ..
            }
        ));
        assert!(context.ledger().used() < bound);
        assert!(last_completed > 0);
        assert_eq!(input.canonical_digests(), identity);
    }
}
