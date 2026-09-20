use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processed::ComponentBasis;
use nmr::processing::{
    FourierExponentSign, ProcessingError, ProcessingOperation, ProcessingOptions, ProcessingPlan,
};
use nmr::raw::{
    AxisIndex, ComponentEvidence, DirectSamples, IndirectComponents, ModulationIndexDomain,
    PeriodicLaneModulation, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata,
};
use std::f64::consts::PI;

use super::support::*;

#[test]
fn component_transform_budget_includes_gathered_lanes() {
    let indirect = encoded_axis(
        1,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        PeriodicLaneModulation::identity(2).unwrap(),
    );
    let raw = raw_dataset(
        vec![
            indirect,
            direct_axis(AxisDomain::Time, DirectSamples::Complex, 1, 0.0, 0.25),
        ],
        vec![Complex64::new(1.0, 2.0), Complex64::new(3.0, 4.0)],
    );
    let plan =
        ProcessingPlan::new(vec![ProcessingOperation::ComponentTransform { axis: 0 }]).unwrap();
    // Two complex current values, two output values, and two gathered lanes.
    let peak = 6 * std::mem::size_of::<Complex64>();
    assert!(matches!(
        plan.apply_raw_with_options(&raw, ProcessingOptions::new().max_working_bytes(peak - 1),)
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            nmr::resource::LimitExceeded {
                resource: nmr::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
    let output = plan
        .apply_raw_with_options(&raw, ProcessingOptions::new())
        .unwrap();
    assert_eq!(output.data().samples(), &[1.0, 2.0, 3.0, 4.0]);
    assert!(matches!(
        output
            .provenance()
            .history()
            .unwrap()
            .replay_raw(&raw, ProcessingOptions::new().max_working_bytes(peak - 1),)
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(
            nmr::resource::LimitExceeded {
                resource: nmr::resource::ResourceKind::WorkingBytes,
                ..
            }
        ))
    ));
}

#[test]
fn raw_expansion_preserves_rank_two_hypercomplex_component_order() {
    let indirect = RawAxis::new(
        RawAxisKind::Indirect(IndirectComponents::Cartesian(
            ComponentEvidence::user_constructed(),
        )),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let direct = direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.25)
        .with_spectral_width_hz(Some(4.0))
        .unwrap();
    let samples = (0..8)
        .map(|value| Complex64::new(100.0 + value as f64, 200.0 + value as f64))
        .collect();
    let raw = raw_dataset(vec![indirect, direct], samples);
    let output = ProcessingPlan::new(vec![zero_fill_operation(1, 3)])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();

    assert_eq!(output.descriptor().component_counts(), [2, 2]);
    assert_eq!(output.data().get(&[1, 1], &[1, 0]).unwrap(), 107.0);
    assert_eq!(output.data().get(&[1, 1], &[1, 1]).unwrap(), 207.0);
    assert_eq!(output.data().get(&[1, 2], &[1, 0]).unwrap(), 0.0);
    assert_eq!(output.data().get(&[1, 2], &[1, 1]).unwrap(), 0.0);
}

#[test]
fn echo_anti_echo_recombination_has_fixed_public_component_order() {
    let indirect = encoded_axis(
        1,
        vec![
            Complex64::new(0.0, 1.0),
            Complex64::new(0.0, 1.0),
            Complex64::new(-1.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        PeriodicLaneModulation::identity(2).unwrap(),
    );
    let direct = direct_axis(AxisDomain::Time, DirectSamples::Complex, 2, 0.0, 0.25);
    let raw = raw_dataset(
        vec![indirect, direct],
        vec![
            Complex64::new(1.0, 2.0),
            Complex64::new(3.0, 4.0),
            Complex64::new(5.0, 6.0),
            Complex64::new(7.0, 8.0),
        ],
    );
    let output = ProcessingPlan::new(vec![
        fft_operation(1, FourierExponentSign::Negative),
        ProcessingOperation::ComponentTransform { axis: 0 },
    ])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    assert_eq!(
        output.descriptor().axes()[0].component_basis(),
        &ComponentBasis::Cartesian
    );
    // At the direct sum bin E=4+6i and A=12+14i.
    close(output.data().get(&[0, 1], &[0, 0]).unwrap(), -20.0);
    close(output.data().get(&[0, 1], &[0, 1]).unwrap(), 16.0);
    close(output.data().get(&[0, 1], &[1, 0]).unwrap(), 8.0);
    close(output.data().get(&[0, 1], &[1, 1]).unwrap(), 8.0);
}

#[test]
fn synthetic_two_dimensional_signal_freezes_signs_components_phase_and_images() {
    const N1: usize = 8;
    const N2: usize = 8;
    let peaks = [
        (
            1_isize,
            -2_isize,
            Complex64::from_polar(2.0, 30_f64.to_radians()),
            Complex64::from_polar(3.0, -20_f64.to_radians()),
        ),
        (
            -2_isize,
            1_isize,
            Complex64::from_polar(1.5, -35_f64.to_radians()),
            Complex64::from_polar(2.5, 25_f64.to_radians()),
        ),
    ];
    let mut samples = Vec::new();
    for t1 in 0..N1 {
        let mut lanes: [Vec<Complex64>; 2] =
            std::array::from_fn(|_| vec![Complex64::new(0.0, 0.0); N2]);
        for &(k1, k2, amplitude1, amplitude2) in &peaks {
            let phase1 = 2.0 * PI * k1 as f64 * t1 as f64 / N1 as f64;
            let indirect = amplitude1 * Complex64::from_polar(1.0, phase1);
            let [real_lane, imaginary_lane] = &mut lanes;
            for (t2, (real, imaginary)) in real_lane.iter_mut().zip(imaginary_lane).enumerate() {
                let phase2 = 2.0 * PI * k2 as f64 * t2 as f64 / N2 as f64;
                let direct = amplitude2 * Complex64::from_polar(1.0, phase2);
                *real += indirect.re * direct;
                *imaginary += indirect.im * direct;
            }
        }
        samples.extend_from_slice(&lanes[0]);
        samples.extend_from_slice(&lanes[1]);
    }
    let raw = raw_dataset(
        vec![
            encoded_axis(
                N1,
                vec![
                    Complex64::new(1.0, 0.0),
                    Complex64::new(0.0, 0.0),
                    Complex64::new(0.0, 0.0),
                    Complex64::new(1.0, 0.0),
                ],
                PeriodicLaneModulation::identity(2).unwrap(),
            ),
            direct_axis(AxisDomain::Time, DirectSamples::Complex, N2, 0.0, 0.5),
        ],
        samples,
    );
    let output = ProcessingPlan::new(vec![
        ProcessingOperation::ComponentTransform { axis: 0 },
        fft_operation(1, FourierExponentSign::Negative),
        fft_operation(0, FourierExponentSign::Negative),
    ])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    assert_eq!(output.descriptor().component_counts(), [2, 2]);

    let centered = |frequency: isize, points: usize| {
        (frequency.rem_euclid(points as isize) as usize + points / 2) % points
    };
    let mut target_l2 = 0.0;
    let mut image_l2 = 0.0;
    for &(k1, k2, amplitude1, amplitude2) in &peaks {
        let coordinate = [centered(k1, N1), centered(k2, N2)];
        let scale = (N1 * N2) as f64;
        let expected = [
            amplitude1.re * amplitude2.re * scale,
            amplitude1.re * amplitude2.im * scale,
            amplitude1.im * amplitude2.re * scale,
            amplitude1.im * amplitude2.im * scale,
        ];
        let actual = [
            output.data().get(&coordinate, &[0, 0]).unwrap(),
            output.data().get(&coordinate, &[0, 1]).unwrap(),
            output.data().get(&coordinate, &[1, 0]).unwrap(),
            output.data().get(&coordinate, &[1, 1]).unwrap(),
        ];
        for (&value, &reference) in actual.iter().zip(&expected) {
            assert!((value - reference).abs() <= 1e-9 * reference.abs().max(1.0));
            target_l2 += value * value;
        }
        close(Complex64::new(actual[0], actual[2]).arg(), amplitude1.arg());
        close(Complex64::new(actual[0], actual[1]).arg(), amplitude2.arg());

        let image = [centered(-k1, N1), centered(-k2, N2)];
        for components in [[0, 0], [0, 1], [1, 0], [1, 1]] {
            let value = output.data().get(&image, &components).unwrap();
            image_l2 += value * value;
        }
    }
    assert!(image_l2.sqrt() / target_l2.sqrt() < 1e-12);
}

#[test]
fn states_tppi_uses_original_logical_coordinate_parity() {
    let indirect = encoded_axis(
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        PeriodicLaneModulation::try_new(
            2,
            2,
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(-1.0, 0.0),
                Complex64::new(-1.0, 0.0),
            ],
            ModulationIndexDomain::AbsoluteGridCoordinate(AxisIndex::new(0)),
            0,
        )
        .unwrap(),
    );
    let direct = direct_axis(AxisDomain::Time, DirectSamples::Complex, 1, 0.0, 0.25);
    let raw = raw_dataset(
        vec![indirect, direct],
        vec![
            Complex64::new(1.0, 2.0),
            Complex64::new(3.0, 4.0),
            Complex64::new(5.0, 6.0),
            Complex64::new(7.0, 8.0),
        ],
    );
    let output = ProcessingPlan::new(vec![
        fft_operation(1, FourierExponentSign::Negative),
        ProcessingOperation::ComponentTransform { axis: 0 },
    ])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    close(output.data().get(&[0, 0], &[0, 0]).unwrap(), 1.0);
    close(output.data().get(&[1, 0], &[0, 0]).unwrap(), -5.0);
    close(output.data().get(&[1, 0], &[1, 1]).unwrap(), -8.0);
}

#[test]
fn states_tppi_crop_origin_does_not_reset_absolute_parity() {
    let indirect = encoded_axis(
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        PeriodicLaneModulation::try_new(
            2,
            2,
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(-1.0, 0.0),
                Complex64::new(-1.0, 0.0),
            ],
            ModulationIndexDomain::AbsoluteGridCoordinate(AxisIndex::new(0)),
            0,
        )
        .unwrap(),
    );
    let direct = direct_axis(AxisDomain::Time, DirectSamples::Complex, 1, 0.0, 0.25);
    let raw = RawDatasetBuilder::new(vec![indirect, direct], RawMetadata::default())
        .unwrap()
        .absolute_grid_origin(vec![3, 0])
        .unwrap()
        .dense(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0),
            Complex64::new(4.0, 0.0),
        ])
        .unwrap();
    let output = ProcessingPlan::new(vec![ProcessingOperation::ComponentTransform { axis: 0 }])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    close(output.data().get(&[0, 0], &[0, 0]).unwrap(), -1.0);
    close(output.data().get(&[0, 0], &[1, 0]).unwrap(), -2.0);
    close(output.data().get(&[1, 0], &[0, 0]).unwrap(), 3.0);
    close(output.data().get(&[1, 0], &[1, 0]).unwrap(), 4.0);
    assert_eq!(
        output.provenance().history().unwrap().records()[0].algorithm_version(),
        Some("linear-component-transform.v1")
    );
}
