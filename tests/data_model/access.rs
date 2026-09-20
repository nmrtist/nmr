use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain};
use nmr::raw::{
    AccessError, DirectSamples, IndirectComponents, ObservationOrdinal, RawAxis, RawAxisKind,
    RawDatasetBuilder, RawMetadata, ReadError, ReadErrorKind, ReadErrorReason, ReadResource,
    Region, SamplingCoordinate, SamplingSchedule, SparseTrace,
};

fn axes(indirect: usize, direct: usize) -> Vec<RawAxis> {
    vec![
        RawAxis::new(
            RawAxisKind::Indirect(IndirectComponents::Scalar),
            AxisDomain::Unknown,
            None,
            indirect,
            AxisCoordinates::Unknown,
        )
        .unwrap(),
        RawAxis::new(
            RawAxisKind::Direct(DirectSamples::Real),
            AxisDomain::Unknown,
            None,
            direct,
            AxisCoordinates::Unknown,
        )
        .unwrap(),
    ]
}

fn dense() -> nmr::raw::RawDataset {
    RawDatasetBuilder::new(axes(2, 3), RawMetadata::default())
        .unwrap()
        .dense(
            (0..6)
                .map(|value| Complex64::new(value as f64, 0.0))
                .collect(),
        )
        .unwrap()
}

fn assert_access(error: ReadError, expected: AccessError, kind: ReadErrorKind) {
    assert_eq!(error.kind(), kind);
    assert!(matches!(error.reason(), ReadErrorReason::Access(actual) if actual == &expected));
}

#[test]
fn coordinate_and_region_errors_are_structured() {
    assert_access(
        dense().read_trace(&[]).unwrap_err(),
        AccessError::TraceRankMismatch {
            expected: 1,
            actual: 0,
        },
        ReadErrorKind::Invalid,
    );
    assert_access(
        dense().read_trace(&[2]).unwrap_err(),
        AccessError::TraceOutOfBounds {
            axis: 0,
            index: 2,
            points: 2,
        },
        ReadErrorKind::Invalid,
    );
    let error = dense()
        .data()
        .read_region(&Region::new([0, 2], [1, 2]).unwrap(), usize::MAX)
        .unwrap_err();
    assert!(matches!(
        error.reason(),
        ReadErrorReason::Access(AccessError::RegionOutOfBounds { .. })
    ));
}

#[test]
fn nested_dense_regions_use_absolute_grid_coordinates() {
    let first = dense()
        .data()
        .read_region(&Region::new([1, 1], [1, 2]).unwrap(), usize::MAX)
        .unwrap();
    assert_eq!(first.data().layout().absolute_origin(), &[1, 1]);
    assert_eq!(
        first.data().read_trace(&[1]).unwrap().samples(),
        &[Complex64::new(4.0, 0.0), Complex64::new(5.0, 0.0)]
    );

    let second = first
        .data()
        .read_region(&Region::new([1, 2], [1, 1]).unwrap(), usize::MAX)
        .unwrap();
    assert_eq!(second.data().layout().absolute_origin(), &[1, 2]);
    assert_eq!(
        second.data().read_trace(&[1]).unwrap().samples(),
        &[Complex64::new(5.0, 0.0)]
    );
    assert!(matches!(
        first
            .data()
            .read_region(&Region::new([0, 1], [1, 1]).unwrap(), usize::MAX)
            .unwrap_err()
            .reason(),
        ReadErrorReason::Access(AccessError::RegionOutOfBounds { .. })
    ));
}

#[test]
fn sparse_duplicates_require_ordinal_access_and_regions_keep_absolute_coordinates() {
    let coordinates = vec![
        SamplingCoordinate::new(vec![1]),
        SamplingCoordinate::new(vec![1]),
        SamplingCoordinate::new(vec![2]),
    ];
    let traces = coordinates
        .iter()
        .enumerate()
        .map(|(ordinal, coordinate)| {
            SparseTrace::new(
                ObservationOrdinal::new(ordinal),
                coordinate.clone(),
                vec![Complex64::new(ordinal as f64, 0.0); 2],
            )
        })
        .collect();
    let dataset = RawDatasetBuilder::new(axes(4, 2), RawMetadata::default())
        .unwrap()
        .sparse(traces, SamplingSchedule::new(vec![4], coordinates).unwrap())
        .unwrap();

    assert_access(
        dataset.read_trace(&[1]).unwrap_err(),
        AccessError::AmbiguousObservation {
            coordinate: vec![1],
        },
        ReadErrorKind::AmbiguousObservation,
    );
    let second = dataset
        .read_observation(ObservationOrdinal::new(1))
        .unwrap();
    assert_eq!(
        second.observation_ordinal(),
        Some(ObservationOrdinal::new(1))
    );
    assert_eq!(second.samples()[0], Complex64::new(1.0, 0.0));

    let region = dataset
        .data()
        .read_region(&Region::new([1, 0], [2, 2]).unwrap(), usize::MAX)
        .unwrap();
    assert_eq!(region.data().layout().absolute_origin(), &[1, 0]);
    let retained: Vec<_> = region
        .data()
        .sparse_traces()
        .unwrap()
        .iter()
        .map(|trace| (trace.ordinal(), trace.coordinate().as_slice().to_vec()))
        .collect();
    assert_eq!(
        retained,
        vec![
            (ObservationOrdinal::new(0), vec![1]),
            (ObservationOrdinal::new(1), vec![1]),
            (ObservationOrdinal::new(2), vec![2]),
        ]
    );
    let nested = region
        .data()
        .read_region(&Region::new([2, 1], [1, 1]).unwrap(), usize::MAX)
        .unwrap();
    assert_eq!(nested.data().layout().absolute_origin(), &[2, 1]);
    let nested_trace = &nested.data().sparse_traces().unwrap()[0];
    assert_eq!(nested_trace.ordinal(), ObservationOrdinal::new(2));
    assert_eq!(nested_trace.coordinate().as_slice(), &[2]);
    assert_eq!(nested_trace.samples(), &[Complex64::new(2.0, 0.0)]);
    assert!(matches!(
        dataset
            .data()
            .materialize_dense(Complex64::default(), usize::MAX)
            .unwrap_err()
            .reason(),
        ReadErrorReason::Access(AccessError::AmbiguousObservation { .. })
    ));
}

#[test]
fn limits_are_checked_before_output_allocation() {
    let dataset = dense();
    let region = Region::new([0, 0], [2, 3]).unwrap();
    let error = dataset.data().read_region(&region, 95).unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::LimitExceeded);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::LimitExceeded {
            resource: ReadResource::RegionBytes,
            limit: 95,
            required: 96,
            ..
        }
    ));
}
