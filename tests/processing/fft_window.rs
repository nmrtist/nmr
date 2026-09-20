use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    FourierExponentSign, ProcessingError, ProcessingInput, ProcessingOperation, ProcessingOptions,
    ProcessingPlan, Window,
};
use nmr::raw::{DirectSamples, RawAxis, RawAxisKind};
use std::f64::consts::PI;

use super::support::*;

#[test]
fn fft_both_signs_match_direct_dft_with_centering_and_nonzero_t0() {
    let input = vec![
        Complex64::new(1.0, 0.0),
        Complex64::new(2.0, 1.0),
        Complex64::new(-1.0, 0.5),
    ];
    for sign in [FourierExponentSign::Negative, FourierExponentSign::Positive] {
        let axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 3, 0.125, 0.25)
            .with_spectral_width_hz(Some(4.0))
            .unwrap();
        let raw = raw_dataset(vec![axis], input.clone());
        let output = ProcessingPlan::new(vec![fft_operation(0, sign)])
            .unwrap()
            .apply_raw(&raw)
            .unwrap();
        let ordinary = dft(&input, sign);
        let sigma = if sign == FourierExponentSign::Negative {
            -1.0
        } else {
            1.0
        };
        for point in 0..3 {
            let q = point as isize - 1;
            let source = if q < 0 { (3 + q) as usize } else { q as usize };
            let frequency = q as f64 * 4.0 / 3.0;
            let angle = sigma * 2.0 * PI * frequency * 0.125;
            let expected = ordinary[source] * Complex64::new(angle.cos(), angle.sin());
            close_complex(
                Complex64::new(
                    output.data().get(&[point], &[0]).unwrap(),
                    output.data().get(&[point], &[1]).unwrap(),
                ),
                expected,
            );
        }
        assert_eq!(
            output.descriptor().axes()[0].domain(),
            AxisDomain::Frequency
        );
        assert_eq!(
            output.descriptor().axes()[0].coordinates(),
            &AxisCoordinates::Uniform {
                start: -4.0 / 3.0,
                step: 4.0 / 3.0,
            }
        );
        assert_eq!(output.descriptor().axes()[0].spectral_width_hz(), Some(4.0));
        close(
            output.descriptor().axes()[0].coordinate_span().unwrap(),
            8.0 / 3.0,
        );
    }
}

#[test]
fn scalar_fft_creates_cartesian_components_and_even_nyquist_is_leftmost() {
    let axis = direct_axis(AxisDomain::Time, DirectSamples::Real, 4, 0.0, 0.5);
    let raw = raw_dataset(
        vec![axis],
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(-1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(-1.0, 0.0),
        ],
    );
    let output = ProcessingPlan::new(vec![fft_operation(0, FourierExponentSign::Negative)])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    assert_eq!(
        output.descriptor().axes()[0].component_basis(),
        &ComponentBasis::Cartesian
    );
    assert_eq!(output.descriptor().axes()[0].component_count(), 2);
    close(output.data().get(&[0], &[0]).unwrap(), 4.0);
    for point in 1..4 {
        close(output.data().get(&[point], &[0]).unwrap(), 0.0);
        close(output.data().get(&[point], &[1]).unwrap(), 0.0);
    }
}

#[test]
fn sine_and_exponential_windows_share_weights_across_components() {
    let axis = direct_axis(AxisDomain::Time, DirectSamples::Complex, 3, 0.0, 0.25)
        .with_spectral_width_hz(Some(4.0))
        .unwrap();
    let raw = raw_dataset(vec![axis], vec![Complex64::new(2.0, 3.0); 3]);
    let output = ProcessingPlan::new(vec![
        ProcessingOperation::Window {
            axis: 0,
            window: Window::sine_bell(0.5, 0.5, 2.0, 0.5).unwrap(),
        },
        ProcessingOperation::Window {
            axis: 0,
            window: Window::exponential(1.0).unwrap(),
        },
    ])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    for point in 0..3 {
        let sine = if point == 0 { 0.5 } else { 1.0 };
        let exponential = (-PI * point as f64 / 4.0).exp();
        close(
            output.data().get(&[point], &[0]).unwrap(),
            2.0 * sine * exponential,
        );
        close(
            output.data().get(&[point], &[1]).unwrap(),
            3.0 * sine * exponential,
        );
    }
}

#[test]
fn fft_and_zero_fill_reject_unverified_or_explicit_coordinates() {
    let unknown = raw_dataset(
        vec![
            RawAxis::new(
                RawAxisKind::Direct(DirectSamples::Complex),
                AxisDomain::Time,
                Some(AxisUnit::Second),
                2,
                AxisCoordinates::Unknown,
            )
            .unwrap(),
        ],
        vec![Complex64::new(1.0, 0.0); 2],
    );
    assert!(
        ProcessingPlan::new(vec![fft_operation(0, FourierExponentSign::Negative)])
            .unwrap()
            .apply_raw(&unknown)
            .is_err()
    );

    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Explicit(vec![0.0, 0.5]),
        ComponentBasis::Scalar,
    )
    .unwrap();
    let descriptor = ProcessedDescriptor::new(vec![axis]).unwrap();
    let imported = ProcessedDataset::new(
        descriptor,
        ProcessedData::new(vec![2], vec![1], vec![1.0, 2.0]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap();
    assert!(
        ProcessingPlan::new(vec![zero_fill_operation(0, 3)])
            .unwrap()
            .apply_processed(&imported)
            .is_err()
    );
}

#[test]
fn window_variants_and_constructors_share_parameter_validation_in_preflight() {
    let raw = raw_dataset(
        vec![direct_axis(
            AxisDomain::Time,
            DirectSamples::Complex,
            3,
            0.0,
            0.25,
        )],
        vec![Complex64::new(2.0, 3.0); 3],
    );
    let input = ProcessingInput::from_dataset(&raw).unwrap();
    let mut cases = vec![(
        Window::SineBell {
            offset: 0.5,
            end: 0.5,
            power: 2.0,
            first_point_scale: -1.0,
        },
        Window::sine_bell(0.5, 0.5, 2.0, -1.0),
        false,
        "sine-bell window",
    )];
    for field in 0..4 {
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            0.0,
            0.5,
            1.0,
            2.0,
        ] {
            let mut params = [0.25, 0.75, 2.0, 0.5];
            params[field] = value;
            let [offset, end, power, first_point_scale] = params;
            let valid = value.is_finite()
                && match field {
                    0 | 1 => (0.0..=1.0).contains(&value),
                    2 => value > 0.0,
                    _ => value >= 0.0,
                };
            cases.push((
                Window::SineBell {
                    offset,
                    end,
                    power,
                    first_point_scale,
                },
                Window::sine_bell(offset, end, power, first_point_scale),
                valid,
                "sine-bell window",
            ));
        }
    }
    for lb_hz in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0, 1.0] {
        cases.push((
            Window::Exponential { lb_hz },
            Window::exponential(lb_hz),
            lb_hz.is_finite(),
            "exponential line broadening",
        ));
    }
    for (window, constructed, valid, parameter) in cases {
        let plan = ProcessingPlan::new(vec![ProcessingOperation::Window {
            axis: 0,
            window: window.clone(),
        }])
        .unwrap();
        let prepared = plan.preflight_raw(&input, ProcessingOptions::default());
        if !valid {
            assert_eq!(
                constructed.unwrap_err().into_root_cause(),
                ProcessingError::InvalidParameter(parameter)
            );
            assert_eq!(
                prepared.unwrap_err().into_root_cause(),
                ProcessingError::InvalidParameter(parameter)
            );
            continue;
        }
        assert_eq!(constructed.unwrap(), window);
        let result = prepared.unwrap().apply(&raw).unwrap();
        assert_eq!(result.data().shape(), [3]);
        assert_eq!(result.data().samples().len(), 6);
        for point in 0..3 {
            let weight = match window {
                Window::SineBell {
                    offset,
                    end,
                    power,
                    first_point_scale,
                } => {
                    let sine = (PI * (offset + (end - offset) * point as f64 / 2.0))
                        .sin()
                        .powf(power);
                    sine * if point == 0 { first_point_scale } else { 1.0 }
                }
                Window::Exponential { lb_hz } => (-PI * lb_hz * point as f64 / 4.0).exp(),
                _ => unreachable!(),
            };
            close(result.data().get(&[point], &[0]).unwrap(), 2.0 * weight);
            close(result.data().get(&[point], &[1]).unwrap(), 3.0 * weight);
        }
    }
}
