//! Independent analytic release gates. Expected signals use closed-form
//! physical line shapes or tones, never the processing kernels under test.

use nmr::Complex64;

#[test]
fn lorentz_to_gauss_recovers_physical_line_shape_width_and_amplitude() {
    use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
    use nmr::processing::{
        FourierExponentSign, FourierTransform, ProcessingOperation as Op, ProcessingPlan, Window,
    };
    use nmr::raw::{DirectSamples, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata};
    const N: usize = 8192;
    const DT: f64 = 0.0005;
    const LORENTZ_WIDTH: f64 = 8.0;
    let pi = std::f64::consts::PI;
    let raw = RawDatasetBuilder::new(
        vec![
            RawAxis::new(
                RawAxisKind::Direct(DirectSamples::Complex),
                AxisDomain::Time,
                Some(AxisUnit::Second),
                N,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: DT,
                },
            )
            .unwrap(),
        ],
        RawMetadata::default(),
    )
    .unwrap()
    .dense(
        (0..N)
            .map(|i| {
                // Half-weight the endpoint as an explicit trapezoidal quadrature
                // input, so the one-sided absorption has no discrete t=0 pedestal.
                Complex64::new(
                    (-pi * LORENTZ_WIDTH * i as f64 * DT).exp() * if i == 0 { 0.5 } else { 1.0 },
                    0.0,
                )
            })
            .collect(),
    )
    .unwrap();
    for (lb, gb, expected_width) in [(LORENTZ_WIDTH, 10.0, 10.0), (-4.0, 0.0, 12.0)] {
        let output = ProcessingPlan::new(vec![
            Op::Window {
                axis: 0,
                window: Window::lorentz_to_gauss(lb, gb).unwrap(),
            },
            Op::FourierTransform {
                axis: 0,
                transform: FourierTransform::new(FourierExponentSign::Negative),
            },
        ])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
        let real = |i| output.data().get(&[i], &[0]).unwrap();
        let peak = real(N / 2);
        let step = 1.0 / (N as f64 * DT);
        for offset in 0..120 {
            let f = offset as f64 * step;
            let expected = if gb > 0.0 {
                (-4.0 * 2.0_f64.ln() * (f / gb).powi(2)).exp()
            } else {
                1.0 / (1.0 + (2.0 * f / expected_width).powi(2))
            };
            let tolerance = if gb > 0.0 { 1e-10 } else { 5e-5 };
            assert!((real(N / 2 + offset) / peak - expected).abs() < tolerance);
        }
        let crossing = (1..N / 2).find(|&i| real(N / 2 + i) <= peak / 2.0).unwrap();
        let high = real(N / 2 + crossing - 1);
        let low = real(N / 2 + crossing);
        let width = 2.0 * step * (crossing as f64 - 1.0 + (high - peak / 2.0) / (high - low));
        assert!((width - expected_width).abs() < 0.02);
        if gb > 0.0 {
            let analytic_peak = 2.0_f64.ln().sqrt() / (pi.sqrt() * gb * DT);
            assert!((peak / analytic_peak - 1.0).abs() < 1e-10);
        }
        println!(
            "window lb={lb} gb={gb}: FWHM={width:.9} Hz, expected={expected_width} Hz; absorption peak={peak:.9}"
        );
    }
}
