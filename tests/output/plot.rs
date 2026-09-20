use nmr::axis::{AxisCoordinates, AxisDirection, AxisDomain, AxisRole};
use nmr::plot::{CoordinateSourceQuality, PlotCoordinates, PlotData, PlotError};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};

fn scalar_dataset(axis: ProcessedAxis, samples: Vec<f64>) -> ProcessedDataset {
    let points = axis.points();
    ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![points], vec![1], samples).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap()
}

#[test]
fn unknown_parameter_coordinates_become_quality_marked_logical_indices() {
    let axis = ProcessedAxis::new(
        AxisRole::ArrayParameter,
        AxisDomain::Parameter,
        None,
        3,
        AxisCoordinates::Unknown,
        ComponentBasis::Scalar,
    )
    .unwrap();
    let plot = PlotData::from_processed(&scalar_dataset(axis, vec![1.0, 2.0, 3.0])).unwrap();
    assert_eq!(
        plot.axes()[0].coordinates(),
        &PlotCoordinates::LogicalIndex(vec![0, 1, 2])
    );
    assert_eq!(
        plot.axes()[0].source_quality(),
        CoordinateSourceQuality::Unknown
    );
    assert_eq!(plot.axes()[0].direction(), AxisDirection::Ascending);
}

#[test]
fn single_and_nonmonotonic_parameter_axes_have_unknown_direction() {
    for coordinates in [
        AxisCoordinates::Explicit(vec![4.0]),
        AxisCoordinates::Explicit(vec![1.0, 3.0, 2.0]),
    ] {
        let points = match &coordinates {
            AxisCoordinates::Explicit(values) => values.len(),
            _ => unreachable!(),
        };
        let axis = ProcessedAxis::new(
            AxisRole::ArrayParameter,
            AxisDomain::Parameter,
            None,
            points,
            coordinates,
            ComponentBasis::Scalar,
        )
        .unwrap();
        let plot = PlotData::from_processed(&scalar_dataset(axis, vec![0.0; points])).unwrap();
        assert_eq!(plot.axes()[0].direction(), AxisDirection::Unknown);
    }
}

#[test]
fn unknown_signal_coordinates_are_not_relabelled_as_logical_indices() {
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Unknown,
        None,
        2,
        AxisCoordinates::Unknown,
        ComponentBasis::Scalar,
    )
    .unwrap();
    assert_eq!(
        PlotData::from_processed(&scalar_dataset(axis, vec![1.0, 2.0])).unwrap_err(),
        PlotError::MissingPhysicalCoordinates
    );
}
