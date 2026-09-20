use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    AslsError, BaselineProfile, PositivePeaksV1, ProcessingError, ProcessingOperation,
    ProcessingOptions, ProcessingPlan, ProcessingRecord, ResolvedOperation,
};

fn synthetic_trace(points: usize) -> (Vec<f64>, Vec<f64>) {
    let coordinates: Vec<_> = (0..points)
        .map(|index| {
            let fraction = index as f64 / (points - 1) as f64;
            1200.0 * fraction.powf(1.15)
        })
        .collect();
    let samples = coordinates
        .iter()
        .map(|x| {
            let baseline = 2.0 + 0.003 * x;
            let peak = 20.0 * (-((*x - 530.0) / 22.0).powi(2)).exp();
            baseline + peak
        })
        .collect();
    (coordinates, samples)
}

#[test]
fn positive_peaks_profile_removes_a_smooth_nonuniform_baseline() {
    let (coordinates, samples) = synthetic_trace(129);
    let corrected = PositivePeaksV1.subtract(&coordinates, &samples).unwrap();

    assert!(corrected[0].abs() < 0.1);
    assert!(corrected[128].abs() < 0.1);
    assert!(corrected.iter().copied().fold(f64::NEG_INFINITY, f64::max) > 18.0);
}

#[test]
fn coordinate_normalization_is_affine_invariant() {
    let (coordinates, samples) = synthetic_trace(97);
    let affine: Vec<_> = coordinates
        .iter()
        .map(|value| 17.0 + 3.25 * value)
        .collect();
    let first = PositivePeaksV1.subtract(&coordinates, &samples).unwrap();
    let second = PositivePeaksV1.subtract(&affine, &samples).unwrap();

    for (left, right) in first.iter().zip(second) {
        assert!((left - right).abs() < 1e-8);
    }
}

#[test]
fn malformed_coordinates_fail_closed() {
    assert_eq!(
        PositivePeaksV1
            .subtract(&[0.0, 1.0, 1.0], &[1.0, 2.0, 3.0])
            .unwrap_err(),
        AslsError::NonMonotonicCoordinates
    );
    assert_eq!(
        PositivePeaksV1
            .subtract(&[0.0, 1.0, 2.0], &[1.0, f64::NAN, 3.0])
            .unwrap_err(),
        AslsError::NonFiniteInput
    );
}

#[test]
fn a_zero_trace_converges_without_inventing_a_baseline() {
    let coordinates = [0.0, 0.2, 0.7, 1.0];
    assert_eq!(
        PositivePeaksV1.subtract(&coordinates, &[0.0; 4]).unwrap(),
        [0.0; 4]
    );
}

#[test]
fn processing_records_the_resolved_baseline_operation() {
    let (coordinates, samples) = synthetic_trace(65);
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        samples.len(),
        AxisCoordinates::Explicit(coordinates),
        ComponentBasis::Scalar,
    )
    .unwrap();
    let dataset = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![samples.len()], vec![1], samples).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap();
    let operation = ProcessingOperation::BaselineCorrection {
        axis: 0,
        profile: PositivePeaksV1.into(),
    };
    let corrected = ProcessingPlan::new(vec![operation.clone()])
        .unwrap()
        .apply_processed(&dataset)
        .unwrap();
    assert_ne!(corrected.data().samples(), dataset.data().samples());
    assert!(matches!(
        &corrected.provenance().history().unwrap().records()[0],
        ProcessingRecord::Applied {
            requested,
            resolved,
            ..
        } if requested.explicit() == Some(&operation)
            && matches!(resolved.as_ref(), ResolvedOperation::BaselineCorrection(BaselineProfile::PositivePeaksV1(_)))
    ));

    let limited = ProcessingPlan::new(vec![operation])
        .unwrap()
        .apply_processed_with_options(&dataset, ProcessingOptions::new().max_working_bytes(10_399));
    assert!(matches!(
        limited.map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            nmr::resource::LimitExceeded {
                resource: nmr::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
}
