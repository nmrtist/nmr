use nmr::processing::{
    PolarityState, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan, Projection,
    WorkLedger,
};
use nmr::snapshot::{self, AcceptRecordedHistory, SnapshotLimits};
use nmr::{AxisIndex, ExecutionContext};

use crate::support::*;
pub(crate) fn interactive_workflow() {
    let spectrum = spectrum(&mut ExecutionContext::default());
    let digest = spectrum.canonical_digests();
    let ptr = spectrum.as_dense_processed().unwrap().samples().as_ptr();
    let narrowed = spectrum.into_processed().unwrap();
    let trace = narrowed
        .processed()
        .complex_trace(AxisIndex::new(0), &[0], AxisIndex::new(0), &[0])
        .unwrap();
    assert_eq!(trace.len(), 64);
    let spectrum = narrowed.into_dataset();
    assert_eq!(spectrum.canonical_digests(), digest);
    assert_eq!(
        spectrum.as_dense_processed().unwrap().samples().as_ptr(),
        ptr
    );
    let magnitude = ProcessingPlan::new(vec![Op::Projection {
        projection: Projection::Magnitude,
        polarity: PolarityState::Ambiguous180,
    }])
    .unwrap()
    .apply(&spectrum)
    .unwrap();
    let plot = nmr::plot::PlotData::from_processed(magnitude.as_processed().unwrap()).unwrap();
    let mut export = Vec::new();
    nmr::export::write_npz(&plot, &mut export).unwrap();
    assert!(export.starts_with(b"PK"));
    println!("Interactive workflow: narrowing, trace, projection, plot and NPZ passed");
}
pub(crate) fn batch() {
    let mut ledger = WorkLedger::new(1_000_000);
    let mut context = ExecutionContext::new(&mut ledger);
    let first = spectrum(&mut context);
    let used = context.ledger().used();
    let second = spectrum(&mut context);
    assert_eq!(first.canonical_digests(), second.canonical_digests());
    assert_eq!(context.ledger().used(), 2 * used);
    println!(
        "Batch: identical results and shared budget passed; units={}",
        context.ledger().used()
    );
}
pub(crate) fn store() {
    use nmr::external::{ExternalAlgorithmDeclaration, ExternalAxisSource};
    use nmr::provenance::{InputAxisRef, InputSlot};
    let parent = spectrum(&mut ExecutionContext::default());
    let declaration =
        ExternalAlgorithmDeclaration::new("host.identity", "1", "Host-owned boundary result")
            .unwrap()
            .with_parameters("application/json", b"{}".to_vec())
            .unwrap();
    let boundary = parent
        .derive_external_processed(
            parent.as_processed().unwrap().descriptor().clone(),
            parent.as_dense_processed().unwrap().samples().to_vec(),
            vec![ExternalAxisSource::Parent(InputAxisRef::new(
                InputSlot::new(0),
                0,
            ))],
            declaration,
        )
        .unwrap();
    let plan = ProcessingPlan::new(vec![Op::ReverseAxis { axis: 0 }]).unwrap();
    let output = plan.apply(&boundary).unwrap();
    let mut frame = Vec::new();
    snapshot::write_snapshot(&output, &mut frame, SnapshotLimits::default()).unwrap();
    let restored = snapshot::read_snapshot(&mut frame.as_slice(), SnapshotLimits::default())
        .unwrap()
        .restore(AcceptRecordedHistory);
    assert_eq!(output.canonical_digests(), restored.canonical_digests());
    plan.apply(&restored).unwrap();
    let replay = restored
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap()
        .replay(&[&boundary], ProcessingOptions::default())
        .unwrap();
    assert_eq!(replay.canonical_digests(), output.canonical_digests());
    let mut report = Vec::new();
    nmr::execution_report::write_json(
        restored.as_processed().unwrap(),
        &[],
        &mut report,
        1_000_000,
    )
    .unwrap();
    assert!(String::from_utf8(report).unwrap().contains("host.identity"));
    println!(
        "Storage: external boundary, accepted archive, continuation, replay and report passed; frame_bytes={}",
        frame.len()
    );
}
