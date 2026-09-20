use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::raw::{
    DirectSamples, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata, SourceFile, SourceKind,
};

use super::support::*;

#[test]
fn canonical_identity_changes_for_descriptor_or_sample_bits() {
    let positive_zero = RawDatasetBuilder::new(
        vec![direct(1, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .dense(vec![Complex64::new(0.0, 0.0)])
    .unwrap();
    let negative_zero = RawDatasetBuilder::new(
        vec![direct(1, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .dense(vec![Complex64::new(-0.0, 0.0)])
    .unwrap();
    assert_ne!(
        positive_zero.canonical_digests().samples(),
        negative_zero.canonical_digests().samples()
    );
    assert_ne!(
        positive_zero.canonical_digests().dataset(),
        negative_zero.canonical_digests().dataset()
    );

    let shifted = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        1,
        AxisCoordinates::Uniform {
            start: 1.0,
            step: 0.5,
        },
    )
    .unwrap();
    let shifted = RawDatasetBuilder::new(vec![shifted], RawMetadata::default())
        .unwrap()
        .dense(vec![Complex64::new(0.0, 0.0)])
        .unwrap();
    assert_ne!(
        positive_zero.canonical_digests().descriptor(),
        shifted.canonical_digests().descriptor()
    );

    let cropped = RawDatasetBuilder::new(
        vec![direct(1, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .absolute_grid_origin(vec![3])
    .unwrap()
    .dense(vec![Complex64::new(0.0, 0.0)])
    .unwrap();
    assert_ne!(
        positive_zero.canonical_digests().descriptor(),
        cropped.canonical_digests().descriptor()
    );
}

#[test]
fn snapshots_keep_only_canonical_evidence_and_stable_source_ids() {
    let sources = vec![
        SourceFile::new(SourceKind::Data, "fid", "synthetic/fid").unwrap(),
        SourceFile::new(SourceKind::Parameters, "procpar", "synthetic/procpar").unwrap(),
    ];
    let dataset = RawDatasetBuilder::new(
        vec![direct(1, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .with_sources(sources)
    .dense(vec![Complex64::default()])
    .unwrap();
    let snapshot = dataset.snapshot();
    assert_eq!(snapshot.sources().len(), 2);
    assert_eq!(snapshot.sources()[0].id().unwrap().ordinal(), 0);
    assert_eq!(snapshot.sources()[1].id().unwrap().ordinal(), 1);
    assert_eq!(snapshot.canonical_digests(), dataset.canonical_digests());
    assert_eq!(
        snapshot.sample_normalization().algorithm_version(),
        "source-sample-normalization.v1"
    );
    assert_eq!(
        snapshot.sample_normalization().source_block_scale_factors(),
        &[1.0]
    );
    assert_eq!(
        snapshot
            .sample_normalization()
            .stored_imaginary_multiplier(),
        1
    );
}
