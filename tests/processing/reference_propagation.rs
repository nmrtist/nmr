use crate::inputs;
use nmr::processing::{ProcessingOperation as Op, ProcessingPlan, SpectrumOperation as S};
use nmr::snapshot;

fn restored(input: &nmr::Dataset) -> nmr::Dataset {
    let mut bytes = Vec::new();
    snapshot::write_snapshot(input, &mut bytes, Default::default()).unwrap();
    snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(snapshot::AcceptRecordedHistory)
}

#[test]
fn processed_sf_remains_reference_through_binning_combination_and_offline_restore() {
    let dir = tempfile::tempdir().unwrap();
    inputs::bruker_processed(dir.path()).unwrap();
    let input = nmr::read(dir.path().join("1r")).unwrap();
    let plan = ProcessingPlan::new(vec![Op::Spectrum {
        axis: 0,
        operation: S::Bin {
            width: 5.0,
            aggregation: nmr::processing::BinAggregation::Sum,
        },
    }])
    .unwrap();
    let binned = plan.apply(&input).unwrap();
    let combined = nmr::processing::LinearCombination::new(1.0)
        .unwrap()
        .prepare(&binned, &binned, Default::default())
        .unwrap()
        .execute()
        .unwrap();
    for value in [&input, &binned, &combined, &restored(&combined)] {
        let processed = value.as_processed().unwrap();
        let evidence = processed.axis_evidence(0).unwrap();
        assert_eq!(evidence.reference_frequency_mhz(), Some(400.0));
        assert_eq!(evidence.observe_frequency_mhz(), None);
        assert_eq!(evidence.chemical_shift_reference(), None);
        assert!(
            matches!(evidence.reference_evidence().unwrap().authority(), nmr::raw::ResolutionAuthority::FormatRule(rule) if rule.as_str() == "bruker.processed-sf.v1")
        );
        assert_eq!(
            evidence.group_delay(),
            nmr::processed::ProcessedGroupDelay::Unknown
        );
        assert!(processed.axis_evidence(1).is_none());
    }
    assert_eq!(
        binned.as_processed().unwrap().descriptor().axes()[0]
            .coordinate_iter()
            .unwrap()
            .collect::<Vec<_>>(),
        [8.75, 3.75]
    );
    let text = std::fs::read_to_string(dir.path().join("procs")).unwrap();
    std::fs::write(
        dir.path().join("procs"),
        text.lines()
            .filter(|l| !l.starts_with("##$SF="))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let unknown = nmr::read(dir.path().join("1r")).unwrap();
    assert_eq!(
        unknown
            .as_processed()
            .unwrap()
            .axis_evidence(0)
            .unwrap()
            .reference_frequency_mhz(),
        None
    );
    assert!(unknown.warnings().iter().any(|w| matches!(
        w,
        nmr::ReadWarning::MissingMetadata {
            field: nmr::MetadataField::ReferenceFrequency,
            ..
        }
    )));
}

fn calibrated_raw(indices: Option<&[usize]>) -> nmr::Dataset {
    use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit, FrequencyEvidence};
    use nmr::raw::*;
    let axis = |kind, observe, reference| {
        RawAxis::new(
            kind,
            AxisDomain::Time,
            Some(AxisUnit::Second),
            8,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.001,
            },
        )
        .unwrap()
        .with_frequency_evidence(Some(FrequencyEvidence::new(Some(observe), None).unwrap()))
        .unwrap()
        .with_chemical_shift_reference(Some(
            ChemicalShiftReference::user_constructed(4.7, reference).unwrap(),
        ))
        .unwrap()
    };
    let indirect = axis(
        RawAxisKind::Indirect(IndirectComponents::Cartesian(
            ComponentEvidence::user_constructed(),
        )),
        125.002,
        125.0,
    );
    let direct = axis(RawAxisKind::Direct(DirectSamples::Complex), 500.005, 500.0)
        .with_group_delay(GroupDelayState::Pending(
            PendingGroupDelay::user_constructed(0.0).unwrap(),
        ))
        .unwrap();
    let builder = RawDatasetBuilder::new(vec![indirect, direct], RawMetadata::default()).unwrap();
    let row = |i: usize| {
        (0..2)
            .flat_map(move |lane| {
                (0..8).map(move |j| {
                    let p = std::f64::consts::TAU * i as f64 / 8.0;
                    nmr::Complex64::from_polar(
                        if lane == 0 { p.cos() } else { p.sin() },
                        std::f64::consts::TAU * j as f64 / 8.0,
                    )
                })
            })
            .collect::<Vec<_>>()
    };
    if let Some(indices) = indices {
        let coordinates = indices
            .iter()
            .map(|&i| SamplingCoordinate::new(vec![i]))
            .collect::<Vec<_>>();
        let traces = indices
            .iter()
            .enumerate()
            .map(|(ordinal, &i)| {
                SparseTrace::new(
                    ObservationOrdinal::new(ordinal),
                    coordinates[ordinal].clone(),
                    row(i),
                )
            })
            .collect();
        builder
            .sparse(traces, SamplingSchedule::new(vec![8], coordinates).unwrap())
            .unwrap()
            .into()
    } else {
        builder
            .dense((0..8).flat_map(row).collect())
            .unwrap()
            .into()
    }
}

fn direct_plan() -> ProcessingPlan {
    use nmr::processing::*;
    ProcessingPlan::new(vec![
        Op::DigitalFilterCorrection {
            axis: 1,
            correction: DigitalFilterCorrection::AcknowledgeZeroDelayV1,
        },
        Op::FourierTransform {
            axis: 1,
            transform: FourierTransform::default(),
        },
        // Explicit reference must replace the effective state, as well as appear in history.
        Op::ResolveFrequencyFrame {
            axis: 1,
            frame: FrequencyFrame::Ppm(ReferenceSource::Explicit(
                nmr::raw::ChemicalShiftReference::user_constructed(5.0, 500.0).unwrap(),
            )),
        },
    ])
    .unwrap()
}

#[test]
fn effective_references_follow_fft_bin_column_slice_binary_and_nus() {
    use nmr::processing::*;
    let dense = calibrated_raw(None);
    let sparse = calibrated_raw(Some(&[7, 0, 1, 3, 4, 6]));
    let transformed = direct_plan().apply(&dense).unwrap();
    let nus = NusSettings {
        max_iterations: 1000,
        noise_standard_deviation: Some(0.0),
    }
    .prepare(&sparse, direct_plan(), Default::default())
    .unwrap()
    .execute()
    .unwrap();
    for input in [&transformed, &nus] {
        let direct = input.as_processed().unwrap().axis_evidence(1).unwrap();
        assert_eq!(direct.reference_frequency_mhz(), Some(500.0));
        assert_eq!(direct.observe_frequency_mhz(), Some(500.005));
        assert_eq!(
            direct.chemical_shift_reference().unwrap().carrier_ppm(),
            5.0
        );
        assert!(matches!(
            direct.group_delay(),
            nmr::processed::ProcessedGroupDelay::Corrected {
                delay_points: 0.0,
                ..
            }
        ));
        let frequency = ProcessingPlan::new(vec![
            Op::FourierTransform {
                axis: 0,
                transform: FourierTransform::default(),
            },
            Op::ResolveFrequencyFrame {
                axis: 0,
                frame: FrequencyFrame::Ppm(ReferenceSource::AxisEvidence),
            },
            Op::Spectrum {
                axis: 1,
                operation: S::Bin {
                    width: 0.5,
                    aggregation: BinAggregation::Sum,
                },
            },
        ])
        .unwrap()
        .apply(input)
        .unwrap();
        let row = ProcessingPlan::new(vec![Op::Spectrum {
            axis: 0,
            operation: S::Slice {
                index: 0,
                component: 0,
            },
        }])
        .unwrap()
        .apply(&frequency)
        .unwrap();
        let column = ProcessingPlan::new(vec![Op::Spectrum {
            axis: 1,
            operation: S::Slice {
                index: 0,
                component: 0,
            },
        }])
        .unwrap()
        .apply(&frequency)
        .unwrap();
        let combined = LinearCombination::new(0.5)
            .unwrap()
            .prepare(&row, &row, Default::default())
            .unwrap()
            .execute()
            .unwrap();
        for (value, reference, observe) in [
            (&row, 500.0, 500.005),
            (&column, 125.0, 125.002),
            (&combined, 500.0, 500.005),
        ] {
            for value in [value, &restored(value)] {
                let e = value.as_processed().unwrap().axis_evidence(0).unwrap();
                assert_eq!(e.reference_frequency_mhz(), Some(reference));
                assert_eq!(e.observe_frequency_mhz(), Some(observe));
            }
        }
        let nmr::processed::ProcessedOrigin::Library(boundary) =
            combined.as_processed().unwrap().provenance().origin()
        else {
            panic!("missing derivation")
        };
        let replay = boundary
            .replay(
                &[&row, &row],
                Default::default(),
                &mut nmr::ExecutionContext::default(),
            )
            .unwrap();
        assert_eq!(replay.canonical_digests(), combined.canonical_digests());
    }
}
