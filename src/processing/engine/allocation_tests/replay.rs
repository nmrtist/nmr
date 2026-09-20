use super::support::*;
use crate::Complex64;
use crate::acquisition::{DirectSamples, RawAxisKind};
use crate::axis::AxisRole;
use crate::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use crate::processed::ProcessedValidationError;
use crate::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
};
use crate::processing::contracts::{error::*, operation::*, options::*};
use crate::processing::engine::execution::*;
use crate::processing::prepare::plan::*;
use crate::processing::prepare::resources::*;
use crate::raw::{RawAxis, RawDatasetBuilder, RawMetadata};

#[test]
fn derived_construction_validates_history_once_and_public_validation_rechecks_it() {
    use crate::processing::contracts::history::HISTORY_VALIDATIONS;
    let input = processed_point();
    let plan = ProcessingPlan::new(vec![phase()]).unwrap();
    let before = HISTORY_VALIDATIONS.with(|count| count.get());
    let first = plan.apply_processed(&input).unwrap();
    assert_eq!(HISTORY_VALIDATIONS.with(|count| count.get()), before + 1);
    first.validate().unwrap();
    assert_eq!(HISTORY_VALIDATIONS.with(|count| count.get()), before + 2);
    let second = plan.apply_processed(&first).unwrap();
    assert_eq!(HISTORY_VALIDATIONS.with(|count| count.get()), before + 3);
    assert!((second.data().samples()[0] + 1.0).abs() < 1e-12);
    assert!((second.data().samples()[1] + 2.0).abs() < 1e-12);
    second.validate().unwrap();
    assert_eq!(HISTORY_VALIDATIONS.with(|count| count.get()), before + 4);
}

#[test]
fn derived_construction_still_rejects_structural_and_history_mismatches() {
    let input = processed_point();
    let result = ProcessingPlan::new(vec![phase()])
        .unwrap()
        .apply_processed(&input)
        .unwrap();
    let data = || ProcessedData::new(vec![2], vec![2], vec![1.0; 4]).unwrap();
    assert!(matches!(
        ProcessedDataset::new_derived(
            result.descriptor().clone(),
            data(),
            result.provenance().clone(),
        ),
        Err(ProcessedValidationError::DescriptorDataMismatch)
    ));
    let wrong_descriptor = ProcessedDescriptor::new(vec![
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
        .unwrap(),
    ])
    .unwrap();
    assert!(matches!(
        ProcessedDataset::new_derived(wrong_descriptor, data(), result.provenance().clone(),),
        Err(ProcessedValidationError::InvalidProcessingHistory)
    ));
}

#[test]
fn segmented_replay_retains_previous_samples_in_addition_to_prepared_vectors() {
    let input = processed_point();
    let plan = ProcessingPlan::new(vec![phase()]).unwrap();
    let first = plan.apply_processed(&input).unwrap();
    let result = plan.apply_processed(&first).unwrap();
    let first_peak = prepared_retained_bytes(1, 1, false).unwrap()
        + state_storage_bytes(1, 0).unwrap()
        + crate::processing::prepare::memory::processed_replay(
            &input,
            first.provenance().history().unwrap(),
        )
        .unwrap()
        + 3 * std::mem::size_of::<Complex64>();
    first
        .provenance()
        .history()
        .unwrap()
        .replay_processed(
            &input,
            ProcessingOptions::new().max_working_bytes(first_peak),
        )
        .unwrap();
    let peak = prepared_retained_bytes(1, 2, false).unwrap()
        + state_storage_bytes(1, 0).unwrap()
        + crate::processing::prepare::memory::processed_replay(
            &input,
            result.provenance().history().unwrap(),
        )
        .unwrap()
        + 4 * std::mem::size_of::<Complex64>();
    let history = result.provenance().history().unwrap();
    assert!(matches!(
        history
            .replay_processed(&input, ProcessingOptions::new().max_working_bytes(peak - 1))
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let replayed = history
        .replay_processed(&input, ProcessingOptions::new().max_working_bytes(peak))
        .unwrap();
    assert_eq!(replayed.data().samples(), result.data().samples());
}

#[test]
fn raw_replay_budgets_historical_axis_labels_separately_from_current_labels() {
    let make = |label: String| {
        let axis = RawAxis::new(
            RawAxisKind::Direct(DirectSamples::Complex),
            AxisDomain::Time,
            Some(AxisUnit::Second),
            2,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.5,
            },
        )
        .unwrap()
        .with_label(Some(label));
        RawDatasetBuilder::new(vec![axis], RawMetadata::default())
            .unwrap()
            .dense(vec![Complex64::new(1.0, 0.0), Complex64::default()])
            .unwrap()
    };
    let original = make("x".repeat(8192));
    let renamed = make("short".into());
    let output = ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply_raw(&original)
    .unwrap();
    let history = output.provenance().history().unwrap();
    let baseline = axis_reservation(
        raw_axis_storage(renamed.descriptor().axes()).unwrap(),
        std::iter::empty(),
        true,
    )
    .unwrap();
    let historical = history_axis_bytes(history).unwrap();
    assert!(historical > baseline);
    let budget = prepared_retained_bytes(1, 1, false).unwrap()
        + 3 * baseline
        + 2 * historical
        + state_storage_bytes(1, 0).unwrap()
        - 1;
    let before = crate::processed::model::AXIS_ALLOCATIONS.with(|count| count.get());
    assert!(matches!(
        history
            .replay_raw(&renamed, ProcessingOptions::new().max_working_bytes(budget))
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            crate::resource::LimitExceeded {
                resource: crate::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    assert_eq!(
        crate::processed::model::AXIS_ALLOCATIONS.with(|count| count.get()),
        before
    );
}

#[test]
fn raw_replay_checks_sample_budgets_before_expanding_samples() {
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
    .unwrap();
    let mut samples = vec![Complex64::default(); 8];
    samples[0] = Complex64::new(1.0, 0.0);
    let raw = RawDatasetBuilder::new(vec![axis], RawMetadata::default())
        .unwrap()
        .dense(samples)
        .unwrap();
    let plan = ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap();
    let output = plan.apply_raw(&raw).unwrap();
    let history = output.provenance().history().unwrap();
    let before = RAW_EXPANSIONS.with(|count| count.get());
    for (options, expected) in [
        (
            ProcessingOptions::new().max_output_bytes(1),
            crate::resource::ResourceKind::OutputBytes,
        ),
        (
            ProcessingOptions::new().max_working_bytes(1),
            crate::resource::ResourceKind::WorkingBytes,
        ),
    ] {
        let error = history.replay_raw(&raw, options).unwrap_err();
        assert!(
            matches!(error.root_cause(), ProcessingError::LimitExceeded(value) if value.resource == expected)
        );
        assert_eq!(RAW_EXPANSIONS.with(|count| count.get()), before);
    }
    let replayed = history.replay_raw(&raw, ProcessingOptions::new()).unwrap();
    assert_eq!(RAW_EXPANSIONS.with(|count| count.get()), before + 1);
    for pair in replayed.data().samples().chunks_exact(2) {
        assert!((pair[0] - 1.0).abs() < 1e-12 && pair[1].abs() < 1e-12);
    }
}
