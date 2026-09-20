use super::support::*;
use crate::raw_support::*;
use nmr::axis::AxisCoordinates;
use nmr::formats::jeol::{ParameterValue, Parts, read_parts};
use nmr::processing::{
    FourierTransform, ProcessingOperation as Op, ProcessingPlan, SpectrumOperation, ZeroFill,
};

fn padded(stored: usize, first: usize, valid: usize, start: f64, dt: f64) -> Vec<u8> {
    let mut bytes = jeol_fixture_f64();
    bytes.resize(1360 + stored * 16, 0);
    bytes[32] = 1;
    bytes[33] = 28;
    put_be_u32(&mut bytes, 176, stored as u32);
    put_be_u32(&mut bytes, 208, first as u32);
    put_be_u32(&mut bytes, 240, (first + valid - 1) as u32);
    bytes[272..280].copy_from_slice(&start.to_be_bytes());
    bytes[336..344].copy_from_slice(&(start + (valid - 1) as f64 * dt).to_be_bytes());
    // Padding deliberately contains huge finite values: it is never FID data.
    for i in 0..stored {
        let re: f64 = if (first..first + valid).contains(&i) {
            1.0
        } else {
            1e12
        };
        bytes[1360 + i * 8..1368 + i * 8].copy_from_slice(&re.to_le_bytes());
    }
    bytes[1360 + stored * 8..].fill(0);
    jeol_with_parameter_records(
        &bytes,
        &[jeol_parameter_record(
            "x_sweep",
            ParameterValue::Float(dt.recip()),
            13,
        )],
    )
}

#[test]
fn proton_storage_padding_does_not_change_dwell_or_enter_the_fft() {
    let bytes = padded(59800, 0, 59793, 0.0, 53.28e-6);
    let raw = read_parts(Parts::new(&bytes).allow_experimental_vendor_semantics(true)).unwrap();
    assert_eq!(raw.data().shape(), &[59793]);
    let AxisCoordinates::Uniform { start, step } = raw.descriptor().axes()[0].coordinates() else {
        panic!()
    };
    assert_eq!(*start, 0.0);
    assert!((*step - 53.28e-6).abs() < 1e-18);
    assert!((*step * 59792.0 - 3.18571776).abs() < 1e-14);
    assert!(
        (raw.descriptor().axes()[0].spectral_width_hz().unwrap() - 18768.76876876877).abs() < 1e-9
    );
    let spectrum = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply_raw(&raw)
    .unwrap();
    assert!((spectrum.data().get(&[59793 / 2], &[0]).unwrap() - 59793.0).abs() < 1e-7);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("padded.jdf");
    std::fs::write(&path, bytes).unwrap();
    let lazy = nmr::raw::OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .open(&path)
        .unwrap();
    assert_eq!(lazy.descriptor().axes(), raw.descriptor().axes());
    assert_eq!(lazy.into_dataset().unwrap().data(), raw.data());
}

#[test]
fn valid_time_endpoints_and_later_cropping_preserve_physical_origin_and_dwell() {
    for first in [0, 2] {
        let bytes = padded(12, first, 6, 0.125, 0.0625);
        let raw = read_parts(Parts::new(&bytes).allow_experimental_vendor_semantics(true)).unwrap();
        assert_eq!(
            raw.descriptor().axes()[0].coordinates(),
            &AxisCoordinates::Uniform {
                start: 0.125,
                step: 0.0625
            }
        );
        let cropped = ProcessingPlan::new(vec![
            Op::Spectrum {
                axis: 0,
                operation: SpectrumOperation::RetainRange { start: 1, end: 5 },
            },
            Op::ZeroFill {
                axis: 0,
                zero_fill: ZeroFill::new(8).unwrap(),
            },
        ])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
        assert_eq!(
            cropped.descriptor().axes()[0].coordinates(),
            &AxisCoordinates::Uniform {
                start: 0.1875,
                step: 0.0625
            }
        );
        let spectrum = ProcessingPlan::new(vec![Op::FourierTransform {
            axis: 0,
            transform: FourierTransform::default(),
        }])
        .unwrap()
        .apply_processed(&cropped)
        .unwrap();
        for k in 0..8 {
            let f = (k as f64 - 4.0) * 2.0;
            let expected: nmr::Complex64 = (0..4)
                .map(|n| {
                    nmr::Complex64::from_polar(
                        1.0,
                        -std::f64::consts::TAU * f * (0.1875 + n as f64 * 0.0625),
                    )
                })
                .sum();
            assert!((spectrum.data().get(&[k], &[0]).unwrap() - expected.re).abs() < 1e-12);
            assert!((spectrum.data().get(&[k], &[1]).unwrap() - expected.im).abs() < 1e-12);
        }
    }
}

#[test]
fn genuine_dwell_sweep_conflicts_still_fail_preflight() {
    let mut bytes = padded(12, 0, 6, 0.0, 0.0625);
    bytes[336..344].copy_from_slice(&0.30_f64.to_be_bytes());
    let raw = read_parts(Parts::new(&bytes).allow_experimental_vendor_semantics(true)).unwrap();
    let plan = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap();
    assert!(plan.apply_raw(&raw).is_err());
}
