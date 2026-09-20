//! Regression contracts for acquisition coordinates, tensor lanes, and neutral operations.
use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    DigitalFilterCorrection, FourierTransform, FrequencyFrame, PhaseCorrection, PolarityState,
    PositivePeaksV1, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan, Projection,
    Window, ZeroFill,
};
use nmr::raw::{
    AxisIndex, DirectSamples, IndirectComponents, LinearComponentTransform, ModulationIndexDomain,
    PeriodicLaneModulation, RawAxis, RawAxisKind, RawDataset, RawDatasetBuilder, RawMetadata,
    ResolvedComponentTransform,
};

fn direct(points: usize) -> RawAxis {
    RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        points,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0,
        },
    )
    .unwrap()
}

fn encoded(modulation_axis: usize, origin: [usize; 2]) -> RawDataset {
    encoded_in_domain(
        ModulationIndexDomain::AbsoluteGridCoordinate(AxisIndex::new(modulation_axis)),
        origin,
    )
}

fn encoded_in_domain(domain: ModulationIndexDomain, origin: [usize; 2]) -> RawDataset {
    let one = Complex64::new(1.0, 0.0);
    let zero = Complex64::new(0.0, 0.0);
    let modulation =
        PeriodicLaneModulation::try_new(2, 2, vec![one, one, -one, -one], domain, 0).unwrap();
    let transform = ResolvedComponentTransform::user_constructed(
        LinearComponentTransform::try_new(2, vec![one, zero, zero, one], modulation).unwrap(),
    )
    .unwrap();
    let indirect = RawAxis::new(
        RawAxisKind::Indirect(IndirectComponents::Encoded(transform)),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0,
        },
    )
    .unwrap();
    RawDatasetBuilder::new(vec![indirect, direct(1)], RawMetadata::default())
        .unwrap()
        .absolute_grid_origin(origin.to_vec())
        .unwrap()
        .dense(
            [1.0, 2.0, 3.0, 4.0]
                .map(|x| Complex64::new(x, 0.0))
                .to_vec(),
        )
        .unwrap()
}

fn plan(ops: Vec<Op>) -> ProcessingPlan {
    ProcessingPlan::new(ops).unwrap()
}

#[test]
fn modulation_uses_the_origin_of_the_referenced_axis() {
    let raw = encoded(1, [0, 1]);
    let result = plan(vec![Op::ComponentTransform { axis: 0 }])
        .apply_raw(&raw)
        .unwrap();
    // The absolute direct coordinate is 1, so both lanes get multiplier -1.
    assert_eq!(result.data().get(&[0, 0], &[0, 0]).unwrap(), -1.0);
}

#[test]
fn pending_coordinate_modulation_is_rejected_before_reversal() {
    let raw = encoded(0, [0, 0]);
    let canonical = plan(vec![
        Op::ComponentTransform { axis: 0 },
        Op::ReverseAxis { axis: 0 },
    ])
    .apply_raw(&raw)
    .unwrap();
    assert_eq!(
        canonical.data().samples(),
        &[-3.0, 0.0, -4.0, 0.0, 1.0, 0.0, 2.0, 0.0]
    );
    let input = nmr::Dataset::from_raw(raw);
    assert!(
        plan(vec![
            Op::ReverseAxis { axis: 0 },
            Op::ComponentTransform { axis: 0 },
        ])
        .preflight(&input, ProcessingOptions::new())
        .is_err()
    );
}

#[test]
fn acquisition_ordinal_modulation_is_also_decoded_before_reversal() {
    let raw = encoded_in_domain(ModulationIndexDomain::ObservationOrdinal, [0, 0]);
    let input = nmr::Dataset::from_raw(raw);
    assert!(
        plan(vec![Op::ReverseAxis { axis: 0 }])
            .preflight(&input, ProcessingOptions::new())
            .is_err()
    );
    assert!(
        plan(vec![
            Op::ComponentTransform { axis: 0 },
            Op::ReverseAxis { axis: 0 }
        ])
        .preflight(&input, ProcessingOptions::new())
        .is_ok()
    );
}

#[test]
fn pending_modulation_blocks_only_transforms_of_its_referenced_axis() {
    let operations = || {
        vec![
            Op::FourierTransform {
                axis: 1,
                transform: FourierTransform::default(),
            },
            Op::ComponentTransform { axis: 0 },
        ]
    };
    let raw = encoded(0, [0, 1]);
    let allowed = plan(operations()).apply_raw(&raw).unwrap();
    let decoded_first = plan(vec![
        Op::ComponentTransform { axis: 0 },
        Op::FourierTransform {
            axis: 1,
            transform: FourierTransform::default(),
        },
    ])
    .apply_raw(&raw)
    .unwrap();
    assert_eq!(allowed.data(), decoded_first.data());
    let referenced_direct = nmr::Dataset::from_raw(encoded(1, [0, 1]));
    assert!(
        plan(operations())
            .preflight(&referenced_direct, ProcessingOptions::new())
            .is_err()
    );
}

#[test]
fn preflight_rejects_a_modulation_axis_outside_the_descriptor() {
    let input = nmr::Dataset::from_raw(encoded(99, [0, 0]));
    assert!(
        plan(vec![Op::ComponentTransform { axis: 0 }])
            .preflight(&input, ProcessingOptions::new())
            .is_err()
    );
}

#[test]
fn nonzero_hertz_window_rejects_unverified_explicit_sampling_in_preflight() {
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        3,
        AxisCoordinates::Explicit(vec![0.0, 1.0, 3.0]),
    )
    .unwrap()
    .with_spectral_width_hz(Some(1.0))
    .unwrap();
    let raw = RawDatasetBuilder::new(vec![axis], RawMetadata::default())
        .unwrap()
        .dense(vec![Complex64::new(1.0, 0.0); 3])
        .unwrap();
    let input = nmr::Dataset::from_raw(raw);
    assert!(
        plan(vec![Op::Window {
            axis: 0,
            window: Window::exponential(1.0 / std::f64::consts::PI).unwrap(),
        }])
        .preflight(&input, ProcessingOptions::new())
        .is_err()
    );
}

fn spectrum(unit: AxisUnit, step: f64, basis: ComponentBasis) -> ProcessedDataset {
    let count = basis.component_count();
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(unit),
        3,
        AxisCoordinates::Uniform { start: 10.0, step },
        basis,
    )
    .unwrap();
    ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![3], vec![count], vec![1.0; 3 * count]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
}

#[test]
fn baseline_preserves_every_independent_cartesian_component_trace() {
    let points = 32;
    let trace: Vec<f64> = (0..points)
        .map(|i| {
            let x = (i as f64 - 15.0) / 2.0;
            1.0 + 8.0 * (-x * x).exp()
        })
        .collect();
    let coordinates: Vec<f64> = (0..points).map(|i| i as f64).collect();
    let expected = PositivePeaksV1.subtract(&coordinates, &trace).unwrap();
    assert!(expected.iter().any(|x| x.abs() > 0.1));
    let axes = vec![
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
        ProcessedAxis::new(
            AxisRole::Signal,
            AxisDomain::Frequency,
            Some(AxisUnit::Hertz),
            points,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 1.0,
            },
            ComponentBasis::Scalar,
        )
        .unwrap(),
    ];
    let input = ProcessedDataset::new(
        ProcessedDescriptor::new(axes).unwrap(),
        ProcessedData::new(vec![2, points], vec![2, 1], trace.repeat(4)).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let output = plan(vec![Op::BaselineCorrection {
        axis: 1,
        profile: PositivePeaksV1.into(),
    }])
    .apply_processed(&input)
    .unwrap();
    for logical in 0..2 {
        for component in 0..2 {
            for (point, expected) in expected.iter().enumerate() {
                let actual = output
                    .data()
                    .get(&[logical, point], &[component, 0])
                    .unwrap();
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "logical={logical}, component={component}, point={point}, actual={actual}, expected={expected}"
                );
            }
        }
    }
}

#[test]
fn zero_broadening_and_equal_length_zero_fill_are_recorded_identities() {
    for coordinates in [
        AxisCoordinates::Unknown,
        AxisCoordinates::Explicit(vec![0.0, 1.0, 3.0]),
    ] {
        let axis = RawAxis::new(
            RawAxisKind::Direct(DirectSamples::Complex),
            AxisDomain::Time,
            Some(AxisUnit::Second),
            3,
            coordinates,
        )
        .unwrap();
        let values = vec![
            Complex64::new(1.0, -2.0),
            Complex64::new(3.0, 4.0),
            Complex64::new(5.0, -6.0),
        ];
        let raw = RawDatasetBuilder::new(vec![axis], RawMetadata::default())
            .unwrap()
            .dense(values)
            .unwrap();
        let output = plan(vec![
            Op::Window {
                axis: 0,
                window: Window::exponential(0.0).unwrap(),
            },
            Op::ZeroFill {
                axis: 0,
                zero_fill: ZeroFill::new(3).unwrap(),
            },
        ])
        .apply_raw(&raw)
        .unwrap();
        assert_eq!(output.data().samples(), &[1.0, -2.0, 3.0, 4.0, 5.0, -6.0]);
        assert_eq!(output.provenance().history().unwrap().records().len(), 2);
        assert!(
            plan(vec![Op::DigitalFilterCorrection {
                axis: 0,
                correction: DigitalFilterCorrection::AcknowledgeZeroDelayV1,
            }])
            .apply_processed(&output)
            .is_err()
        );
    }
}

#[test]
fn hertz_identity_preserves_calibration_and_is_recorded() {
    let input = spectrum(AxisUnit::Hertz, -0.1, ComponentBasis::Cartesian);
    let output = plan(vec![Op::ResolveFrequencyFrame {
        axis: 0,
        frame: FrequencyFrame::Hertz,
    }])
    .apply_processed(&input)
    .unwrap();
    assert_eq!(output.descriptor(), input.descriptor());
    assert_eq!(output.data(), input.data());
    assert_eq!(output.provenance().history().unwrap().records().len(), 1);
}

#[test]
fn neutral_real_projection_needs_no_phase_claim_and_retains_request() {
    let input = spectrum(AxisUnit::Ppm, -0.1, ComponentBasis::Cartesian);
    let request = Op::Projection {
        projection: Projection::Real,
        polarity: PolarityState::Ambiguous180,
    };
    let output = plan(vec![request.clone()]).apply_processed(&input).unwrap();
    assert_eq!(output.data().samples(), &[1.0; 3]);
    assert_eq!(output.descriptor().component_counts(), [1]);
    let history = output.provenance().history().unwrap();
    assert_eq!(history.records().len(), 1);
    assert_eq!(history.records()[0].requested().explicit(), Some(&request));
    assert_eq!(
        history
            .replay_processed(&input, ProcessingOptions::new())
            .unwrap()
            .data(),
        output.data()
    );
    assert!(
        plan(vec![Op::Projection {
            projection: Projection::RealSigned,
            polarity: PolarityState::Ambiguous180
        }])
        .apply_processed(&input)
        .is_err()
    );
    assert!(
        plan(vec![Op::Projection {
            projection: Projection::Real,
            polarity: PolarityState::UserAssertedPositive
        }])
        .apply_processed(&input)
        .is_err()
    );
}

#[test]
fn phase_rotation_is_equivalent_after_reversing_its_index_convention() {
    let input = spectrum(AxisUnit::Ppm, -0.1, ComponentBasis::Cartesian);
    let (p0, p1, pivot) = (27.0, -93.0, 0.25);
    let n = 3.0;
    let first = plan(vec![
        Op::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(p0, p1, pivot).unwrap(),
        },
        Op::ReverseAxis { axis: 0 },
    ])
    .apply_processed(&input)
    .unwrap();
    let reversed = plan(vec![
        Op::ReverseAxis { axis: 0 },
        Op::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(p0 + p1 * ((n - 1.0) / n - 2.0 * pivot), -p1, pivot)
                .unwrap(),
        },
    ])
    .apply_processed(&input)
    .unwrap();
    for (actual, expected) in reversed.data().samples().iter().zip(first.data().samples()) {
        assert!((actual - expected).abs() < 1e-12);
    }
}
