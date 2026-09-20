use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processing::ExpectedPolarity;
use nmr::processing::{
    DelaySource, DensePipeline, DirectDelayMode, FourierExponentSign, FrequencyFrame,
    PhaseCorrection, ProcessingError, ProcessingOptions, Projection,
};
use nmr::raw::{
    ComponentEvidence, DirectSamples, GroupDelayState, IndirectComponents,
    LinearComponentTransform, PendingGroupDelay, PeriodicLaneModulation, RawAxis, RawAxisKind,
    ResolvedComponentTransform,
};
use std::f64::consts::PI;

use super::support::*;

#[test]
fn dense_pipeline_uses_fixed_one_hz_window_double_power_of_two_fill_and_projection() {
    let raw = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            4,
            0.0,
            0.25,
        )],
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
        ],
    );
    let pipeline = DensePipeline::new(
        vec![Some(PhaseCorrection::new(0.0, 0.0, 0.0).unwrap())]
            .into_iter()
            .zip(vec![Some(FrequencyFrame::Hertz)])
            .enumerate()
            .map(
                |(index, (phase, frequency_frame))| nmr::processing::DenseAxisConfig {
                    axis: nmr::AxisIndex::new(index),
                    phase,
                    frequency_frame,
                },
            )
            .collect(),
        nmr::processing::DensePipelineOptions {
            direct_delay: DirectDelayMode::NoDelay,
            projection: Projection::RealSigned,
            expected_polarity: ExpectedPolarity::Signed,
            descending_ppm: false,
        },
    )
    .unwrap();
    let plot = pipeline.process(&raw).unwrap();
    assert_eq!(plot.shape(), [8]);
    assert_eq!(plot.data(), [1.0; 8]);
    assert_eq!(plot.provenance().history().unwrap().records().len(), 5);
}

#[test]
fn dense_pipeline_selects_component_steps_from_indirect_semantics() {
    for components in [
        IndirectComponents::Scalar,
        IndirectComponents::Cartesian(ComponentEvidence::user_constructed()),
        IndirectComponents::Encoded(
            ResolvedComponentTransform::user_constructed(
                LinearComponentTransform::try_new(
                    2,
                    vec![
                        Complex64::new(2.0, 0.0),
                        Complex64::default(),
                        Complex64::default(),
                        Complex64::new(3.0, 0.0),
                    ],
                    PeriodicLaneModulation::identity(2).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        ),
    ] {
        let encoded = matches!(components, IndirectComponents::Encoded(_));
        let kind = RawAxisKind::Indirect(components);
        let mut samples = vec![Complex64::default(); 4 * kind.lane_count()];
        // An impulse in the first lane gives a constant spectrum; the encoded
        // transform scales that lane by two, independently of either FFT.
        samples[0] = Complex64::new(1.0, 0.0);
        let indirect = RawAxis::new(
            kind,
            AxisDomain::Time,
            Some(AxisUnit::Second),
            2,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.5,
            },
        )
        .unwrap();
        let raw = raw_dataset(
            vec![
                indirect,
                direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.25),
            ],
            samples,
        );
        let result = DensePipeline::new(
            vec![None; 2]
                .into_iter()
                .zip(vec![Some(FrequencyFrame::Hertz); 2])
                .enumerate()
                .map(
                    |(index, (phase, frequency_frame))| nmr::processing::DenseAxisConfig {
                        axis: nmr::AxisIndex::new(index),
                        phase,
                        frequency_frame,
                    },
                )
                .collect(),
            nmr::processing::DensePipelineOptions {
                direct_delay: DirectDelayMode::NoDelay,
                projection: Projection::Magnitude,
                expected_polarity: ExpectedPolarity::Signed,
                descending_ppm: false,
            },
        )
        .unwrap()
        .process_dataset(&raw)
        .unwrap();
        assert_eq!(result.data().shape(), [4, 4]);
        assert_eq!(result.data().samples().len(), 16);
        for &sample in result.data().samples() {
            close(sample, if encoded { 2.0 } else { 1.0 });
        }
        let history = result.provenance().history().unwrap();
        let transforms = history.records().iter().filter(|record| matches!(record,
            nmr::processing::ProcessingRecord::Applied { resolved, .. }
                if matches!(resolved.as_ref(), nmr::processing::ResolvedOperation::ComponentTransform { .. })
        )).count();
        assert_eq!(transforms, usize::from(encoded));
        let replay = history
            .replay_raw(&raw, ProcessingOptions::default())
            .unwrap();
        assert_eq!(replay.data().samples(), result.data().samples());
    }
}

#[test]
fn dense_pipeline_delay_modes_have_analytic_samples_and_explicit_identity_policy() {
    for evidence in [
        GroupDelayState::Unknown,
        GroupDelayState::NotApplicable,
        GroupDelayState::Pending(PendingGroupDelay::user_constructed(0.0).unwrap()),
        GroupDelayState::Pending(PendingGroupDelay::user_constructed(0.75).unwrap()),
    ] {
        for mode in [
            DirectDelayMode::Automatic,
            DirectDelayMode::NoDelay,
            DirectDelayMode::FrequencyDomainPhaseRampV1(DelaySource::AxisEvidence),
            DirectDelayMode::FrequencyDomainPhaseRampV1(DelaySource::Explicit(0.0)),
            DirectDelayMode::FrequencyDomainPhaseRampV1(DelaySource::Explicit(0.75)),
        ] {
            let axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.25)
                .with_group_delay(evidence.clone())
                .unwrap();
            let raw = raw_dataset(
                vec![axis],
                vec![Complex64::new(1.0, 0.0), Complex64::default()],
            );
            let pipeline = DensePipeline::new(
                vec![Some(PhaseCorrection::new(0.0, 0.0, 0.0).unwrap())]
                    .into_iter()
                    .zip(vec![Some(FrequencyFrame::Hertz)])
                    .enumerate()
                    .map(
                        |(index, (phase, frequency_frame))| nmr::processing::DenseAxisConfig {
                            axis: nmr::AxisIndex::new(index),
                            phase,
                            frequency_frame,
                        },
                    )
                    .collect(),
                nmr::processing::DensePipelineOptions {
                    direct_delay: mode,
                    projection: Projection::RealSigned,
                    expected_polarity: ExpectedPolarity::Signed,
                    descending_ppm: false,
                },
            )
            .unwrap();
            let known = match &evidence {
                GroupDelayState::Pending(d) => Some(d.delay_points()),
                _ => None,
            };
            let delay = match mode {
                DirectDelayMode::Automatic => known.unwrap_or(0.0),
                DirectDelayMode::FrequencyDomainPhaseRampV1(DelaySource::AxisEvidence) => {
                    known.unwrap_or(0.0)
                }
                DirectDelayMode::FrequencyDomainPhaseRampV1(DelaySource::Explicit(d)) => d,
                _ => 0.0,
            };
            let result = pipeline.process_dataset(&raw);
            if mode == DirectDelayMode::Automatic && evidence == GroupDelayState::Unknown {
                assert!(matches!(
                    result.map_err(ProcessingError::into_root_cause),
                    Err(ProcessingError::MissingCapability {
                        capability: "direct group-delay evidence",
                        axis: Some(0)
                    })
                ));
                continue;
            }
            if let DirectDelayMode::FrequencyDomainPhaseRampV1(source) = mode {
                if source == DelaySource::AxisEvidence && known.is_none() {
                    assert!(matches!(
                        result.map_err(ProcessingError::into_root_cause),
                        Err(ProcessingError::MissingCapability {
                            capability: "group-delay evidence",
                            axis: Some(0)
                        })
                    ));
                    continue;
                }
                if matches!(source, DelaySource::Explicit(d) if known.is_some_and(|k| k != d)) {
                    assert!(matches!(
                        result.map_err(ProcessingError::into_root_cause),
                        Err(ProcessingError::DelayEvidenceMismatch)
                    ));
                    continue;
                }
                if delay == 0.0 {
                    assert!(matches!(
                        result.map_err(ProcessingError::into_root_cause),
                        Err(ProcessingError::InvalidState { .. })
                    ));
                    continue;
                }
            }
            let result = result.unwrap();
            assert_eq!(result.data().shape(), [4]);
            assert_eq!(result.data().samples().len(), 4);
            for (index, &sample) in result.data().samples().iter().enumerate() {
                close(
                    sample,
                    (2.0 * PI * delay * (index as f64 - 2.0) / 4.0).cos(),
                );
            }
            let history = result.provenance().history().unwrap();
            assert_eq!(
                history
                    .records()
                    .iter()
                    .filter(|r| r.algorithm_version() == Some("frequency-domain-phase-ramp.v1"))
                    .count(),
                usize::from(delay != 0.0)
            );
            assert_eq!(history.records().iter().filter(|record| record.algorithm_version() == Some("acknowledge-zero-delay.v1")).count(), usize::from(mode == DirectDelayMode::Automatic && known == Some(0.0)));
            let replay = history
                .replay_raw(&raw, ProcessingOptions::default())
                .unwrap();
            assert_eq!(replay.data().samples(), result.data().samples());
            assert_eq!(raw.descriptor().axes()[0].group_delay(), &evidence);
        }
    }
}

#[test]
fn dense_pipeline_automatic_delay_requires_evidence_and_resolves_phase_ramp_v1() {
    let pipeline = DensePipeline::new(
        vec![Some(PhaseCorrection::new(0.0, 0.0, 0.0).unwrap())]
            .into_iter()
            .zip(vec![Some(FrequencyFrame::Hertz)])
            .enumerate()
            .map(
                |(index, (phase, frequency_frame))| nmr::processing::DenseAxisConfig {
                    axis: nmr::AxisIndex::new(index),
                    phase,
                    frequency_frame,
                },
            )
            .collect(),
        nmr::processing::DensePipelineOptions {
            direct_delay: DirectDelayMode::Automatic,
            projection: Projection::RealSigned,
            expected_polarity: ExpectedPolarity::Signed,
            descending_ppm: false,
        },
    )
    .unwrap();
    let unknown_axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 4, 0.0, 0.25)
        .with_group_delay(GroupDelayState::Unknown)
        .unwrap();
    let unknown = raw_dataset(vec![unknown_axis], vec![Complex64::new(1.0, 0.0); 4]);
    assert!(matches!(
        pipeline
            .process_dataset(&unknown)
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::MissingCapability {
            capability: "direct group-delay evidence",
            axis: Some(0)
        }
    ));

    let pending_axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 4, 0.0, 0.25)
        .with_group_delay(GroupDelayState::Pending(
            PendingGroupDelay::user_constructed(0.75).unwrap(),
        ))
        .unwrap();
    let pending = raw_dataset(vec![pending_axis], vec![Complex64::new(1.0, 0.0); 4]);
    let processed = pipeline.process_dataset(&pending).unwrap();
    let ramp = processed
        .provenance()
        .history()
        .unwrap()
        .records()
        .iter()
        .find(|record| record.algorithm_version() == Some("frequency-domain-phase-ramp.v1"))
        .expect("automatic pending delay must produce a phase-ramp record");
    let nmr::processing::ProcessingRecord::Applied { resolved, .. } = ramp else {
        panic!("a resolved delay record must be applied");
    };
    assert!(matches!(
        resolved.as_ref(),
        nmr::processing::ResolvedOperation::FrequencyDomainPhaseRampV1 {
            delay: 0.75,
            sign: FourierExponentSign::Negative,
            ..
        }
    ));
}
