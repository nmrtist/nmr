use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processed::{ProcessedDataset, ProcessedOrigin, ProcessedValidationError};
use nmr::processing::{
    FourierExponentSign, ProcessingError, ProcessingInput, ProcessingOptions, ProcessingPlan,
};
use nmr::raw::{
    DirectSamples, IndirectComponents, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata,
};

use super::support::*;

#[test]
fn prepared_plan_rejects_descriptor_and_sample_identity_changes() {
    let axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.5);
    let original = raw_dataset(
        vec![axis],
        vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
    );
    let plan = ProcessingPlan::new(vec![zero_fill_operation(0, 4)]).unwrap();
    let prepared = plan
        .preflight_raw(
            &ProcessingInput::from_dataset(&original).unwrap(),
            ProcessingOptions::default(),
        )
        .unwrap();
    assert!(prepared.apply(&original).is_ok());

    let changed_samples = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            2,
            0.0,
            0.5,
        )],
        vec![Complex64::new(1.0, 0.0), Complex64::new(3.0, 0.0)],
    );
    assert_eq!(
        prepared
            .apply(&changed_samples)
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::InputIdentityMismatch
    );

    let changed_descriptor = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            2,
            1.0,
            0.5,
        )],
        vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
    );
    assert_eq!(
        prepared
            .apply(&changed_descriptor)
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::InputIdentityMismatch
    );
}

#[test]
fn preflight_is_final_authority_for_rank_and_dense_sampling() {
    let indirect = || {
        RawAxis::new(
            RawAxisKind::Indirect(IndirectComponents::Scalar),
            AxisDomain::Time,
            Some(AxisUnit::Second),
            2,
            AxisCoordinates::Unknown,
        )
        .unwrap()
    };
    let rank_three = raw_dataset(
        vec![
            indirect(),
            indirect(),
            direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.5),
        ],
        vec![Complex64::default(); 8],
    );
    let plan = ProcessingPlan::new(vec![zero_fill_operation(2, 4)]).unwrap();
    assert_eq!(
        plan.preflight_raw(
            &ProcessingInput::from_dataset(&rank_three).unwrap(),
            ProcessingOptions::default(),
        )
        .unwrap_err()
        .into_root_cause(),
        ProcessingError::UnsupportedRank { rank: 3 }
    );

    let coordinates = vec![nmr::raw::SamplingCoordinate::new(vec![0])];
    let trace = nmr::raw::SparseTrace::new(
        nmr::raw::ObservationOrdinal::new(0),
        coordinates[0].clone(),
        vec![Complex64::default(); 2],
    );
    let sparse = RawDatasetBuilder::new(
        vec![
            indirect(),
            direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.5),
        ],
        RawMetadata::default(),
    )
    .unwrap()
    .sparse(
        vec![trace],
        nmr::raw::SamplingSchedule::new(vec![2], coordinates).unwrap(),
    )
    .unwrap();
    let plan = ProcessingPlan::new(vec![zero_fill_operation(1, 4)]).unwrap();
    assert!(matches!(
        plan.preflight_raw(
            &ProcessingInput::from_dataset(&sparse).unwrap(),
            ProcessingOptions::default(),
        )
        .unwrap_err()
        .into_root_cause(),
        ProcessingError::MissingCapability {
            capability: "dense sampling",
            axis: None
        }
    ));
}

#[test]
fn prepared_raw_accepts_display_label_changes_with_same_canonical_identity() {
    let original = labeled_raw("original");
    let renamed = labeled_raw("renamed");
    assert_eq!(original.canonical_digests(), renamed.canonical_digests());
    let plan = ProcessingPlan::new(vec![fft_operation(0, FourierExponentSign::Negative)]).unwrap();
    let prepared = plan
        .preflight_raw(
            &ProcessingInput::from_dataset(&original).unwrap(),
            ProcessingOptions::new(),
        )
        .unwrap();
    let output = prepared.apply(&renamed).unwrap();
    assert_eq!(output.descriptor().axes()[0].label(), Some("original"));
    assert_eq!(output.data().samples(), &[1.0, 0.0, 1.0, 0.0]);
    let ProcessedOrigin::DerivedRaw(origin) = output.provenance().origin() else {
        panic!("derived raw origin");
    };
    assert_eq!(
        origin.snapshot().descriptor().acquisition().title(),
        Some("title renamed")
    );
    assert_eq!(
        origin.snapshot().descriptor().axes()[0].label(),
        Some("renamed")
    );
    output.validate().unwrap();
}

#[test]
fn output_working_limits_and_cloned_derived_provenance_are_rejected() {
    let raw = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            2,
            0.0,
            0.5,
        )],
        vec![Complex64::new(1.0, 0.0); 2],
    );
    let plan = ProcessingPlan::new(vec![zero_fill_operation(0, 4)]).unwrap();
    assert_eq!(
        plan.apply_raw_with_options(&raw, ProcessingOptions::new().max_output_bytes(63))
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::LimitExceeded(nmr::resource::LimitExceeded {
            resource: nmr::resource::ResourceKind::OutputBytes,
            required: 64,
            limit: 63
        })
    );
    assert!(matches!(
        plan.apply_raw_with_options(&raw, ProcessingOptions::new().max_working_bytes(95))
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            nmr::resource::LimitExceeded {
                resource: nmr::resource::ResourceKind::WorkingBytes,
                limit: 95,
                ..
            }
        ))
    ));
    let output = plan.apply_raw(&raw).unwrap();
    assert_eq!(
        ProcessedDataset::new(
            output.descriptor().clone(),
            output.data().clone(),
            output.provenance().clone(),
        )
        .unwrap_err(),
        ProcessedValidationError::LibraryDerivedProvenance
    );
}
