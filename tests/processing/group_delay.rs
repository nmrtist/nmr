use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain};
use nmr::processing::{
    DelaySource, DigitalFilterCorrection, FourierExponentSign, FrequencyFrame, ProcessingError,
    ProcessingInput, ProcessingOperation, ProcessingOptions, ProcessingPlan, ReferenceSource,
    TimeDomainResidualPolicy, Window,
};
use nmr::raw::{ChemicalShiftReference, DirectSamples, GroupDelayState, PendingGroupDelay};
use std::f64::consts::PI;

use super::support::*;

#[test]
fn generic_delay_rejects_imported_frequency_but_uses_library_fft_sign() {
    let operation = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(DelaySource::Explicit(1.0)),
    };
    let error = ProcessingPlan::new(vec![operation.clone()])
        .unwrap()
        .apply_processed(&imported_frequency_dataset())
        .unwrap_err();
    assert!(matches!(
        error.root_cause(),
        ProcessingError::InvalidState { .. }
    ));

    for sign in [FourierExponentSign::Negative, FourierExponentSign::Positive] {
        let axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 4, 0.0, 0.25)
            .with_group_delay(GroupDelayState::Pending(
                PendingGroupDelay::user_constructed(1.0).unwrap(),
            ))
            .unwrap();
        let input = vec![
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 2.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ];
        let raw = raw_dataset(vec![axis], input);
        let transformed = ProcessingPlan::new(vec![fft_operation(0, sign)])
            .unwrap()
            .apply_raw(&raw)
            .unwrap();
        let output = ProcessingPlan::new(vec![fft_operation(0, sign), operation.clone()])
            .unwrap()
            .apply_raw(&raw)
            .unwrap();
        let split_output = ProcessingPlan::new(vec![operation.clone()])
            .unwrap()
            .apply_processed(&transformed)
            .unwrap();
        assert_eq!(split_output.data(), output.data());
        let sigma = if sign == FourierExponentSign::Negative {
            -1.0
        } else {
            1.0
        };
        for point in 0..4 {
            let before = Complex64::new(
                transformed.data().get(&[point], &[0]).unwrap(),
                transformed.data().get(&[point], &[1]).unwrap(),
            );
            let q = point as isize - 2;
            let angle = -sigma * 2.0 * PI * q as f64 / 4.0;
            close_complex(
                Complex64::new(
                    output.data().get(&[point], &[0]).unwrap(),
                    output.data().get(&[point], &[1]).unwrap(),
                ),
                before * Complex64::new(angle.cos(), angle.sin()),
            );
        }
    }
}

#[test]
fn continuous_delay_and_reference_state_survives_releasing_the_raw_input() {
    for points in [4, 5] {
        for sign in [FourierExponentSign::Negative, FourierExponentSign::Positive] {
            for delay in [0.0, 0.75] {
                let make_raw = || {
                    let axis =
                        direct_axis(AxisDomain::Time, DirectSamples::Complex, points, 0.0, 0.25)
                            .with_group_delay(GroupDelayState::Pending(
                                PendingGroupDelay::user_constructed(delay).unwrap(),
                            ))
                            .unwrap()
                            .with_chemical_shift_reference(Some(
                                ChemicalShiftReference::user_constructed(4.7, 400.0).unwrap(),
                            ))
                            .unwrap();
                    let mut samples = vec![Complex64::default(); points];
                    samples[0] = Complex64::new(1.0, 0.0);
                    raw_dataset(vec![axis], samples)
                };
                let raw = make_raw();
                let correction = ProcessingOperation::DigitalFilterCorrection {
                    axis: 0,
                    correction: if delay == 0.0 {
                        DigitalFilterCorrection::AcknowledgeZeroDelayV1
                    } else {
                        DigitalFilterCorrection::FrequencyDomainPhaseRampV1(
                            DelaySource::AxisEvidence,
                        )
                    },
                };
                let reference = ProcessingOperation::ResolveFrequencyFrame {
                    axis: 0,
                    frame: FrequencyFrame::Ppm(ReferenceSource::AxisEvidence),
                };
                let full = ProcessingPlan::new(vec![
                    fft_operation(0, sign),
                    correction.clone(),
                    reference.clone(),
                ])
                .unwrap()
                .apply_raw(&raw)
                .unwrap();
                let transformed = ProcessingPlan::new(vec![fft_operation(0, sign)])
                    .unwrap()
                    .apply_raw(&raw)
                    .unwrap();
                drop(raw);
                let corrected = ProcessingPlan::new(vec![correction.clone()])
                    .unwrap()
                    .apply_processed(&transformed)
                    .unwrap();
                if delay == 0.0 {
                    assert_eq!(corrected.data(), transformed.data());
                    assert_eq!(
                        corrected.canonical_digests().samples(),
                        transformed.canonical_digests().samples()
                    );
                    assert_ne!(
                        corrected.canonical_digests().descriptor(),
                        transformed.canonical_digests().descriptor()
                    );
                }
                drop(transformed);
                let split = ProcessingPlan::new(vec![reference])
                    .unwrap()
                    .apply_processed(&corrected)
                    .unwrap();
                drop(corrected);
                assert_eq!(split.data(), full.data());
                assert_eq!(split.descriptor(), full.descriptor());
                assert_eq!(split.canonical_digests(), full.canonical_digests());
                let expected_step = 4.0 / points as f64 / 400.0;
                match split.descriptor().axes()[0].coordinates() {
                    AxisCoordinates::Uniform { start, step } => {
                        close(*start, 4.7 - (points / 2) as f64 * expected_step);
                        close(*step, expected_step);
                    }
                    other => panic!("unexpected coordinates: {other:?}"),
                }
                let sigma = if sign == FourierExponentSign::Negative {
                    -1.0
                } else {
                    1.0
                };
                for point in 0..points {
                    let angle = -sigma * 2.0 * PI * delay * (point as f64 - (points / 2) as f64)
                        / points as f64;
                    close_complex(
                        Complex64::new(
                            split.data().get(&[point], &[0]).unwrap(),
                            split.data().get(&[point], &[1]).unwrap(),
                        ),
                        Complex64::new(angle.cos(), angle.sin()),
                    );
                }
                for repeated in [
                    correction,
                    ProcessingOperation::DigitalFilterCorrection {
                        axis: 0,
                        correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(
                            DelaySource::Explicit(0.75),
                        ),
                    },
                ] {
                    assert!(matches!(
                        ProcessingPlan::new(vec![repeated])
                            .unwrap()
                            .apply_processed(&split)
                            .unwrap_err()
                            .into_root_cause(),
                        ProcessingError::InvalidState {
                            axis: 0,
                            reason: "group delay has already been corrected",
                            ..
                        }
                    ));
                }
                let replay = split
                    .provenance()
                    .history()
                    .unwrap()
                    .replay_raw(&make_raw(), ProcessingOptions::default())
                    .unwrap();
                assert_eq!(replay.data(), full.data());
                assert_eq!(replay.canonical_digests(), full.canonical_digests());
            }
        }
    }
}

#[test]
fn zero_delay_acknowledgement_requires_exact_evidence_and_preserves_time_samples() {
    for delay in [None, Some(0.0), Some(0.75)] {
        let evidence = delay
            .map(|delay| {
                GroupDelayState::Pending(PendingGroupDelay::user_constructed(delay).unwrap())
            })
            .unwrap_or(GroupDelayState::Unknown);
        let raw = raw_dataset(
            vec![
                direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.25)
                    .with_group_delay(evidence)
                    .unwrap(),
            ],
            vec![Complex64::new(1.0, -2.0), Complex64::new(3.0, -4.0)],
        );
        let operation = ProcessingOperation::DigitalFilterCorrection {
            axis: 0,
            correction: DigitalFilterCorrection::AcknowledgeZeroDelayV1,
        };
        let result = ProcessingPlan::new(vec![operation])
            .unwrap()
            .apply_raw(&raw);
        match delay {
            Some(0.0) => {
                let output = result.unwrap();
                assert_eq!(output.data().samples(), &[1.0, -2.0, 3.0, -4.0]);
                assert!(matches!(
                    output.provenance().history().unwrap().records()[0],
                    nmr::processing::ProcessingRecord::Applied {
                        algorithm_version: "acknowledge-zero-delay.v1",
                        ..
                    }
                ));
            }
            Some(_) => assert_eq!(
                result.unwrap_err().into_root_cause(),
                ProcessingError::DelayEvidenceMismatch
            ),
            None => assert!(matches!(
                result.unwrap_err().into_root_cause(),
                ProcessingError::MissingCapability {
                    capability: "zero group-delay evidence",
                    axis: Some(0)
                }
            )),
        }
    }
}

#[test]
fn reversed_fft_bins_reject_delay_in_preflight() {
    for points in [4, 5] {
        for sign in [FourierExponentSign::Negative, FourierExponentSign::Positive] {
            for delay in [0.5, 1.0, 0.0] {
                let mut samples = vec![Complex64::new(0.0, 0.0); points];
                samples[0] = Complex64::new(1.0, 0.0);
                let raw = raw_dataset(
                    vec![direct_axis(
                        AxisDomain::Time,
                        DirectSamples::Complex,
                        points,
                        0.0,
                        0.25,
                    )],
                    samples,
                );
                let correction = ProcessingOperation::DigitalFilterCorrection {
                    axis: 0,
                    correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(
                        DelaySource::Explicit(delay),
                    ),
                };
                let plan = ProcessingPlan::new(vec![
                    fft_operation(0, sign),
                    ProcessingOperation::ReverseAxis { axis: 0 },
                    correction.clone(),
                ])
                .unwrap();
                let error = plan
                    .preflight_raw(
                        &ProcessingInput::from_dataset(&raw).unwrap(),
                        ProcessingOptions::default(),
                    )
                    .unwrap_err();
                assert!(
                    matches!(
                        error.root_cause(),
                        ProcessingError::InvalidState {
                            axis: 0,
                            reason: "requires the canonical centered-bin layout of a library FFT",
                            ..
                        }
                    ),
                    "{error:?}"
                );

                let reversed = ProcessingPlan::new(vec![
                    fft_operation(0, sign),
                    ProcessingOperation::ReverseAxis { axis: 0 },
                ])
                .unwrap()
                .apply_raw(&raw)
                .unwrap();
                let error = ProcessingPlan::new(vec![correction.clone()])
                    .unwrap()
                    .apply_processed(&reversed)
                    .unwrap_err();
                assert!(matches!(
                    error.root_cause(),
                    ProcessingError::InvalidState {
                        axis: 0,
                        reason: "requires the canonical centered-bin layout of a library FFT",
                        ..
                    }
                ));
                let restored =
                    ProcessingPlan::new(vec![ProcessingOperation::ReverseAxis { axis: 0 }])
                        .unwrap()
                        .apply_processed(&reversed)
                        .unwrap();
                let error = ProcessingPlan::new(vec![correction.clone()])
                    .unwrap()
                    .apply_processed(&restored)
                    .unwrap_err();
                assert!(matches!(
                    error.root_cause(),
                    ProcessingError::InvalidState {
                        axis: 0,
                        reason: "requires the canonical centered-bin layout of a library FFT",
                        ..
                    }
                ));

                if delay == 0.0 {
                    continue;
                }
                let output = ProcessingPlan::new(vec![
                    fft_operation(0, sign),
                    correction,
                    ProcessingOperation::ReverseAxis { axis: 0 },
                ])
                .unwrap()
                .apply_raw(&raw)
                .unwrap();
                assert_eq!(output.data().shape(), &[points]);
                assert_eq!(output.data().samples().len(), 2 * points);
                assert_eq!(
                    output
                        .provenance()
                        .history()
                        .unwrap()
                        .records()
                        .last()
                        .unwrap()
                        .algorithm_version(),
                    Some("reverse-axis.v1")
                );
                let sigma = if sign == FourierExponentSign::Negative {
                    -1.0
                } else {
                    1.0
                };
                for point in 0..points {
                    // The impulse FFT is one; reversal maps i to N - 1 - i.
                    let q = (points - 1 - point) as isize - (points / 2) as isize;
                    let angle = -sigma * 2.0 * PI * delay * q as f64 / points as f64;
                    close_complex(
                        Complex64::new(
                            output.data().get(&[point], &[0]).unwrap(),
                            output.data().get(&[point], &[1]).unwrap(),
                        ),
                        Complex64::new(angle.cos(), angle.sin()),
                    );
                }
            }
        }
    }
}

#[test]
fn delay_evidence_mismatch_fails_and_mathematical_identities_succeed() {
    let axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 4, 0.0, 0.25)
        .with_group_delay(GroupDelayState::Pending(
            PendingGroupDelay::user_constructed(1.0).unwrap(),
        ))
        .unwrap();
    let raw = raw_dataset(vec![axis], vec![Complex64::new(1.0, 0.0); 4]);
    let mismatch = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(DelaySource::Explicit(1.1)),
    };
    assert_eq!(
        ProcessingPlan::new(vec![
            fft_operation(0, FourierExponentSign::Negative),
            mismatch,
        ])
        .unwrap()
        .apply_raw(&raw)
        .unwrap_err()
        .into_root_cause(),
        ProcessingError::DelayEvidenceMismatch
    );
    assert!(ProcessingPlan::new(vec![]).is_err());
    assert!(
        ProcessingPlan::new(vec![zero_fill_operation(0, 4)])
            .unwrap()
            .apply_raw(&raw)
            .is_ok()
    );
    let identity = ProcessingOperation::Window {
        axis: 0,
        window: Window::exponential(0.0).unwrap(),
    };
    assert!(
        ProcessingPlan::new(vec![identity])
            .unwrap()
            .apply_raw(&raw)
            .is_ok()
    );
}

#[test]
fn recorded_identities_preserve_first_effective_delay_correction() {
    let raw = bruker_raw(8, "0.25", "");
    let correction = ProcessingOperation::DigitalFilterCorrection {
        axis: 0,
        correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
            source: DelaySource::AxisEvidence,
            policy: TimeDomainResidualPolicy::CorrectFully,
        },
    };
    let direct = ProcessingPlan::new(vec![correction.clone()])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    let with_identities = ProcessingPlan::new(vec![
        ProcessingOperation::Window {
            axis: 0,
            window: Window::exponential(0.0).unwrap(),
        },
        zero_fill_operation(0, 8),
        correction,
    ])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    assert_eq!(with_identities.descriptor(), direct.descriptor());
    assert_eq!(with_identities.data(), direct.data());
    assert_eq!(
        with_identities
            .provenance()
            .history()
            .unwrap()
            .records()
            .len(),
        3
    );
}
