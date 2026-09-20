//! Independent physical-coordinate oracles for the public processing contract.
//! Expectations use direct sums and separable analytic tones, never the FFT kernel.

use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processing::{
    FourierExponentSign, FourierTransform, FrequencyFrame, PhaseCorrection, ProcessingOperation,
    ProcessingPlan, ReferenceSource, Window, ZeroFill,
};
use nmr::raw::{
    ChemicalShiftReference, ComponentEvidence, DirectSamples, IndirectComponents, RawAxis,
    RawAxisKind, RawDatasetBuilder, RawMetadata,
};
use std::f64::consts::PI;

fn time_axis(kind: RawAxisKind, points: usize, start: f64, step: f64) -> RawAxis {
    RawAxis::new(
        kind,
        AxisDomain::Time,
        Some(AxisUnit::Second),
        points,
        AxisCoordinates::Uniform { start, step },
    )
    .unwrap()
    .with_spectral_width_hz(Some(1.0 / step))
    .unwrap()
}

fn fft(axis: usize) -> ProcessingOperation {
    ProcessingOperation::FourierTransform {
        axis,
        transform: FourierTransform::new(FourierExponentSign::Negative),
    }
}

fn coordinate(coordinates: &AxisCoordinates, point: usize) -> f64 {
    match coordinates {
        AxisCoordinates::Uniform { start, step } => start + point as f64 * step,
        AxisCoordinates::Explicit(values) => values[point],
        _ => panic!("the output must have physical coordinates"),
    }
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual:.16e}, expected={expected:.16e}, tolerance={tolerance:.3e}"
    );
}

#[test]
fn one_dimensional_physical_dft_survives_window_fill_phase_ppm_and_reverse() {
    const ACQUIRED: usize = 6;
    const TRANSFORMED: usize = 12;
    const START: f64 = 0.0625;
    const DWELL: f64 = 0.125;
    const LB_HZ: f64 = 0.8;
    const P0: f64 = 30.0;
    const P1: f64 = -50.0;
    const PIVOT: f64 = 0.37;
    let axis = time_axis(
        RawAxisKind::Direct(DirectSamples::Complex),
        ACQUIRED,
        START,
        DWELL,
    )
    .with_chemical_shift_reference(Some(
        ChemicalShiftReference::user_constructed(4.7, 400.0).unwrap(),
    ))
    .unwrap();
    let tones = [
        (1.0, Complex64::new(2.0, -0.5)),
        (-2.0, Complex64::new(-0.3, 1.0)),
    ];
    let input: Vec<_> = (0..ACQUIRED)
        .map(|n| {
            let time = START + n as f64 * DWELL;
            tones
                .iter()
                .map(|&(frequency, amplitude)| {
                    amplitude * Complex64::from_polar(1.0, 2.0 * PI * frequency * time)
                })
                .sum::<Complex64>()
        })
        .collect();
    let raw = RawDatasetBuilder::new(vec![axis], RawMetadata::default())
        .unwrap()
        .dense(input.clone())
        .unwrap();
    let result = ProcessingPlan::new(vec![
        ProcessingOperation::Window {
            axis: 0,
            window: Window::exponential(LB_HZ).unwrap(),
        },
        ProcessingOperation::ZeroFill {
            axis: 0,
            zero_fill: ZeroFill::new(TRANSFORMED).unwrap(),
        },
        fft(0),
        ProcessingOperation::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::new(P0, P1, PIVOT).unwrap(),
        },
        ProcessingOperation::ResolveFrequencyFrame {
            axis: 0,
            frame: FrequencyFrame::Ppm(ReferenceSource::AxisEvidence),
        },
        ProcessingOperation::ReverseAxis { axis: 0 },
    ])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    assert_eq!(result.descriptor().axes()[0].unit(), Some(AxisUnit::Ppm));
    for displayed in 0..TRANSFORMED {
        let before_reverse = TRANSFORMED - 1 - displayed;
        let q = before_reverse as isize - (TRANSFORMED / 2) as isize;
        let frequency = q as f64 / (TRANSFORMED as f64 * DWELL);
        // The physical Fourier sum independently includes t0 and the acquired
        // duration. Zero-filled samples contribute zero; there is no 1/N scale.
        let expected: Complex64 = input
            .iter()
            .enumerate()
            .map(|(n, &sample)| {
                let elapsed = n as f64 * DWELL;
                sample
                    * (-PI * LB_HZ * elapsed).exp()
                    * Complex64::from_polar(1.0, -2.0 * PI * frequency * (START + elapsed))
            })
            .sum();
        let phase = P0 + P1 * (before_reverse as f64 / TRANSFORMED as f64 - PIVOT);
        let expected = expected * Complex64::from_polar(1.0, phase.to_radians());
        close(
            result.data().get(&[displayed], &[0]).unwrap(),
            expected.re,
            2e-12,
        );
        close(
            result.data().get(&[displayed], &[1]).unwrap(),
            expected.im,
            2e-12,
        );
        close(
            coordinate(result.descriptor().axes()[0].coordinates(), displayed),
            4.7 + frequency / 400.0,
            2e-14,
        );
    }
}

#[test]
fn two_dimensional_tones_retain_physical_frequencies_phase_and_component_energy() {
    const N1: usize = 8;
    const N2: usize = 6;
    let axes = vec![
        time_axis(
            RawAxisKind::Indirect(IndirectComponents::Cartesian(
                ComponentEvidence::user_constructed(),
            )),
            N1,
            0.125,
            0.25,
        ),
        time_axis(RawAxisKind::Direct(DirectSamples::Complex), N2, 0.025, 0.1),
    ];
    let a1 = Complex64::from_polar(2.0, 0.3);
    let a2 = Complex64::from_polar(3.0, -0.4);
    let f1 = -1.0;
    let f2 = 1.0 / 0.6;
    let mut samples = Vec::new();
    for n1 in 0..N1 {
        let indirect = a1 * Complex64::from_polar(1.0, 2.0 * PI * f1 * (0.125 + n1 as f64 * 0.25));
        for lane in [indirect.re, indirect.im] {
            for n2 in 0..N2 {
                samples.push(
                    lane * a2
                        * Complex64::from_polar(1.0, 2.0 * PI * f2 * (0.025 + n2 as f64 * 0.1)),
                );
            }
        }
    }
    let raw = RawDatasetBuilder::new(axes, RawMetadata::default())
        .unwrap()
        .dense(samples)
        .unwrap();
    let result = ProcessingPlan::new(vec![fft(1), fft(0)])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
    let peak = [2, 4];
    close(
        coordinate(result.descriptor().axes()[0].coordinates(), peak[0]),
        f1,
        1e-15,
    );
    close(
        coordinate(result.descriptor().axes()[1].coordinates(), peak[1]),
        f2,
        1e-15,
    );
    let mut leakage_energy = 0.0;
    let mut peak_energy = 0.0;
    for n1 in 0..N1 {
        for n2 in 0..N2 {
            for c1 in 0..2 {
                for c2 in 0..2 {
                    let actual = result.data().get(&[n1, n2], &[c1, c2]).unwrap();
                    if [n1, n2] == peak {
                        let expected = (N1 * N2) as f64 * [a1.re, a1.im][c1] * [a2.re, a2.im][c2];
                        close(actual, expected, 2e-12);
                        peak_energy += actual * actual;
                    } else {
                        leakage_energy += actual * actual;
                    }
                }
            }
        }
    }
    assert!((leakage_energy / peak_energy).sqrt() < 1e-13);
}
