use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::raw::{
    ComponentEvidence, DirectSamples, IndirectComponents, RawAxis, RawAxisKind, RawDatasetBuilder,
    RawMetadata, Region, SamplingCoordinate, SamplingSchedule, ValidationError,
};
use proptest::prelude::*;

fn axes(shape: &[usize], cartesian: &[bool], direct: DirectSamples) -> Vec<RawAxis> {
    let last = shape.len() - 1;
    shape
        .iter()
        .enumerate()
        .map(|(axis, &points)| {
            let kind = if axis == last {
                RawAxisKind::Direct(direct)
            } else if cartesian[axis] {
                RawAxisKind::Indirect(IndirectComponents::Cartesian(
                    ComponentEvidence::user_constructed(),
                ))
            } else {
                RawAxisKind::Indirect(IndirectComponents::Scalar)
            };
            RawAxis::new(
                kind,
                AxisDomain::Time,
                Some(AxisUnit::Second),
                points,
                AxisCoordinates::Unknown,
            )
            .unwrap()
        })
        .collect()
}

proptest! {
    #[test]
    fn dense_storage_and_full_region_preserve_all_samples(
        shape in prop::collection::vec(1usize..5, 1..4),
        cartesian in prop::collection::vec(any::<bool>(), 0..3),
    ) {
        let mut flags = cartesian;
        flags.resize(shape.len().saturating_sub(1), false);
        flags.truncate(shape.len().saturating_sub(1));
        let axes = axes(&shape, &flags, DirectSamples::Complex);
        let lanes: Vec<_> = axes.iter().map(RawAxis::component_lanes).collect();
        let storage_len = shape.iter().zip(&lanes).map(|(p, l)| p * l).product::<usize>();
        let samples: Vec<_> = (0..storage_len).map(|index| Complex64::new(index as f64, -(index as f64))).collect();
        let dataset = RawDatasetBuilder::new(axes, RawMetadata::default()).unwrap().dense(samples.clone()).unwrap();

        prop_assert_eq!(dataset.data().component_lanes(), lanes.as_slice());
        prop_assert_eq!(dataset.data().dense_samples().unwrap(), samples.as_slice());
        prop_assert_eq!(dataset.data().validate(), Ok(()));
        let request = Region::new(vec![0; shape.len()], shape.clone()).unwrap();
        let region = dataset.data().read_region(&request, usize::MAX).unwrap();
        prop_assert_eq!(region.data(), dataset.data());
    }

    #[test]
    fn builder_rejects_every_wrong_dense_length(
        shape in prop::collection::vec(1usize..5, 1..4),
        cartesian in prop::collection::vec(any::<bool>(), 0..3),
        delta in 1usize..5,
    ) {
        let mut flags = cartesian;
        flags.resize(shape.len().saturating_sub(1), false);
        flags.truncate(shape.len().saturating_sub(1));
        let axes = axes(&shape, &flags, DirectSamples::Complex);
        let expected = axes.iter().map(|axis| axis.points() * axis.component_lanes()).product::<usize>();
        let too_short = expected.saturating_sub(delta);
        prop_assume!(too_short != expected);
        let result = RawDatasetBuilder::new(axes, RawMetadata::default()).unwrap().dense(vec![Complex64::new(0.0, 0.0); too_short]);
        prop_assert_eq!(result.unwrap_err(), ValidationError::DenseLengthMismatch);
    }

    #[test]
    fn real_direct_axis_rejects_exactly_nonzero_imaginary_samples(
        imaginary in prop::collection::vec(any::<f64>().prop_filter("finite", |v| v.is_finite()), 1..16),
    ) {
        let axes = axes(&[imaginary.len()], &[], DirectSamples::Real);
        let samples = imaginary.iter().map(|&im| Complex64::new(0.0, im)).collect();
        let result = std::panic::catch_unwind(|| RawDatasetBuilder::new(axes, RawMetadata::default()).unwrap().dense(samples));
        prop_assert!(result.is_ok());
        let result = result.unwrap();
        if imaginary.iter().any(|&im| im != 0.0) {
            prop_assert_eq!(result.unwrap_err(), ValidationError::ImaginarySamplesOnRealAxis);
        } else {
            prop_assert!(result.is_ok());
        }
    }
}

#[test]
fn schedule_retains_repeated_observations_for_each_supported_rank() {
    for rank in 1..=3 {
        let grid = vec![2; rank];
        let coordinate = SamplingCoordinate::new(vec![1; rank]);
        let schedule = SamplingSchedule::new(grid, vec![coordinate.clone(), coordinate]).unwrap();
        assert!(schedule.has_repeated_coordinates());
        assert_eq!(schedule.coordinates().len(), 2);
    }
}

#[test]
fn schedule_rejects_an_empty_coordinate_set() {
    assert_eq!(
        SamplingSchedule::new(vec![2], vec![]),
        Err(ValidationError::EmptySchedule)
    );
}
