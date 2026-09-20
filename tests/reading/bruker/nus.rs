use nmr::Complex64;
use nmr::formats::bruker::{Parts, read_parts};
use nmr::processing::{ProcessingOperation, ProcessingPlan};
use nmr::raw::{
    AccessError, OpenOptions, RawFormat as Format, ReadErrorKind, ReadErrorReason, ReadLimits,
    Region,
};
use std::fs;

use super::support::*;

#[test]
fn bruker_nus_requires_explicit_experimental_acceptance() {
    let (acqus, acqu2s) = two_dimensional_parameters(2, 6, 4, 8, true);
    let bytes = encoded_ser(4, true);
    let parameters = [&*acqus, &*acqu2s];
    let parts = Parts::new(&bytes, &parameters).nuslist("3\n1\n");
    for error in [
        read_parts(parts).unwrap_err(),
        nmr::formats::bruker::read_parts_with_limits(parts, ReadLimits::new()).unwrap_err(),
    ] {
        assert!(
            matches!(error.reason(), ReadErrorReason::UnsupportedFeature { code, .. }
            if *code == nmr::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS)
        );
    }
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("acqus"), &acqus).unwrap();
    fs::write(directory.path().join("acqu2s"), &acqu2s).unwrap();
    fs::write(directory.path().join("ser"), bytes).unwrap();
    fs::write(directory.path().join("nuslist"), "3\n1\n").unwrap();
    for error in [
        nmr::raw::open(directory.path()).unwrap_err(),
        nmr::read(directory.path()).unwrap_err(),
    ] {
        assert!(
            matches!(error.reason(), ReadErrorReason::UnsupportedFeature { code, .. }
            if *code == nmr::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS)
        );
    }
    let dataset = nmr::ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(directory.path())
        .unwrap();
    assert!(!dataset.warnings().iter().any(|warning| matches!(
        warning,
        nmr::ReadWarning::ExperimentalVendorSemantics {
            format: nmr::Format::Raw(Format::BrukerRaw),
            ..
        }
    )));
}

#[test]
fn incomplete_bruker_nus_remains_sparse() {
    let (acqus, acqu2s) = two_dimensional_parameters(2, 6, 4, 8, true);
    let dataset =
        read_from_parts(&encoded_ser(4, true), &[&acqus, &acqu2s], Some("3\n1\n")).unwrap();

    assert_eq!(dataset.data().shape(), &[4, 2]);
    assert!(dataset.data().is_sparse());
    assert_eq!(dataset.sampling_schedule().unwrap().grid(), &[4]);
    assert_eq!(
        dataset
            .sampling_schedule()
            .unwrap()
            .coordinates()
            .iter()
            .map(|coordinate| coordinate.as_slice()[0])
            .collect::<Vec<_>>(),
        vec![3, 1]
    );
    assert_eq!(
        dataset.data().read_trace(&[3]).unwrap().samples(),
        vec![
            Complex64::new(1.0, 2.0),
            Complex64::new(3.0, 4.0),
            Complex64::new(11.0, 12.0),
            Complex64::new(13.0, 14.0),
        ]
    );
    let error = dataset.data().read_trace(&[0]).unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::UnsampledCoordinate);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::Access(AccessError::UnsampledCoordinate { coordinate })
            if coordinate.as_slice() == [0]
    ));
}

#[test]
fn complete_unordered_bruker_nus_scatters_to_dense_grid() {
    let (acqus, acqu2s) = two_dimensional_parameters(2, 5, 4, 4, true);
    let dataset =
        read_from_parts(&encoded_ser(4, true), &[&acqus, &acqu2s], Some("1\n0\n")).unwrap();

    assert!(!dataset.data().is_sparse());
    assert_eq!(dataset.data().read_trace(&[0]).unwrap()[0].re, 21.0);
    assert_eq!(dataset.data().read_trace(&[1]).unwrap()[0].re, 1.0);

    let transformed =
        ProcessingPlan::new(vec![ProcessingOperation::ComponentTransform { axis: 0 }])
            .unwrap()
            .apply_raw(&dataset)
            .unwrap();
    assert_eq!(transformed.data().get(&[0, 0], &[0, 0]).unwrap(), 21.0);
    assert_eq!(transformed.data().get(&[0, 0], &[1, 0]).unwrap(), 31.0);
    assert_eq!(transformed.data().get(&[1, 0], &[0, 0]).unwrap(), -1.0);
    assert_eq!(transformed.data().get(&[1, 0], &[1, 0]).unwrap(), -11.0);
}

#[test]
fn parts_keep_repeated_observations_when_schedule_count_equals_grid_size() {
    let (acqus, acqu2s) = two_dimensional_parameters(2, 6, 4, 4, true);
    let bytes = encoded_ser(4, true);
    let parts = read_from_parts(&bytes, &[&acqus, &acqu2s], Some("0\n0\n")).unwrap();
    assert!(parts.data().is_sparse());
    let traces = parts.data().sparse_traces().unwrap();
    assert_eq!(traces.len(), 2);
    assert_eq!(traces[0].ordinal().get(), 0);
    assert_eq!(traces[1].ordinal().get(), 1);
    assert_eq!(traces[0].coordinate().as_slice(), &[0]);
    assert_eq!(traces[1].coordinate().as_slice(), &[0]);
    assert_eq!(traces[0].samples()[0], Complex64::new(1.0, 2.0));
    assert_eq!(traces[1].samples()[0], Complex64::new(21.0, 22.0));
    assert_eq!(
        parts.read_trace(&[0]).unwrap_err().kind(),
        ReadErrorKind::AmbiguousObservation
    );
    assert_eq!(
        parts.read_trace(&[1]).unwrap_err().kind(),
        ReadErrorKind::UnsampledCoordinate
    );
    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("acqus"), acqus).unwrap();
    fs::write(temporary.path().join("acqu2s"), acqu2s).unwrap();
    fs::write(temporary.path().join("ser"), bytes).unwrap();
    fs::write(temporary.path().join("nuslist"), "0\n0\n").unwrap();
    let path = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .open(temporary.path())
        .unwrap()
        .into_dataset()
        .unwrap();
    assert_eq!(path.data(), parts.data());
    assert_eq!(path.sampling_schedule(), parts.sampling_schedule());
}

#[test]
fn repeated_nus_coordinates_preserve_each_acquisition() {
    let temporary = tempfile::tempdir().unwrap();
    let (acqus, acqu2s) = two_dimensional_parameters(2, 6, 4, 8, true);
    fs::write(temporary.path().join("acqus"), acqus).unwrap();
    fs::write(temporary.path().join("acqu2s"), acqu2s).unwrap();
    let mut ser = encoded_ser(4, true);
    ser.resize(8 * 16, 0);
    fs::write(temporary.path().join("ser"), ser).unwrap();
    fs::write(temporary.path().join("nuslist"), "0\n0\n2\n3\n").unwrap();

    let reader = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .open(temporary.path())
        .unwrap();
    assert!(
        reader
            .sampling_schedule()
            .unwrap()
            .has_repeated_coordinates()
    );
    assert_eq!(
        reader.read_trace(&[0]).unwrap_err().kind(),
        ReadErrorKind::AmbiguousObservation
    );
    assert_eq!(
        reader
            .read_observation(nmr::raw::ObservationOrdinal::new(0))
            .unwrap()
            .samples()[0]
            .re,
        1.0
    );
    assert_eq!(
        reader
            .read_observation(nmr::raw::ObservationOrdinal::new(1))
            .unwrap()
            .samples()[0]
            .re,
        21.0
    );
    let region = reader
        .read_region(&Region::new([0, 0], [1, 2]).unwrap(), usize::MAX)
        .unwrap();
    let region_traces = region.data().sparse_traces().unwrap();
    assert_eq!(region_traces.len(), 2);
    assert_eq!(region_traces[0].ordinal().get(), 0);
    assert_eq!(region_traces[1].ordinal().get(), 1);
    let dataset = reader.into_dataset().unwrap();
    let traces = dataset.data().sparse_traces().unwrap();
    assert_eq!(traces.len(), 2);
    assert_eq!(traces[0].coordinate().as_slice(), &[0]);
    assert_eq!(traces[1].coordinate().as_slice(), &[0]);
    assert_eq!(traces[0].samples()[0].re, 1.0);
    assert_eq!(traces[1].samples()[0].re, 21.0);
    assert_eq!(
        dataset.data().read_trace(&[0]).unwrap_err().kind(),
        ReadErrorKind::AmbiguousObservation
    );
    assert_eq!(
        dataset
            .data()
            .materialize_dense(Complex64::default(), usize::MAX)
            .unwrap_err()
            .kind(),
        ReadErrorKind::AmbiguousObservation
    );
}
