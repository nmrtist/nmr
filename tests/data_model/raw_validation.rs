use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain as Domain, AxisUnit};
use nmr::raw::{
    ComponentEvidence, DirectSamples, IndirectComponents, ObservationOrdinal, RawAxis as Axis,
    RawAxisKind, RawDatasetBuilder, RawMetadata as AcquisitionMetadata, SamplingCoordinate,
    SamplingSchedule, SparseTrace, ValidationError,
};

#[test]
fn shared_indirect_components_require_complex_direct_samples() {
    let indirect = Axis::new(
        RawAxisKind::Indirect(IndirectComponents::SharedComplex {
            conjugated: true,
            evidence: ComponentEvidence::user_constructed(),
        }),
        Domain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    for samples in [DirectSamples::Real, DirectSamples::Complex] {
        let direct = Axis::new(
            RawAxisKind::Direct(samples),
            Domain::Time,
            Some(AxisUnit::Second),
            3,
            AxisCoordinates::Unknown,
        )
        .unwrap();
        let builder = RawDatasetBuilder::new(
            vec![indirect.clone(), direct],
            AcquisitionMetadata::default(),
        );
        assert_eq!(builder.is_ok(), samples == DirectSamples::Complex);
    }
}

#[test]
fn public_builder_derives_layout_and_rejects_bad_sample_lengths() {
    let indirect = Axis::new(
        RawAxisKind::Indirect(IndirectComponents::Cartesian(
            ComponentEvidence::user_constructed(),
        )),
        Domain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let direct = Axis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        Domain::Time,
        Some(AxisUnit::Second),
        3,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let builder =
        RawDatasetBuilder::new(vec![indirect, direct], AcquisitionMetadata::default()).unwrap();
    assert_eq!(
        builder.dense(vec![Complex64::default(); 11]).unwrap_err(),
        ValidationError::DenseLengthMismatch
    );
}

#[test]
fn public_builder_rejects_non_finite_and_real_direct_imaginary_samples() {
    let complex = Axis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        Domain::Time,
        Some(AxisUnit::Second),
        1,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    assert_eq!(
        RawDatasetBuilder::new(vec![complex], AcquisitionMetadata::default())
            .unwrap()
            .dense(vec![Complex64::new(f64::NAN, 0.0)])
            .unwrap_err(),
        ValidationError::NonFiniteSample
    );

    let real = Axis::new(
        RawAxisKind::Direct(DirectSamples::Real),
        Domain::Time,
        Some(AxisUnit::Second),
        1,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    assert_eq!(
        RawDatasetBuilder::new(vec![real], AcquisitionMetadata::default())
            .unwrap()
            .dense(vec![Complex64::new(0.0, 1.0)])
            .unwrap_err(),
        ValidationError::ImaginarySamplesOnRealAxis
    );
}

#[test]
fn sparse_builder_requires_schedule_order_and_unique_ordinals() {
    let indirect = Axis::new(
        RawAxisKind::Indirect(IndirectComponents::Scalar),
        Domain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let direct = Axis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        Domain::Time,
        Some(AxisUnit::Second),
        1,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let coordinate = SamplingCoordinate::new(vec![0]);
    let traces = vec![
        SparseTrace::new(
            ObservationOrdinal::new(0),
            coordinate.clone(),
            vec![Complex64::default()],
        ),
        SparseTrace::new(
            ObservationOrdinal::new(0),
            coordinate.clone(),
            vec![Complex64::default()],
        ),
    ];
    let schedule = SamplingSchedule::new(vec![2], vec![coordinate.clone(), coordinate]).unwrap();
    assert_eq!(
        RawDatasetBuilder::new(vec![indirect, direct], AcquisitionMetadata::default())
            .unwrap()
            .sparse(traces, schedule)
            .unwrap_err(),
        ValidationError::DuplicateObservationOrdinal
    );

    let indirect = Axis::new(
        RawAxisKind::Indirect(IndirectComponents::Scalar),
        Domain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let direct = Axis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        Domain::Time,
        Some(AxisUnit::Second),
        1,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let first = SamplingCoordinate::new(vec![0]);
    let second = SamplingCoordinate::new(vec![1]);
    let out_of_order = vec![
        SparseTrace::new(
            ObservationOrdinal::new(1),
            first.clone(),
            vec![Complex64::default()],
        ),
        SparseTrace::new(
            ObservationOrdinal::new(0),
            second.clone(),
            vec![Complex64::default()],
        ),
    ];
    assert_eq!(
        RawDatasetBuilder::new(vec![indirect, direct], AcquisitionMetadata::default())
            .unwrap()
            .sparse(
                out_of_order,
                SamplingSchedule::new(vec![2], vec![first, second]).unwrap(),
            )
            .unwrap_err(),
        ValidationError::ScheduleDataMismatch
    );
}
