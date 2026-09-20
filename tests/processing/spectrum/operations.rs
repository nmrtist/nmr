//! Spectrum operations and independent analytic expectations.
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processed::{ComponentBasis, ProcessedOrigin};
use nmr::processing::{ProcessingOperation as Op, SpectrumOperation as S, *};
use nmr::{Dataset, ExecutionContext};

use crate::spectrum_support::*;

#[test]
fn reference_reverse_invert_affine_preserve_nonuniform_coordinates() {
    let input = spectrum(
        vec![4.0, 3.0, 1.0],
        &[[1.0, 1.0], [2.0, 3.0], [0.0, -4.0]],
        AxisUnit::Ppm,
    );
    let shifted = apply(&input, 0, S::Reference { delta_ppm: -3.0 });
    assert_eq!(
        shifted.as_processed().unwrap().descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Explicit(vec![1.0, 0.0, -2.0])
    );
    let reversed = apply(&input, 0, S::Reverse);
    close(values(&reversed), &[0.0, -4.0, 2.0, 3.0, 1.0, 1.0], 0.0);
    assert_eq!(
        reversed.as_processed().unwrap().descriptor(),
        input.as_processed().unwrap().descriptor()
    );
    close(
        values(&apply(&reversed, 0, S::Reverse)),
        values(&input),
        0.0,
    );
    let inverted = apply(&input, 0, S::Invert);
    close(values(&apply(&inverted, 0, S::Invert)), values(&input), 0.0);
    close(
        values(&apply(
            &input,
            0,
            S::Affine {
                scale: 2.0,
                real_offset: 3.0,
            },
        )),
        &[5.0, 2.0, 7.0, 6.0, 3.0, -8.0],
        0.0,
    );
}

#[test]
fn moving_average_and_sg_reproduce_edges_and_small_arrays() {
    let input = spectrum(
        (0..5).map(f64::from).collect(),
        &[[0.0, 0.0], [0.0, 0.0], [3.0, 6.0], [0.0, 0.0], [0.0, 0.0]],
        AxisUnit::Hertz,
    );
    close(
        values(&apply(&input, 0, S::MovingAverage { window: 3 })),
        &[0.0, 0.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 0.0, 0.0],
        1e-12,
    );
    let polynomial: Vec<_> = (0..21)
        .map(|i| {
            let t = i as f64 / 10.0 - 1.0;
            let y = 2.0 + 3.0 * t - 1.5 * t * t + 0.25 * t * t * t;
            [y, 2.0 * y]
        })
        .collect();
    let input = spectrum((0..21).map(f64::from).collect(), &polynomial, AxisUnit::Ppm);
    close(
        values(&apply(
            &input,
            0,
            S::SavitzkyGolay {
                window: 9,
                order: 3,
            },
        )),
        values(&input),
        1e-9,
    );
    for n in [1, 2, 3, 6] {
        let input = spectrum(
            (0..n).map(|i| i as f64).collect(),
            &vec![[2.0, 4.0]; n],
            AxisUnit::Hertz,
        );
        for operation in [
            S::MovingAverage { window: 9 },
            S::SavitzkyGolay {
                window: 9,
                order: 3,
            },
        ] {
            close(values(&apply(&input, 0, operation)), values(&input), 1e-9);
        }
    }
    // An alternating perturbation isolates the high-frequency response; the
    // exact cubic above is the independent low-frequency reference.
    let noisy: Vec<_> = polynomial
        .iter()
        .enumerate()
        .map(|(i, z)| {
            let noise = if i % 2 == 0 { 0.25 } else { -0.25 };
            [z[0] + noise, z[1] + 2.0 * noise]
        })
        .collect();
    let input = spectrum((0..21).map(f64::from).collect(), &noisy, AxisUnit::Ppm);
    let output = apply(
        &input,
        0,
        S::SavitzkyGolay {
            window: 9,
            order: 3,
        },
    );
    let ratio = (4..17)
        .map(|i| ((values(&output)[2 * i] - polynomial[i][0]) / 0.25).powi(2))
        .sum::<f64>()
        / 13.0;
    assert!(ratio.sqrt() < 0.3, "SG alternating-noise amplitude ratio");
}

#[test]
fn normalization_and_bins_use_complex_values_and_actual_coordinates() {
    let input = spectrum(vec![0.0, 0.5], &[[3.0, 4.0], [-6.0, 8.0]], AxisUnit::Hertz);
    for (mode, divisor) in [
        (Normalization::MaxPeak, 10.0),
        (
            Normalization::TotalArea {
                singleton_width: None,
            },
            4.5,
        ),
        (Normalization::Constant(-2.0), -2.0),
    ] {
        close(
            values(&apply(&input, 0, S::Normalize(mode))),
            &values(&input)
                .iter()
                .map(|v| v / divisor)
                .collect::<Vec<_>>(),
            1e-12,
        );
    }
    let input = spectrum(
        (0..5).map(f64::from).collect(),
        &[
            [1.0, 10.0],
            [2.0, 20.0],
            [3.0, 30.0],
            [4.0, 40.0],
            [5.0, 50.0],
        ],
        AxisUnit::Ppm,
    );
    for (aggregation, expected) in [
        (BinAggregation::Sum, vec![3.0, 30.0, 7.0, 70.0, 5.0, 50.0]),
        (BinAggregation::Mean, vec![1.5, 15.0, 3.5, 35.0, 5.0, 50.0]),
    ] {
        let output = apply(
            &input,
            0,
            S::Bin {
                width: 2.0,
                aggregation,
            },
        );
        close(values(&output), &expected, 1e-12);
        assert_eq!(
            output.as_processed().unwrap().descriptor().axes()[0].coordinates(),
            &AxisCoordinates::Explicit(vec![0.5, 2.5, 4.0])
        );
    }
}

#[test]
fn dimension_reductions_retain_complex_values_and_axis_lineage() {
    let input = dataset(
        vec![
            axis(
                2,
                AxisUnit::Hertz,
                ComponentBasis::Scalar,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0,
                },
            ),
            axis(
                2,
                AxisUnit::Ppm,
                ComponentBasis::Cartesian,
                AxisCoordinates::Explicit(vec![4.0, 1.0]),
            ),
        ],
        vec![1.0, 1.0, -5.0, 0.0, 3.0, 4.0, 2.0, 0.0],
    );
    close(
        values(&apply(&input, 0, S::Sum { component: 0 })),
        &[4.0, 5.0, -3.0, 0.0],
        1e-12,
    );
    close(
        values(&apply(&input, 0, S::Skyline { component: 0 })),
        &[3.0, 4.0, -5.0, 0.0],
        0.0,
    );
    close(
        values(&apply(
            &input,
            0,
            S::Slice {
                index: 1,
                component: 0,
            },
        )),
        &[3.0, 4.0, 2.0, 0.0],
        0.0,
    );
    let hyper = dataset(
        vec![
            axis(
                1,
                AxisUnit::Hertz,
                ComponentBasis::Cartesian,
                AxisCoordinates::Explicit(vec![0.0]),
            ),
            axis(
                1,
                AxisUnit::Hertz,
                ComponentBasis::Cartesian,
                AxisCoordinates::Explicit(vec![0.0]),
            ),
        ],
        vec![3.0, 4.0, 5.0, 12.0],
    );
    let f2 = apply(&hyper, 1, S::Magnitude);
    close(values(&f2), &[5.0, 13.0], 0.0);
    close(
        values(&apply(&f2, 0, S::Magnitude)),
        &[194.0_f64.sqrt()],
        1e-12,
    );
    ProcessingPlan::new(vec![Op::PhaseCorrection {
        axis: 0,
        correction: PhaseCorrection::zero_order_degrees(90.0).unwrap(),
    }])
    .unwrap()
    .apply(&f2)
    .unwrap();
}

#[test]
fn lorentz_to_gauss_matches_analytic_elapsed_time_for_odd_even_singleton() {
    use nmr::raw::*;
    for n in [1, 3, 6] {
        let axis = RawAxis::new(
            RawAxisKind::Direct(DirectSamples::Complex),
            AxisDomain::Time,
            Some(AxisUnit::Second),
            n,
            AxisCoordinates::Uniform {
                start: 0.17,
                step: 0.001,
            },
        )
        .unwrap();
        let input: Dataset = RawDatasetBuilder::new(vec![axis], RawMetadata::default())
            .unwrap()
            .dense(vec![nmr::Complex64::new(2.0, 3.0); n])
            .unwrap()
            .into();
        let plan = ProcessingPlan::new(vec![Op::Window {
            axis: 0,
            window: Window::lorentz_to_gauss(1.0, 2.0).unwrap(),
        }])
        .unwrap();
        let output = plan.apply(&input).unwrap();
        for (i, pair) in values(&output).chunks_exact(2).enumerate() {
            let t = i as f64 * 0.001;
            let pi = std::f64::consts::PI;
            let w = (pi * t - (pi * 2.0 * t).powi(2) / (4.0 * 2.0_f64.ln())).exp();
            close(pair, &[2.0 * w, 3.0 * w], 1e-12);
        }
        let history = output
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        assert_eq!(
            history
                .replay(&[&input], ProcessingOptions::new())
                .unwrap()
                .canonical_digests(),
            output.canonical_digests()
        );
    }
}

#[test]
fn binary_arithmetic_interpolates_both_directions_and_replays_two_sources() {
    let a = spectrum(vec![0.0, 0.5, 1.0, 2.0], &[[1.0, 1.0]; 4], AxisUnit::Ppm);
    for b in [
        spectrum(vec![0.0, 1.0], &[[0.0, 0.0], [2.0, 4.0]], AxisUnit::Ppm),
        spectrum(vec![1.0, 0.0], &[[2.0, 4.0], [0.0, 0.0]], AxisUnit::Ppm),
    ] {
        let prepared = LinearCombination::new(0.5)
            .unwrap()
            .prepare(&a, &b, ProcessingOptions::new())
            .unwrap();
        let r = prepared.resources();
        let output = LinearCombination::new(0.5)
            .unwrap()
            .prepare(
                &a,
                &b,
                ProcessingOptions::new()
                    .max_output_bytes(r.output_bytes())
                    .max_working_bytes(r.working_bytes())
                    .max_metadata_bytes(r.metadata_bytes()),
            )
            .unwrap()
            .execute()
            .unwrap();
        close(
            values(&output),
            &[1.0, 1.0, 1.5, 2.0, 2.0, 3.0, 1.0, 1.0],
            1e-12,
        );
        let ProcessedOrigin::Library(boundary) =
            output.as_processed().unwrap().provenance().origin()
        else {
            panic!("library source evidence missing")
        };
        assert_eq!(boundary.inputs().len(), 2);
        let replay = boundary
            .replay(
                &[&a, &b],
                ProcessingOptions::new(),
                &mut ExecutionContext::default(),
            )
            .unwrap();
        assert_eq!(replay.canonical_digests(), output.canonical_digests());
        assert!(
            boundary
                .replay(
                    &[&b, &a],
                    ProcessingOptions::new(),
                    &mut ExecutionContext::default()
                )
                .is_err()
        );
        apply(&output, 0, S::Invert);
    }
}

#[test]
fn narrow_peak_analysis_uses_every_point_and_range_preserves_origin() {
    let n = 8193;
    let mut z = vec![[0.0, 0.0]; n];
    let peak = nmr::Complex64::from_polar(2.0, 0.73);
    z[4097] = [peak.re, peak.im];
    let input = spectrum((0..n).map(|i| i as f64).collect(), &z, AxisUnit::Hertz);
    let phase = PhaseMethod::AbsorptivePeak
        .prepare(&input, 0, ProcessingOptions::new())
        .unwrap()
        .estimate()
        .unwrap();
    let output = phase.apply(&input, ProcessingOptions::new()).unwrap();
    close(&values(&output)[8194..8196], &[2.0, 0.0], 1e-12);
    let retained = apply(
        &input,
        0,
        S::RetainRange {
            start: 4096,
            end: 4100,
        },
    );
    assert_eq!(
        retained.as_processed().unwrap().descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Explicit(vec![4096.0, 4097.0, 4098.0, 4099.0])
    );
    close(
        values(&retained),
        &[0.0, 0.0, peak.re, peak.im, 0.0, 0.0, 0.0, 0.0],
        0.0,
    );
    let zeros = spectrum(vec![0.0, 1.0, 2.0, 3.0], &[[0.0, 0.0]; 4], AxisUnit::Ppm);
    for method in [
        PhaseMethod::AbsorptivePeak,
        PhaseMethod::Entropy,
        PhaseMethod::NegativeMinimization,
        PhaseMethod::PeakRegression,
        PhaseMethod::RobustConsensus,
    ] {
        assert!(
            method
                .prepare(&zeros, 0, ProcessingOptions::new())
                .unwrap()
                .estimate()
                .is_err()
        );
    }
}
