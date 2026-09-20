use nmr::axis::{AxisCoordinates, AxisDomain};
use nmr::snapshot::{self, AcceptRecordedHistory, SnapshotError, SnapshotLimits};
use nmr::{Complex64, Dataset};

use crate::datasets::*;

#[test]
fn raw_import_and_processed_snapshots_restore_without_source_files() {
    roundtrip(&raw());
    roundtrip(&scalar(23));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("spectrum.dx");
    std::fs::copy(
        crate::fixture_paths::fixture("jcamp_dx/scaled-dif.dx"),
        &path,
    )
    .unwrap();
    let imported = nmr::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let input = reverse().apply(&imported).unwrap();
    let output = roundtrip(&input);
    assert_eq!(output.selected_path(), input.selected_path());
    assert_eq!(
        reverse()
            .apply(&output)
            .unwrap()
            .as_dense_processed()
            .unwrap()
            .samples(),
        imported.as_dense_processed().unwrap().samples()
    );
}

#[test]
fn snapshots_reject_truncation_tampering_limits_and_unknown_versions() {
    let mut bytes = Vec::new();
    snapshot::write_snapshot(&scalar(12), &mut bytes, SnapshotLimits::default()).unwrap();
    for len in [0, 8, 19, bytes.len() - 1] {
        assert!(snapshot::read_snapshot(&mut &bytes[..len], SnapshotLimits::default()).is_err());
    }
    let mut tampered = bytes.clone();
    let i = tampered.len() - 41;
    tampered[i] ^= 1;
    assert!(snapshot::read_snapshot(&mut tampered.as_slice(), SnapshotLimits::default()).is_err());
    let mut future = bytes.clone();
    future[8..12].copy_from_slice(&99u32.to_le_bytes());
    assert!(matches!(
        snapshot::read_snapshot(&mut future.as_slice(), SnapshotLimits::default()),
        Err(SnapshotError::UnsupportedVersion(99))
    ));
    assert!(matches!(
        snapshot::read_snapshot(
            &mut bytes.as_slice(),
            SnapshotLimits {
                max_sample_bytes: 8,
                ..SnapshotLimits::default()
            }
        ),
        Err(SnapshotError::ResourceLimit)
    ));
}

#[test]
fn sparse_snapshot_keeps_schedule_order_duplicate_observations_and_signed_zero() {
    use nmr::raw::*;
    let coordinates: Vec<_> = [3, 1, 1]
        .into_iter()
        .map(|p| SamplingCoordinate::new(vec![p]))
        .collect();
    let traces = coordinates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            SparseTrace::new(
                ObservationOrdinal::new(i),
                c.clone(),
                vec![Complex64::new(i as f64, -0.0); 2],
            )
        })
        .collect();
    let axes = vec![
        RawAxis::new(
            RawAxisKind::Indirect(IndirectComponents::Scalar),
            AxisDomain::Unknown,
            None,
            4,
            AxisCoordinates::Unknown,
        )
        .unwrap(),
        RawAxis::new(
            RawAxisKind::Direct(DirectSamples::Complex),
            AxisDomain::Unknown,
            None,
            2,
            AxisCoordinates::Unknown,
        )
        .unwrap(),
    ];
    let input: Dataset = RawDatasetBuilder::new(axes, RawMetadata::default())
        .unwrap()
        .sparse(traces, SamplingSchedule::new(vec![4], coordinates).unwrap())
        .unwrap()
        .into();
    let output = roundtrip(&input);
    assert_eq!(input.as_raw(), output.as_raw());
    assert_eq!(
        output.as_raw().unwrap().data().sparse_traces().unwrap()[2].samples()[0]
            .im
            .to_bits(),
        (-0.0f64).to_bits()
    );
}

fn rehash(frame: &mut [u8]) {
    use sha2::{Digest, Sha256};
    let end = frame.len() - 40;
    let hash = Sha256::digest(&frame[..end]);
    frame[end..end + 32].copy_from_slice(&hash);
}

#[test]
fn checksum_is_not_a_substitute_for_scientific_and_version_validation() {
    let input = reverse().apply(&scalar(23)).unwrap();
    let mut frame = Vec::new();
    snapshot::write_snapshot(&input, &mut frame, SnapshotLimits::default()).unwrap();
    let old = b"reverse-axis.v1";
    let at = frame.windows(old.len()).position(|w| w == old).unwrap();
    let mut unknown = frame.clone();
    unknown[at + old.len() - 1] = b'9';
    rehash(&mut unknown);
    assert!(matches!(
        snapshot::read_snapshot(&mut unknown.as_slice(), SnapshotLimits::default()),
        Err(SnapshotError::UnsupportedHistoryVersion(_))
    ));
    let old = 22.0f64.to_le_bytes();
    let at = frame.windows(8).rposition(|w| w == old).unwrap();
    frame[at..at + 8].copy_from_slice(&23.5f64.to_le_bytes());
    rehash(&mut frame);
    assert!(snapshot::read_snapshot(&mut frame.as_slice(), SnapshotLimits::default()).is_err());
}

#[test]
fn accepted_history_keeps_its_recorded_environment_and_new_steps_record_current_build() {
    let input = reverse().apply(&scalar(11)).unwrap();
    let original = input
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap()
        .segments()[0]
        .environment()
        .build_identifier()
        .unwrap();
    let mut frame = Vec::new();
    snapshot::write_snapshot(&input, &mut frame, SnapshotLimits::default()).unwrap();
    let at = frame
        .windows(original.len())
        .position(|w| w == original.as_bytes())
        .unwrap();
    frame[at..at + original.len()].fill(b'a');
    rehash(&mut frame);
    let restored = snapshot::read_snapshot(&mut frame.as_slice(), SnapshotLimits::default())
        .unwrap()
        .restore(AcceptRecordedHistory);
    let output = reverse().apply(&restored).unwrap();
    let segments = output
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap()
        .segments();
    assert_eq!(
        segments[0].environment().build_identifier().unwrap(),
        "a".repeat(original.len())
    );
    assert_eq!(segments[1].environment().build_identifier(), Some(original));
    assert!(segments[0].accepted_archive());
    assert!(!segments[1].accepted_archive());
}
