use nmr::raw::{DirectSamples, RawAxisKind, RawDataset, RawFormat};

use crate::fixture_support::*;

fn assert_jeol(bytes: &[u8], expected: &[(f64, f64)]) -> RawDataset {
    let dataset = nmr::formats::jeol::read_parts(
        nmr::formats::jeol::Parts::new(bytes).allow_experimental_vendor_semantics(true),
    )
    .unwrap();
    assert_eq!(dataset.provenance().format(), Some(RawFormat::JeolDelta));
    assert_eq!(
        sample_bits(dataset.data().dense_samples().unwrap()),
        expected_bits(expected)
    );
    dataset
}

#[test]
fn jeol_binary_fixtures_freeze_experimental_layouts_and_evidence_gates() {
    let real = assert_jeol(
        include_bytes!("../../fixtures/jeol/axis-1.jdf"),
        &[(1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0)],
    );
    assert_eq!(
        real.descriptor().axes()[0].kind(),
        &RawAxisKind::Direct(DirectSamples::Real)
    );

    let complex = assert_jeol(
        include_bytes!("../../fixtures/jeol/axis-3.jdf"),
        &[(1.0, -10.0), (2.0, -20.0), (3.0, -30.0), (4.0, -40.0)],
    );
    assert_eq!(
        complex.descriptor().axes()[0].kind(),
        &RawAxisKind::Direct(DirectSamples::Complex)
    );

    let two_dimensional = nmr::formats::jeol::read_parts(
        nmr::formats::jeol::Parts::new(include_bytes!("../../fixtures/jeol/axes-3-1.jdf"))
            .allow_experimental_vendor_semantics(true),
    )
    .unwrap();
    assert_eq!(two_dimensional.descriptor().logical_shape(), vec![3, 6]);
    assert_eq!(two_dimensional.descriptor().component_lanes(), vec![1, 1]);
    assert_eq!(
        sample_bits(two_dimensional.data().read_trace(&[2]).unwrap().samples()),
        expected_bits(&[
            (20.0, -1020.0),
            (21.0, -1021.0),
            (22.0, -1022.0),
            (23.0, -1023.0),
            (24.0, -1024.0),
            (25.0, -1025.0),
        ])
    );

    let hypercomplex = nmr::formats::jeol::read_parts(
        nmr::formats::jeol::Parts::new(include_bytes!("../../fixtures/jeol/axes-3-3.jdf"))
            .allow_experimental_vendor_semantics(true),
    )
    .unwrap();
    assert_eq!(hypercomplex.descriptor().logical_shape(), vec![2, 3]);
    assert_eq!(hypercomplex.descriptor().component_lanes(), vec![2, 1]);
    assert_eq!(
        sample_bits(hypercomplex.data().read_trace(&[0]).unwrap().samples()),
        expected_bits(&[
            (0.0, -100.0),
            (1.0, -101.0),
            (2.0, -102.0),
            (-200.0, 300.0),
            (-201.0, 301.0),
            (-202.0, 302.0),
        ])
    );

    let real_complex = nmr::formats::jeol::read_parts(
        nmr::formats::jeol::Parts::new(include_bytes!("../../fixtures/jeol/axes-4-4.jdf"))
            .allow_experimental_vendor_semantics(true),
    )
    .unwrap();
    assert_eq!(real_complex.descriptor().logical_shape(), vec![3, 6]);
    assert_eq!(real_complex.descriptor().component_lanes(), vec![1, 1]);
    assert_eq!(
        sample_bits(real_complex.data().read_trace(&[0]).unwrap().samples()),
        expected_bits(&[
            (0.0, -1000.0),
            (1.0, -1001.0),
            (2.0, -1002.0),
            (3.0, -1003.0),
            (4.0, -1004.0),
            (5.0, -1005.0),
        ])
    );
}
