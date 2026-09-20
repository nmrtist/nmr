use nmr::processing::{
    ProcessingErrorCode, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan, WorkLedger,
};
use nmr::snapshot::{self, SnapshotError, SnapshotLimits};
use nmr::{CancellationToken, ExecutionContext};

use crate::datasets::*;

#[test]
fn cancellation_preserves_consumed_budget_and_does_not_return_partial_output() {
    let input = scalar(20000);
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let mut events = 0;
    let mut receiver = |event: nmr::execution::ProgressEvent| {
        if event.stage == nmr::execution::ExecutionStage::Processing && event.completed > 0 {
            events += 1;
            signal.cancel();
        }
    };
    let mut work = WorkLedger::new(100000);
    let error = reverse()
        .apply_with_context(
            &input,
            ProcessingOptions::default(),
            &mut ExecutionContext::new(&mut work)
                .with_cancellation(cancel)
                .with_progress(&mut receiver),
        )
        .unwrap_err();
    assert_eq!(error.code(), ProcessingErrorCode::Cancelled);
    assert_eq!(error.step_index(), Some(0));
    assert_eq!(events, 1);
    assert!(work.used() > 0 && work.used() < 20000);
    let mut tiny = WorkLedger::new(1);
    assert_eq!(
        reverse()
            .apply_with_context(
                &input,
                ProcessingOptions::default(),
                &mut ExecutionContext::new(&mut tiny)
            )
            .unwrap_err()
            .code(),
        ProcessingErrorCode::ResourceLimit
    );
    assert_eq!(tiny.used(), 0);
}

#[test]
fn shared_budget_and_observation_preserve_successful_numerics() {
    let input = scalar(17000);
    let expected = reverse().apply(&input).unwrap();
    let mut ledger = WorkLedger::new(100000);
    let mut events = Vec::new();
    let mut receive = |e| events.push(e);
    let mut context = ExecutionContext::new(&mut ledger).with_progress(&mut receive);
    let first = reverse()
        .apply_with_context(&input, ProcessingOptions::default(), &mut context)
        .unwrap();
    let used = context.ledger().used();
    assert_eq!(first.canonical_digests(), expected.canonical_digests());
    reverse()
        .apply_with_context(&first, ProcessingOptions::default(), &mut context)
        .unwrap();
    assert_eq!(context.ledger().used(), 2 * used);
    assert!(context.peak_payload_bytes() > 0);
    assert_eq!(context.io_bytes(), 0);
    assert!(events.iter().any(|e| e.completed == 17000));
    let independent = ledger.clone();
    assert_eq!(independent.used(), ledger.used());
}

#[test]
fn cancelled_read_snapshot_and_export_preserve_commit_contracts() {
    use nmr::execution::ExecutionStage;
    let input = scalar(20000);
    let token = CancellationToken::new();
    token.cancel();
    let mut context = ExecutionContext::default().with_cancellation(token);
    assert_eq!(
        nmr::read_with_context(
            crate::fixture_paths::fixture("jcamp_dx/scaled-dif.dx"),
            &mut context
        )
        .unwrap_err()
        .kind(),
        nmr::ReadErrorKind::Cancelled
    );
    let mut bytes = Vec::new();
    assert!(matches!(
        snapshot::write_snapshot_with_context(
            &input,
            &mut bytes,
            SnapshotLimits::default(),
            &mut context
        ),
        Err(SnapshotError::Execution(
            nmr::execution::ExecutionError::Cancelled
        ))
    ));
    assert!(bytes.is_empty());
    let token = CancellationToken::new();
    let signal = token.clone();
    let mut receive = move |e: nmr::execution::ProgressEvent| {
        if e.stage == ExecutionStage::Snapshot && e.completed > 1000 {
            signal.cancel();
        }
    };
    assert!(
        snapshot::write_snapshot_with_context(
            &input,
            &mut bytes,
            SnapshotLimits::default(),
            &mut ExecutionContext::default()
                .with_cancellation(token)
                .with_progress(&mut receive)
        )
        .is_err()
    );
    assert!(!bytes.is_empty());
    assert!(snapshot::read_snapshot(&mut bytes.as_slice(), SnapshotLimits::default()).is_err());
    let plot = nmr::plot::PlotData::from_processed(input.as_processed().unwrap()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("result.npz");
    std::fs::write(&target, b"previous").unwrap();
    let token = CancellationToken::new();
    let signal = token.clone();
    let mut receive = move |e: nmr::execution::ProgressEvent| {
        if e.stage == ExecutionStage::Export && e.completed > 1000 {
            signal.cancel();
        }
    };
    assert!(
        nmr::export::export_npz_with_context(
            &plot,
            &target,
            &mut ExecutionContext::default()
                .with_cancellation(token)
                .with_progress(&mut receive)
        )
        .is_err()
    );
    assert_eq!(std::fs::read(target).unwrap(), b"previous");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn cancellation_between_steps_and_during_baseline_is_typed() {
    let input = scalar(17000);
    let token = CancellationToken::new();
    let signal = token.clone();
    let mut progress = move |event: nmr::execution::ProgressEvent| {
        if event.stage == nmr::execution::ExecutionStage::Processing && event.step_index == Some(1)
        {
            signal.cancel();
        }
    };
    let plan = ProcessingPlan::new(vec![
        Op::ReverseAxis { axis: 0 },
        Op::ReverseAxis { axis: 0 },
    ])
    .unwrap();
    let mut work = WorkLedger::new(100000);
    let error = plan
        .apply_with_context(
            &input,
            ProcessingOptions::default(),
            &mut ExecutionContext::new(&mut work)
                .with_cancellation(token)
                .with_progress(&mut progress),
        )
        .unwrap_err();
    assert_eq!(error.code(), ProcessingErrorCode::Cancelled);
    assert_eq!(error.step_index(), Some(1));
    assert_eq!(work.used(), 17000);
    let coordinates: Vec<_> = (0..129).map(|i| i as f64).collect();
    let samples: Vec<_> = coordinates
        .iter()
        .map(|x| 2.0 + 0.003 * x + 20.0 * (-((x - 60.0) / 3.0).powi(2)).exp())
        .collect();
    let token = CancellationToken::new();
    let signal = token.clone();
    let mut progress = move |event: nmr::execution::ProgressEvent| {
        if event.completed > 0 {
            signal.cancel();
        }
    };
    let error = nmr::processing::PositivePeaksV1
        .subtract_with_context(
            &coordinates,
            &samples,
            &mut ExecutionContext::default()
                .with_cancellation(token)
                .with_progress(&mut progress),
        )
        .unwrap_err();
    assert_eq!(
        error,
        nmr::processing::AslsError::Execution(nmr::execution::ExecutionError::Cancelled)
    );
}
