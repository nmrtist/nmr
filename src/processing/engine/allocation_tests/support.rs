use crate::axis::AxisRole;
use crate::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use crate::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use crate::processing::contracts::operation::*;

pub(super) fn processed_point() -> ProcessedDataset {
    ProcessedDataset::new(
        ProcessedDescriptor::new(vec![
            ProcessedAxis::new(
                AxisRole::Signal,
                AxisDomain::Frequency,
                Some(AxisUnit::Hertz),
                1,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0,
                },
                ComponentBasis::Cartesian,
            )
            .unwrap(),
        ])
        .unwrap(),
        ProcessedData::new(vec![1], vec![2], vec![1.0, 2.0]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
}

pub(super) fn phase() -> ProcessingOperation {
    ProcessingOperation::PhaseCorrection {
        axis: 0,
        correction: PhaseCorrection::new(90.0, 0.0, 0.0).unwrap(),
    }
}
