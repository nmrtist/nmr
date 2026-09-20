use super::encoding::Encoder;
use super::*;
use crate::Complex64;
use crate::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use crate::raw::{DirectSamples, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata};

fn hex(value: CanonicalDigest) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn canonical_raw_v1_descriptor_and_samples_have_independent_golden_bytes() {
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.5,
        },
    )
    .unwrap();
    let dataset = RawDatasetBuilder::new(vec![axis], RawMetadata::default())
        .unwrap()
        .dense(vec![Complex64::new(1.0, -2.0), Complex64::new(-0.0, 3.5)])
        .unwrap();
    let digests = dataset.canonical_digests();
    assert_eq!(
        hex(digests.descriptor()),
        "0c77c6fa36cff6f73f3a9d5848ac74fd30b23a8647700b914e132a228f52a4d0"
    );
    assert_eq!(
        hex(digests.samples()),
        "f9ecf9d37eacacecb7977c26ebe6f4874b5bf590aad1abecb90d5d01f60ce4b9"
    );
    assert_eq!(
        hex(digests.dataset()),
        "0bf0105d0c044f1b086e223466e07f47a86c6b9204d0d6c1edb6cf01badefcd7"
    );
}

#[test]
fn raw_display_labels_are_excluded_but_acquisition_facts_remain_bound() {
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        1,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.5,
        },
    )
    .unwrap();
    let make = |axis, title: Option<&str>, solvent: Option<&str>, value| {
        RawDatasetBuilder::new(
            vec![axis],
            RawMetadata::new(
                title.map(str::to_owned),
                solvent.map(str::to_owned),
                None,
                None,
                None,
            )
            .unwrap(),
        )
        .unwrap()
        .dense(vec![Complex64::new(value, 0.0)])
        .unwrap()
        .canonical_digests()
    };
    let original = make(axis.clone(), Some("original"), Some("D2O"), 1.0);
    assert_eq!(
        original,
        make(
            axis.clone().with_label(Some("renamed axis".into())),
            Some("original"),
            Some("D2O"),
            1.0
        )
    );
    assert_eq!(
        original,
        make(axis.clone(), Some("renamed title"), Some("D2O"), 1.0)
    );
    assert_ne!(
        original.descriptor(),
        make(axis.clone(), Some("original"), Some("CDCl3"), 1.0).descriptor()
    );
    assert_ne!(
        original.descriptor(),
        make(
            axis.clone().with_nucleus(Some("1H".into())).unwrap(),
            Some("original"),
            Some("D2O"),
            1.0
        )
        .descriptor()
    );
    assert_ne!(
        original.samples(),
        make(axis, Some("original"), Some("D2O"), 2.0).samples()
    );
}

#[test]
fn encoder_preserves_nan_payload_bits() {
    let mut left = Encoder::new(b"nan-payload-test\0");
    left.f64(f64::from_bits(0x7ff8_0000_0000_0001));
    let mut right = Encoder::new(b"nan-payload-test\0");
    right.f64(f64::from_bits(0x7ff8_0000_0000_0002));
    assert_ne!(left.finish(), right.finish());
}
