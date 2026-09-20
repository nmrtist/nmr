use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{ProcessingError, ProcessingOperation, ProcessingOptions, ProcessingPlan};

fn dataset(coordinates: Vec<(usize, AxisCoordinates)>) -> ProcessedDataset {
    let rank = coordinates.len();
    let shape: Vec<_> = coordinates.iter().map(|(points, _)| *points).collect();
    let axes = coordinates
        .into_iter()
        .enumerate()
        .map(|(index, (points, coordinates))| {
            let unknown = matches!(coordinates, AxisCoordinates::Unknown);
            ProcessedAxis::new(
                if index + 1 == rank {
                    AxisRole::DirectAcquisition
                } else {
                    AxisRole::IndirectAcquisition
                },
                if unknown {
                    AxisDomain::Unknown
                } else {
                    AxisDomain::Frequency
                },
                if unknown { None } else { Some(AxisUnit::Hertz) },
                points,
                coordinates,
                if unknown {
                    ComponentBasis::Scalar
                } else {
                    ComponentBasis::Cartesian
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let counts: Vec<_> = axes.iter().map(ProcessedAxis::component_count).collect();
    let samples = (0..shape.iter().product::<usize>() * counts.iter().product::<usize>())
        .map(|index| index as f64 + 0.5)
        .collect();
    ProcessedDataset::new(
        ProcessedDescriptor::new(axes).unwrap(),
        ProcessedData::new(shape, counts, samples).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
}

fn reversal(axis: usize) -> ProcessingPlan {
    ProcessingPlan::new(vec![ProcessingOperation::ReverseAxis { axis }]).unwrap()
}

#[test]
fn either_coordinate_direction_reverses_every_cartesian_plane_and_replays() {
    for axis in 0..2 {
        let input = dataset(vec![
            (3, AxisCoordinates::Explicit(vec![9.0, 4.0, 1.0])),
            (
                2,
                AxisCoordinates::Uniform {
                    start: -2.0,
                    step: 4.0,
                },
            ),
        ]);
        let first = reversal(axis).apply_processed(&input).unwrap();
        for row in 0..3 {
            for column in 0..2 {
                for a in 0..2 {
                    for b in 0..2 {
                        let source_row = if axis == 0 { 2 - row } else { row };
                        let source_column = if axis == 1 { 1 - column } else { column };
                        let expected =
                            ((source_row * 2 + a) * 4 + source_column * 2 + b) as f64 + 0.5;
                        assert_eq!(first.data().get(&[row, column], &[a, b]).unwrap(), expected);
                    }
                }
            }
        }
        let second = reversal(axis).apply_processed(&first).unwrap();
        drop(first);
        assert_eq!(second.data(), input.data());
        assert_eq!(second.descriptor(), input.descriptor());
        let history = second.provenance().history().unwrap();
        assert_eq!(history.segments().len(), 2);
        assert!(
            history
                .records()
                .iter()
                .all(|record| record.algorithm_version() == Some("reverse-axis.v1"))
        );
        let replay = history
            .replay_processed(&input, ProcessingOptions::default())
            .unwrap();
        assert_eq!(replay.data(), input.data());
        assert_eq!(replay.descriptor(), input.descriptor());
    }
}

#[test]
fn singleton_reversal_records_an_unchanged_permutation() {
    for coordinates in [
        AxisCoordinates::Uniform {
            start: 7.0,
            step: -2.0,
        },
        AxisCoordinates::Explicit(vec![7.0]),
    ] {
        let input = dataset(vec![(1, coordinates)]);
        let output = reversal(0).apply_processed(&input).unwrap();
        assert_eq!(output.data(), input.data());
        assert_eq!(output.descriptor(), input.descriptor());
        assert_eq!(output.provenance().history().unwrap().records().len(), 1);
        assert_eq!(
            output
                .provenance()
                .history()
                .unwrap()
                .replay_processed(&input, ProcessingOptions::default())
                .unwrap()
                .data(),
            input.data()
        );
    }
}

#[test]
fn unknown_coordinates_do_not_authorize_reversal_even_for_a_singleton() {
    for points in [1, 3] {
        let input = dataset(vec![(points, AxisCoordinates::Unknown)]);
        assert!(matches!(
            reversal(0)
                .apply_processed(&input)
                .unwrap_err()
                .into_root_cause(),
            ProcessingError::InvalidState {
                axis: 0,
                reason: "requires established coordinates",
                ..
            }
        ));
    }
}
