use nmr::raw::{
    OpenOptions, RawFormat as Format, ReadErrorKind, ReadErrorReason, ReadLimits, Region,
};
use std::fs;

use super::support::*;

#[test]
fn lazy_reader_enforces_per_trace_working_limit() {
    let temp = tempfile::tempdir().unwrap();
    let (acqus, acqu2s) = two_dimensional_parameters(0, 5, 4, 4, true);
    let acqus = acqus.replace("##$TD= 4\n", "##$TD= 4096\n");
    fs::write(temp.path().join("acqus"), acqus).unwrap();
    fs::write(temp.path().join("acqu2s"), acqu2s).unwrap();
    fs::write(temp.path().join("ser"), vec![0; 4 * 4096 * 4]).unwrap();
    let error = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .limits(ReadLimits::new().max_working_bytes(81_919))
        .open(temp.path())
        .unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::LimitExceeded);
    let required = match error.reason() {
        ReadErrorReason::LimitExceeded {
            resource: nmr::raw::ReadResource::TraceBytes,
            limit: 81_919,
            required,
            ..
        } => *required,
        other => panic!("expected trace working limit, found {other:?}"),
    };
    assert!(required > 81_920); // Retained parameters and owned input text coexist.
    let reader = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .limits(ReadLimits::new().max_working_bytes(required))
        .open(temp.path())
        .unwrap();
    assert_eq!(reader.read_trace(&[0]).unwrap().samples().len(), 4096);
}

#[test]
fn path_reader_detects_ser_and_records_all_sources() {
    let temp = tempfile::tempdir().unwrap();
    let (acqus, acqu2s) = two_dimensional_parameters(2, 6, 4, 8, true);
    fs::write(temp.path().join("acqus"), acqus).unwrap();
    fs::write(temp.path().join("acqu2s"), acqu2s).unwrap();
    fs::write(temp.path().join("ser"), encoded_ser(4, true)).unwrap();
    fs::write(temp.path().join("nuslist"), "3\n1\n").unwrap();

    assert_eq!(
        nmr::raw::detect(temp.path().join("ser")).unwrap(),
        Format::BrukerRaw
    );
    assert_eq!(
        nmr::raw::detect(temp.path().join("acqu2s")).unwrap(),
        Format::BrukerRaw
    );
    let reader = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .open(temp.path())
        .unwrap();
    assert_eq!(reader.read_trace(&[3]).unwrap().samples()[0].re, 1.0);
    let region = reader
        .read_region(&Region::new([1, 0], [3, 1]).unwrap(), usize::MAX)
        .unwrap();
    assert_eq!(region.data().shape(), &[3, 1]);
    assert!(region.data().is_sparse());
    let dataset = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .open(temp.path())
        .unwrap()
        .into_dataset()
        .unwrap();
    assert!(dataset.data().is_sparse());
    assert_eq!(
        dataset
            .provenance()
            .sources()
            .iter()
            .map(|source| source.role())
            .collect::<Vec<_>>(),
        vec!["ser", "acqus", "acqu2s", "sampling_schedule"]
    );
}

#[test]
fn path_reader_errors_retain_the_real_ser_path() {
    let temp = tempfile::tempdir().unwrap();
    let (acqus, acqu2s) = two_dimensional_parameters(0, 5, 2, 2, true);
    fs::write(temp.path().join("acqus"), acqus).unwrap();
    fs::write(temp.path().join("acqu2s"), acqu2s).unwrap();
    let mut ser = encoded_ser(2, true);
    ser.push(0);
    fs::write(temp.path().join("ser"), ser).unwrap();

    let error = nmr::raw::read(temp.path()).unwrap_err();
    assert!(matches!(
        error.reason(),
        ReadErrorReason::Corrupt { input, .. }
            if input.path() == Some(temp.path().join("ser").as_path())
    ));
}
