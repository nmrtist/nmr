use nmr::processing::{ProcessingOperation, ProcessingPlan};
use nmr::raw::RawFormat;
use nmr::{DatasetKind, Format};
use std::fs;

use super::support::*;

#[test]
fn aggregate_retains_selection_identity_and_warnings_across_processed_segments() {
    let temporary = tempfile::tempdir().unwrap();
    let procno = temporary
        .path()
        .join("subject")
        .join("7")
        .join("pdata")
        .join("3");
    fs::create_dir_all(&procno).unwrap();
    let text = processed_parameters(3).replace("##$AXNUC= <1H>\n", "");
    fs::write(procno.join("procs"), text).unwrap();
    write_i32(&procno.join("1r"), &[1, -2, 3]);
    let input = nmr::read(&procno).unwrap();
    assert!(!input.warnings().is_empty());
    assert!(matches!(
        input.descriptor(),
        nmr::dataset::DescriptorRef::Processed(_)
    ));
    assert!(matches!(
        input.provenance(),
        nmr::dataset::ProvenanceRef::Processed(_)
    ));
    let metadata = input.metadata().clone();
    let plan = ProcessingPlan::new(vec![ProcessingOperation::ReverseAxis { axis: 0 }]).unwrap();
    let prepared = plan
        .preflight(&input, nmr::processing::ProcessingOptions::default())
        .unwrap();
    assert_eq!(prepared.input_digest(), input.canonical_digests().dataset());
    let first = prepared.execute().unwrap();
    let second = plan
        .preflight(&first, nmr::processing::ProcessingOptions::default())
        .unwrap()
        .execute()
        .unwrap();
    drop(first);
    assert_eq!(second.metadata(), &metadata);
    assert_eq!(
        second.as_dense_processed().unwrap().samples(),
        &[2.0, -4.0, 6.0]
    );
    assert_eq!(second.sources(), input.sources());
    let history = second
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap();
    assert_eq!(history.segments().len(), 2);
    assert_eq!(history.inputs().len(), 1);
    assert_eq!(
        history.axis_lineage(),
        &[nmr::provenance::InputAxisRef::new(
            nmr::provenance::InputSlot::new(0),
            0
        )]
    );
    for supplied in [vec![], vec![&input, &input]] {
        assert_eq!(
            history
                .replay(&supplied, nmr::processing::ProcessingOptions::default())
                .unwrap_err(),
            nmr::processing::ProcessingError::InputCountMismatch {
                expected: 1,
                actual: supplied.len()
            }
        );
    }
    let replay = history
        .replay(&[&input], nmr::processing::ProcessingOptions::default())
        .unwrap();
    assert_eq!(replay.as_dense_processed(), second.as_dense_processed());
    assert_eq!(replay.metadata(), input.metadata());
}

#[test]
fn aggregate_raw_processing_preserves_the_original_selection_and_exact_fft() {
    let temporary = tempfile::tempdir().unwrap();
    write_complete_bruker_raw_1d(temporary.path());
    let input = nmr::read(temporary.path()).unwrap();
    assert!(matches!(
        input.descriptor(),
        nmr::dataset::DescriptorRef::Raw(_)
    ));
    let plan = ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
        axis: 0,
        transform: nmr::processing::FourierTransform::default(),
    }])
    .unwrap();
    let prepared = plan
        .preflight(&input, nmr::processing::ProcessingOptions::default())
        .unwrap();
    assert_eq!(prepared.input_digest(), input.canonical_digests().dataset());
    assert_eq!(prepared.descriptor().logical_shape(), &[2]);
    drop(plan);
    let output = prepared.execute().unwrap();
    assert_eq!(output.kind(), DatasetKind::Processed);
    assert_eq!(
        output.source_format(),
        Some(Format::Raw(RawFormat::BrukerRaw))
    );
    assert_eq!(output.metadata(), input.metadata());
    assert_eq!(
        output.as_dense_processed().unwrap().samples(),
        &[-2.0, -2.0, 4.0, 6.0]
    );
    assert_eq!(
        output.canonical_digests(),
        output.as_processed().unwrap().canonical_digests()
    );
    let history = output
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap();
    assert!(matches!(
        history.inputs(),
        [nmr::processing::HistoryInput::Raw { .. }]
    ));
    let replay = history
        .replay(&[&input], nmr::processing::ProcessingOptions::default())
        .unwrap();
    assert_eq!(replay.as_dense_processed(), output.as_dense_processed());
    assert_eq!(replay.metadata(), input.metadata());
    assert_eq!(
        history
            .replay(&[&output], nmr::processing::ProcessingOptions::default())
            .unwrap_err(),
        nmr::processing::ProcessingError::InputIdentityMismatch
    );
    let processed = output.into_processed_data().unwrap();
    let memory = nmr::Dataset::from_processed(processed);
    assert!(memory.source_format().is_none());
    assert!(memory.selected_path().is_none());
    assert!(memory.metadata().selection().is_none());
    assert_eq!(
        memory.canonical_digests(),
        memory.as_processed().unwrap().canonical_digests()
    );
}
