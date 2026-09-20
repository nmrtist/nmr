use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::PolarityState;
use nmr::processing::{
    FourierExponentSign, FrequencyFrame, PhaseCorrection, ProcessingError, ProcessingOperation,
    ProcessingOptions, ProcessingPlan, Projection, ReferenceSource,
};
use nmr::raw::{ChemicalShiftReference, DirectSamples};

use super::support::*;

#[test]
fn history_replays_resolved_operations_bitwise_without_source_metadata() {
    let raw = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            4,
            0.0,
            0.25,
        )],
        vec![
            Complex64::new(1.0, -0.5),
            Complex64::new(2.0, 0.25),
            Complex64::new(-1.5, 3.0),
            Complex64::new(0.75, -2.0),
        ],
    );
    let source_metadata = raw.provenance().source_metadata();
    assert!(source_metadata.as_bruker().is_none());
    assert!(source_metadata.as_jeol().is_none());
    assert!(source_metadata.as_varian().is_none());
    let plan = ProcessingPlan::new(vec![
        zero_fill_operation(0, 8),
        fft_operation(0, FourierExponentSign::Negative),
        ProcessingOperation::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(17.0, -23.0, 0.375).unwrap(),
        },
    ])
    .unwrap();
    let processed = plan.apply_raw(&raw).unwrap();
    let history = processed.provenance().history().unwrap();
    assert_eq!(history.raw_input_digests(), Some(raw.canonical_digests()));
    assert_eq!(
        history
            .raw_input_normalization()
            .unwrap()
            .source_block_scale_factors(),
        &[1.0]
    );

    let replayed = history
        .replay_raw(&raw, ProcessingOptions::default())
        .unwrap();
    assert_eq!(replayed.descriptor(), processed.descriptor());
    assert_eq!(
        replayed
            .data()
            .samples()
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        processed
            .data()
            .samples()
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        replayed.provenance().history(),
        processed.provenance().history()
    );

    let changed = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            4,
            0.0,
            0.25,
        )],
        vec![
            Complex64::new(1.0, -0.5),
            Complex64::new(2.0, 0.25),
            Complex64::new(-1.5, 3.0),
            Complex64::new(0.75, -2.5),
        ],
    );
    assert_eq!(
        history
            .replay_raw(&changed, ProcessingOptions::default())
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::InputIdentityMismatch
    );
}

#[test]
fn raw_history_accepts_display_label_changes_with_same_canonical_identity() {
    let original = labeled_raw("original");
    let renamed = labeled_raw("renamed");
    let plan = ProcessingPlan::new(vec![fft_operation(0, FourierExponentSign::Negative)]).unwrap();
    let expected = plan.apply_raw(&original).unwrap();
    let actual = expected
        .provenance()
        .history()
        .unwrap()
        .replay_raw(&renamed, ProcessingOptions::new())
        .unwrap();
    assert_eq!(actual.descriptor(), expected.descriptor());
    assert_eq!(actual.canonical_digests(), expected.canonical_digests());
    assert_eq!(actual.data().samples(), expected.data().samples());
    actual.validate().unwrap();
}

#[test]
fn ppm_axis_evidence_is_optional_until_the_frame_is_requested() {
    let dataset = imported_frequency_dataset();
    assert!(
        dataset.descriptor().axes()[0]
            .frequency_evidence()
            .is_none()
    );
    let error = ProcessingPlan::new(vec![ProcessingOperation::ResolveFrequencyFrame {
        axis: 0,
        frame: FrequencyFrame::Ppm(ReferenceSource::AxisEvidence),
    }])
    .unwrap()
    .apply_processed(&dataset)
    .unwrap_err();
    assert_eq!(
        error.into_root_cause(),
        ProcessingError::MissingCapability {
            capability: "chemical-shift reference",
            axis: Some(0),
        }
    );
}

#[test]
fn additional_processing_recovers_raw_ppm_evidence_from_history() {
    let reference = ChemicalShiftReference::user_constructed(4.7, 400.0).unwrap();
    let axis = direct_axis(
        AxisDomain::Frequency,
        DirectSamples::Complex,
        2,
        -100.0,
        100.0,
    )
    .with_chemical_shift_reference(Some(reference))
    .unwrap();
    let raw = raw_dataset(
        vec![axis],
        vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
    );
    let phased = ProcessingPlan::new(vec![ProcessingOperation::PhaseCorrection {
        axis: 0,
        correction: PhaseCorrection::new(10.0, 0.0, 0.5).unwrap(),
    }])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    let ppm = ProcessingPlan::new(vec![ProcessingOperation::ResolveFrequencyFrame {
        axis: 0,
        frame: FrequencyFrame::Ppm(ReferenceSource::AxisEvidence),
    }])
    .unwrap()
    .apply_processed(&phased)
    .unwrap();
    assert_eq!(ppm.descriptor().axes()[0].unit(), Some(AxisUnit::Ppm));
    assert_eq!(
        ppm.descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Uniform {
            start: 4.45,
            step: 0.25,
        }
    );
}

#[test]
fn phase_uses_full_width_pivot_and_history_appends() {
    let axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 4, 0.0, 0.25);
    let raw = raw_dataset(
        vec![axis],
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
    );
    let transformed = ProcessingPlan::new(vec![fft_operation(0, FourierExponentSign::Negative)])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    let phased = ProcessingPlan::new(vec![ProcessingOperation::PhaseCorrection {
        axis: 0,
        correction: PhaseCorrection::new(0.0, 360.0, 1.0).unwrap(),
    }])
    .unwrap()
    .apply_processed(&transformed)
    .unwrap();
    let history = phased.provenance().history().unwrap();
    assert_eq!(history.records().len(), 2);
    // k=0 has -360 degrees (identity), while k=3 has -90 degrees.
    close(phased.data().get(&[0], &[0]).unwrap(), 1.0);
    close(phased.data().get(&[3], &[1]).unwrap(), -1.0);
    phased.validate().unwrap();
}

#[test]
fn processed_history_shares_unchanged_axis_storage_and_builders_detach() {
    let points = 4096;
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        points,
        AxisCoordinates::Explicit((0..points).map(|point| point as f64).collect()),
        ComponentBasis::Cartesian,
    )
    .unwrap()
    .with_label(Some("original".into()));
    let clone = axis.clone();
    assert!(std::ptr::eq(axis.coordinates(), clone.coordinates()));
    let renamed = clone.with_label(Some("renamed".into()));
    assert_eq!(axis.label(), Some("original"));
    assert_eq!(renamed.label(), Some("renamed"));
    assert_eq!(renamed.coordinates(), axis.coordinates());
    let dataset = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![points], vec![2], vec![1.0; points * 2]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let output = ProcessingPlan::new(vec![
        ProcessingOperation::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(90.0, 0.0, 0.0).unwrap(),
        };
        5
    ])
    .unwrap()
    .apply_processed(&dataset)
    .unwrap();
    for record in output.provenance().history().unwrap().records() {
        let nmr::processing::ProcessingRecord::Applied {
            input_descriptor,
            output_descriptor,
            ..
        } = record
        else {
            panic!("explicit phase must produce an applied record");
        };
        for descriptor in [input_descriptor, output_descriptor] {
            assert!(std::ptr::eq(
                dataset.descriptor().axes()[0].coordinates(),
                descriptor.axes()[0].coordinates()
            ));
        }
    }
    for pair in output.data().samples().chunks_exact(2) {
        close(pair[0], -1.0);
        close(pair[1], 1.0);
    }
    assert!(dataset.data().samples().iter().all(|value| *value == 1.0));
}

#[test]
fn projections_enforce_phase_and_polarity_and_magnitude_uses_scaled_l2() {
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        1,
        AxisCoordinates::Explicit(vec![0.0]),
        ComponentBasis::Cartesian,
    )
    .unwrap();
    let dataset = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![1], vec![2], vec![3e200, 4e200]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap();
    let magnitude_plan = ProcessingPlan::new(vec![ProcessingOperation::Projection {
        projection: Projection::Magnitude,
        polarity: PolarityState::Ambiguous180,
    }])
    .unwrap();
    // Two current fields, one output scalar, and two gathered fields.
    let peak = 5 * std::mem::size_of::<f64>();
    assert!(matches!(
        magnitude_plan
            .apply_processed_with_options(
                &dataset,
                ProcessingOptions::new().max_working_bytes(peak - 1),
            )
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            nmr::resource::LimitExceeded {
                resource: nmr::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let magnitude = magnitude_plan
        .apply_processed_with_options(&dataset, ProcessingOptions::new())
        .unwrap();
    assert!((magnitude.data().samples()[0] / 5e200 - 1.0).abs() < 1e-15);

    let absorptive = ProcessingPlan::new(vec![
        ProcessingOperation::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(0.0, 0.0, 0.0).unwrap(),
        },
        ProcessingOperation::Projection {
            projection: Projection::RealAbsorptive,
            polarity: PolarityState::Ambiguous180,
        },
    ])
    .unwrap()
    .apply_processed(&dataset)
    .unwrap_err();
    assert!(matches!(
        absorptive.root_cause(),
        ProcessingError::InvalidDatasetState { .. }
    ));
}

#[test]
fn descending_ppm_reverses_samples_and_coordinates_together() {
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        2,
        AxisCoordinates::Uniform {
            start: -100.0,
            step: 100.0,
        },
        ComponentBasis::Cartesian,
    )
    .unwrap()
    .with_nucleus(Some("1H".into()))
    .unwrap();
    let dataset = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![2], vec![2], vec![1.0, 0.0, 2.0, 0.0]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap();
    let reference = ChemicalShiftReference::user_constructed(0.0, 400.0).unwrap();
    let output = ProcessingPlan::new(vec![
        ProcessingOperation::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(0.0, 0.0, 0.0).unwrap(),
        },
        ProcessingOperation::Projection {
            projection: Projection::RealSigned,
            polarity: PolarityState::Ambiguous180,
        },
        ProcessingOperation::ResolveFrequencyFrame {
            axis: 0,
            frame: FrequencyFrame::Ppm(ReferenceSource::Explicit(reference)),
        },
        ProcessingOperation::ReverseAxis { axis: 0 },
    ])
    .unwrap()
    .apply_processed(&dataset)
    .unwrap();
    assert_eq!(output.data().samples(), [2.0, 1.0]);
    assert_eq!(
        output.descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Uniform {
            start: 0.0,
            step: -0.25,
        }
    );
}
