use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::data::{AccessError, DataBlock, Region};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::raw::{
    DirectSamples, IndirectComponents, ObservationOrdinal, RawAxis, RawAxisKind, RawDatasetBuilder,
    RawMetadata, SamplingCoordinate, SamplingSchedule, SparseTrace,
};
use nmr::resource::{MemoryLimits, ResourceKind};
use nmr::{Complex64, Dataset};

fn exact_block(input: &Dataset, region: &Region) -> DataBlock {
    let estimate = input.region_resources(region).unwrap();
    let limits = MemoryLimits::new()
        .max_output_bytes(estimate.output_bytes())
        .max_metadata_bytes(estimate.metadata_bytes())
        .max_working_bytes(estimate.working_bytes());
    for (limits, resource, required) in [
        (
            limits.max_output_bytes(estimate.output_bytes() - 1),
            ResourceKind::OutputBytes,
            estimate.output_bytes(),
        ),
        (
            limits.max_metadata_bytes(estimate.metadata_bytes() - 1),
            ResourceKind::MetadataBytes,
            estimate.metadata_bytes(),
        ),
        (
            limits.max_working_bytes(estimate.working_bytes() - 1),
            ResourceKind::WorkingBytes,
            estimate.working_bytes(),
        ),
    ] {
        let error = input.read_region(region, limits).unwrap_err();
        assert!(matches!(error, AccessError::LimitExceeded(value)
            if value.resource == resource && value.required == required && value.limit == required - 1));
    }
    input.read_region(region, limits).unwrap()
}

fn raw_axes() -> Vec<RawAxis> {
    vec![
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
    ]
}

#[test]
fn dense_raw_block_owns_samples_and_absolute_region_under_exact_limits() {
    let input = Dataset::from_raw(
        RawDatasetBuilder::new(raw_axes(), RawMetadata::default())
            .unwrap()
            .dense(
                (0..8)
                    .map(|value| Complex64::new(value as f64, -(value as f64)))
                    .collect(),
            )
            .unwrap(),
    );
    let region = Region::new([1, 0], [2, 2]).unwrap();
    let estimate = input.region_resources(&region).unwrap();
    assert_eq!(estimate.output_bytes(), 64);
    assert_eq!(estimate.metadata_bytes(), 12 * std::mem::size_of::<usize>());
    assert_eq!(
        estimate.working_bytes(),
        estimate.output_bytes() + estimate.metadata_bytes()
    );
    let block = exact_block(&input, &region);
    drop(input);
    assert_eq!(block.region(), &region);
    assert!(block.as_processed().is_none());
    let data = block.as_raw().unwrap().data();
    assert_eq!(data.layout().absolute_origin(), &[1, 0]);
    assert_eq!(
        data.dense_samples().unwrap(),
        &[
            Complex64::new(2.0, -2.0),
            Complex64::new(3.0, -3.0),
            Complex64::new(4.0, -4.0),
            Complex64::new(5.0, -5.0)
        ]
    );
}

#[test]
fn sparse_block_preserves_duplicate_observations_and_only_charges_matches() {
    let coordinates: Vec<_> = [3, 1, 1]
        .into_iter()
        .map(|point| SamplingCoordinate::new(vec![point]))
        .collect();
    let traces = coordinates
        .iter()
        .enumerate()
        .map(|(ordinal, coordinate)| {
            SparseTrace::new(
                ObservationOrdinal::new(ordinal),
                coordinate.clone(),
                vec![
                    Complex64::new(ordinal as f64, 0.0),
                    Complex64::new(ordinal as f64, 1.0),
                ],
            )
        })
        .collect();
    let input = Dataset::from_raw(
        RawDatasetBuilder::new(raw_axes(), RawMetadata::default())
            .unwrap()
            .sparse(traces, SamplingSchedule::new(vec![4], coordinates).unwrap())
            .unwrap(),
    );
    let region = Region::new([1, 1], [1, 1]).unwrap();
    let estimate = input.region_resources(&region).unwrap();
    assert_eq!(estimate.output_bytes(), 32);
    assert_eq!(
        estimate.metadata_bytes(),
        12 * std::mem::size_of::<usize>()
            + 2 * (std::mem::size_of::<SparseTrace>()
                + std::mem::size_of::<usize>()
                + std::mem::size_of::<ObservationOrdinal>())
    );
    let block = exact_block(&input, &region);
    let missing = Region::new([0, 0], [1, 1]).unwrap();
    assert!(matches!(
        input.read_region(&missing, MemoryLimits::new()),
        Err(AccessError::Region(
            nmr::raw::AccessError::UnsampledRegion { .. }
        ))
    ));
    drop(input);
    let traces = block.as_raw().unwrap().data().sparse_traces().unwrap();
    assert_eq!(traces.len(), 2);
    assert_eq!(traces[0].ordinal().get(), 1);
    assert_eq!(traces[1].ordinal().get(), 2);
    assert_eq!(traces[0].samples(), &[Complex64::new(1.0, 1.0)]);
    assert_eq!(traces[1].samples(), &[Complex64::new(2.0, 1.0)]);
}

#[test]
fn processed_block_keeps_all_three_axis_components_and_outlives_input() {
    let axes = [2, 3, 4]
        .into_iter()
        .map(|points| {
            ProcessedAxis::new(
                AxisRole::Signal,
                AxisDomain::Frequency,
                Some(AxisUnit::Hertz),
                points,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0,
                },
                ComponentBasis::Cartesian,
            )
            .unwrap()
        })
        .collect();
    let input = Dataset::from_processed(
        ProcessedDataset::new(
            ProcessedDescriptor::new(axes).unwrap(),
            ProcessedData::new(
                vec![2, 3, 4],
                vec![2, 2, 2],
                (0..192).map(|value| value as f64).collect(),
            )
            .unwrap(),
            ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
        )
        .unwrap(),
    );
    let region = Region::new([1, 1, 1], [1, 2, 2]).unwrap();
    let estimate = input.region_resources(&region).unwrap();
    assert_eq!(estimate.output_bytes(), 256);
    assert_eq!(estimate.metadata_bytes(), 12 * std::mem::size_of::<usize>());
    let block = exact_block(&input, &region);
    for invalid in [
        Region::new([0], [1]).unwrap(),
        Region::new([usize::MAX, 0, 0], [1, 1, 1]).unwrap(),
    ] {
        assert!(matches!(
            input.read_region(&invalid, MemoryLimits::new().max_working_bytes(0)),
            Err(AccessError::Region(_))
        ));
    }
    drop(input);
    assert!(block.as_raw().is_none());
    assert_eq!(block.region(), &region);
    let data = block.as_processed().unwrap();
    assert_eq!(data.shape(), &[1, 2, 2]);
    assert_eq!(data.component_counts(), &[2, 2, 2]);
    let mut expected = Vec::new();
    for slow in 0..2 {
        for middle in 0..4 {
            for fast in 0..4 {
                expected.push((((2 + slow) * 6 + 2 + middle) * 8 + 2 + fast) as f64);
            }
        }
    }
    assert_eq!(data.samples(), expected);
}
