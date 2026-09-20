use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::external::ExternalAlgorithmDeclaration;
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedDataset, ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{ProcessingOperation as Op, ProcessingPlan};
use nmr::snapshot::{self, AcceptRecordedHistory, SnapshotLimits};
use nmr::{Complex64, Dataset};

pub fn axis(n: usize, basis: ComponentBasis) -> ProcessedAxis {
    ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        n,
        AxisCoordinates::Uniform {
            start: -3.0,
            step: 0.125,
        },
        basis,
    )
    .unwrap()
}

pub fn scalar(n: usize) -> Dataset {
    ProcessedDataset::scalar_spectrum(
        axis(n, ComponentBasis::Scalar),
        (0..n).map(|x| x as f64).collect(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
    .into()
}

pub fn raw() -> Dataset {
    use nmr::raw::*;
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        8,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        },
    )
    .unwrap()
    .with_group_delay(GroupDelayState::NotApplicable)
    .unwrap();
    RawDatasetBuilder::new(vec![axis], RawMetadata::default())
        .unwrap()
        .dense(
            (0..8)
                .map(|i| Complex64::new(i as f64, -(i as f64)))
                .collect(),
        )
        .unwrap()
        .into()
}

pub fn reverse() -> ProcessingPlan {
    ProcessingPlan::new(vec![Op::ReverseAxis { axis: 0 }]).unwrap()
}

pub fn declaration() -> ExternalAlgorithmDeclaration {
    ExternalAlgorithmDeclaration::new("host.crop", "1", "Take a selected region")
        .unwrap()
        .with_parameters("application/json;v=1", br#"{"start":1}"#.to_vec())
        .unwrap()
}

pub fn roundtrip(input: &Dataset) -> Dataset {
    let mut frame = Vec::new();
    snapshot::write_snapshot(input, &mut frame, SnapshotLimits::default()).unwrap();
    let checked =
        snapshot::read_snapshot(&mut frame.as_slice(), SnapshotLimits::default()).unwrap();
    assert_eq!(checked.canonical_digests(), input.canonical_digests());
    let output = checked.restore(AcceptRecordedHistory);
    assert!(output.metadata().accepted_archive());
    assert_eq!(output.canonical_digests(), input.canonical_digests());
    output
}
