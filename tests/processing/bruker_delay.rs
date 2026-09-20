use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    DelaySource, DigitalFilterCorrection, ProcessingError, ProcessingOperation, ProcessingPlan,
    TimeDomainResidualPolicy,
};
use nmr::raw::{DirectSamples, GroupDelayState};

use super::support::*;

#[test]
fn bruker_profile_matches_odd_length_fsh2_and_retains_fractional_residual() {
    let raw = bruker_raw(5, "0.25", "");
    let expected = bruker_oracle(raw.data().dense_samples().unwrap().to_vec(), 0.0);
    let operation = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
            source: DelaySource::AxisEvidence,
            policy: TimeDomainResidualPolicy::IntegerOnlyRetainResidual,
        },
    };
    let output = ProcessingPlan::new(vec![operation])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    assert_eq!(output.descriptor().logical_shape(), [3]);
    assert!(matches!(
        output.provenance().history().unwrap().records()[0],
        nmr::processing::ProcessingRecord::Applied {
            ref resolved,
            ..
        } if matches!(resolved.as_ref(),
            nmr::processing::ResolvedOperation::TimeDomainShiftFoldV1 {
                residual: 0.25,
                ..
            })
    ));
    for (point, expected) in expected.into_iter().enumerate() {
        close_complex(
            Complex64::new(
                output.data().get(&[point], &[0]).unwrap(),
                output.data().get(&[point], &[1]).unwrap(),
            ),
            expected,
        );
    }
}

#[test]
fn bruker_profile_even_fold_uses_an_unmodified_tail_snapshot() {
    let raw = bruker_raw(16, "7.25", "##$DSPFVS= nonsense\n##$DECIM= nonsense\n");
    let expected = bruker_oracle(raw.data().dense_samples().unwrap().to_vec(), 7.25);
    let operation = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
            source: DelaySource::AxisEvidence,
            policy: TimeDomainResidualPolicy::CorrectFully,
        },
    };
    let output = ProcessingPlan::new(vec![operation])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    assert_eq!(output.descriptor().logical_shape(), [7]);
    for (point, expected) in expected.into_iter().enumerate() {
        close_complex(
            Complex64::new(
                output.data().get(&[point], &[0]).unwrap(),
                output.data().get(&[point], &[1]).unwrap(),
            ),
            expected,
        );
    }
}

#[test]
fn bruker_grpdly_sentinel_uses_supported_table_evidence() {
    let raw = bruker_raw(8, "-1", "##$DSPFVS= 13\n##$DECIM= 2\n");
    assert!(matches!(
        raw.descriptor().axes()[0].group_delay(),
        GroupDelayState::Pending(delay) if (delay.delay_points() - 2.75).abs() < 1e-9
    ));
    let operation = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
            source: DelaySource::AxisEvidence,
            policy: TimeDomainResidualPolicy::CorrectFully,
        },
    };
    let corrected = ProcessingPlan::new(vec![operation.clone()])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    let explicit = bruker_raw(8, "2.75", "");
    let expected = ProcessingPlan::new(vec![operation.clone()])
        .unwrap()
        .apply_raw(&explicit)
        .unwrap();
    assert_eq!(corrected.data(), expected.data());

    let unsupported = bruker_raw(8, "-1", "##$DSPFVS= 14\n##$DECIM= 2\n");
    assert!(matches!(
        ProcessingPlan::new(vec![operation])
            .unwrap()
            .apply_raw(&unsupported)
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::MissingCapability { .. }
    ));
}

#[test]
fn bruker_profile_is_not_authorized_for_apply_or_declared_raw() {
    let operation = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
            source: DelaySource::AxisEvidence,
            policy: TimeDomainResidualPolicy::CorrectFully,
        },
    };
    assert!(matches!(
        ProcessingPlan::new(vec![operation])
            .unwrap()
            .apply_processed(&imported_frequency_dataset())
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::InvalidState { .. }
    ));

    let raw = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            4,
            0.0,
            0.25,
        )],
        vec![Complex64::new(0.0, 0.0); 4],
    );
    let axis = ProcessedAxis::new(
        AxisRole::DirectAcquisition,
        AxisDomain::Time,
        Some(AxisUnit::Second),
        4,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.25,
        },
        ComponentBasis::Cartesian,
    )
    .unwrap();
    let descriptor = ProcessedDescriptor::new(vec![axis]).unwrap();
    let declared = ProcessedDataset::new(
        descriptor,
        ProcessedData::new(vec![4], vec![2], vec![0.0; 8]).unwrap(),
        ProcessedProvenance::new(
            ProcessedOrigin::DeclaredRaw {
                snapshot: Box::new(raw.snapshot()),
                axis_lineage: vec![nmr::provenance::InputAxisRef::new(
                    nmr::provenance::InputSlot::new(0),
                    0,
                )],
            },
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    let operation = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
            source: DelaySource::AxisEvidence,
            policy: TimeDomainResidualPolicy::CorrectFully,
        },
    };
    assert!(matches!(
        ProcessingPlan::new(vec![operation])
            .unwrap()
            .apply_processed(&declared)
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::MissingCapability { .. } | ProcessingError::InvalidState { .. }
    ));
}
