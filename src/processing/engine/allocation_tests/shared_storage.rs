use crate::axis::AxisRole;
use crate::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use crate::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use crate::processing::contracts::{error::*, operation::*, options::*};
use crate::processing::kernels::tensor::*;
use crate::processing::prepare::plan::*;
use crate::processing::prepare::resources::*;

#[test]
fn allocation_free_tensor_indices_preserve_row_major_components_and_bounds() {
    let descriptor = ProcessedDescriptor::new(
        (0..3)
            .map(|_| {
                ProcessedAxis::new(
                    AxisRole::Signal,
                    AxisDomain::Frequency,
                    Some(AxisUnit::Hertz),
                    2,
                    AxisCoordinates::Uniform {
                        start: 0.0,
                        step: 1.0,
                    },
                    ComponentBasis::Cartesian,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    for a in 0..2 {
        for b in 0..2 {
            for c in 0..2 {
                for ar in 0..2 {
                    for br in 0..2 {
                        for cr in 0..2 {
                            let expected = 16 * (2 * a + ar) + 4 * (2 * b + br) + 2 * c + cr;
                            assert_eq!(
                                tensor_index(&descriptor, &[a, b, c], &[ar, br, cr]).unwrap(),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(tensor_index(&descriptor, &[0, 0], &[0, 0]).is_err());
    assert!(tensor_index(&descriptor, &[2, 0, 0], &[0, 0, 0]).is_err());
    assert!(tensor_index(&descriptor, &[0, 0, 0], &[0, 2, 0]).is_err());
    for axis in 0..3 {
        let shape = [2, 3, 4];
        let samples: Vec<_> = (0..24).map(|n| n as f64).collect();
        for a in 0..2 {
            for b in 0..3 {
                for c in 0..4 {
                    let coordinate = [a, b, c];
                    let other: Vec<_> = coordinate
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &v)| (i != axis).then_some(v))
                        .collect();
                    let index = 12 * a + 4 * b + c;
                    assert_eq!(
                        line_sample_index(&shape, axis, &other, coordinate[axis]).unwrap(),
                        index
                    );
                    assert_eq!(
                        sample_at(&samples, &shape, axis, &other, coordinate[axis]).unwrap(),
                        index as f64
                    );
                }
            }
        }
    }
    assert!(line_sample_index(&[2, 3], 2, &[0], 0).is_err());
    assert!(line_sample_index(&[2, 3], 0, &[], 0).is_err());
    assert!(line_sample_index(&[2, 3], 0, &[3], 0).is_err());
    assert!(line_sample_index(&[2, usize::MAX], 0, &[usize::MAX - 1], 1).is_err());
}

#[test]
fn explicit_axis_backing_is_checked_before_rebuild_and_preserves_portable_metadata() {
    use crate::processed::model::AXIS_ALLOCATIONS;
    let points = 4096;
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        points,
        AxisCoordinates::Explicit((0..points).map(|point| point as f64).collect()),
        ComponentBasis::Scalar,
    )
    .unwrap()
    .with_label(Some("retained label".into()))
    .with_nucleus(Some("H1".into()))
    .unwrap();
    let input = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(
            vec![points],
            vec![1],
            (0..points).map(|point| point as f64).collect(),
        )
        .unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let plan = ProcessingPlan::new(vec![ProcessingOperation::ReverseAxis { axis: 0 }]).unwrap();
    let axis_bytes = reserved_processed_axis_bytes(&input, plan.operations()).unwrap();
    assert!(axis_bytes > points * std::mem::size_of::<f64>());
    let prepared = prepared_retained_bytes(1, 1, true).unwrap();
    let input = crate::Dataset::from_processed(input);
    let before = AXIS_ALLOCATIONS.with(|count| count.get());
    assert!(matches!(
        plan.preflight(
            &input,
            ProcessingOptions::new().max_working_bytes(prepared + axis_bytes - 1)
        )
        .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(AXIS_ALLOCATIONS.with(|count| count.get()), before);
    let peak = prepared
        + axis_bytes
        + state_storage_bytes(1, 0).unwrap()
        + crate::processing::prepare::memory::processed_apply(input.as_processed().unwrap(), 1)
            .unwrap()
        + 2 * points * std::mem::size_of::<f64>();
    assert!(matches!(
        plan.preflight(&input, ProcessingOptions::new().max_working_bytes(peak - 1))
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let output = plan
        .preflight(&input, ProcessingOptions::new().max_working_bytes(peak))
        .unwrap()
        .execute()
        .unwrap();
    let processed = output.as_processed().unwrap();
    assert_eq!(
        processed.descriptor().axes()[0].label(),
        Some("retained label")
    );
    assert_eq!(processed.descriptor().axes()[0].nucleus(), Some("1H"));
    for (index, &value) in processed.data().samples().iter().enumerate() {
        assert_eq!(value, (points - 1 - index) as f64);
    }
    assert_eq!(input.as_processed().unwrap().data().samples()[0], 0.0);
}
