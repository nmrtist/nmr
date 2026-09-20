use super::support::*;
use crate::raw_support::*;
use nmr::raw::{OpenOptions, RawFormat as Format, ReadErrorKind, ReadLimits};
use std::fs;

#[test]
fn input_limit_is_checked_before_header_decode() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("truncated.jdf");
    fs::write(&path, b"JEOL.NMR").unwrap();
    let error = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .limits(ReadLimits::new().max_source_bytes(7))
        .open(&path)
        .unwrap_err();
    assert_vendor_error(&error, Format::JeolDelta, ReadErrorKind::LimitExceeded);
}

#[test]
fn detection_and_jeol_selection_report_ambiguity() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("first.jdf"), jeol_fixture_f64()).unwrap();
    fs::write(directory.path().join("second.JDF"), jeol_fixture_f64()).unwrap();
    let error = nmr::raw::read(directory.path()).unwrap_err();
    assert_vendor_error(&error, Format::JeolDelta, ReadErrorKind::Ambiguous);
}
