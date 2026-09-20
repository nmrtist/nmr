//! Physical direct-sum oracles through the public JDF reader and processing API.
mod pn;
mod slices;
use nmr::Complex64 as C;
use nmr::axis::AxisCoordinates;
use nmr::formats::jeol::{Parts, read_parts};
use nmr::processed::ComponentBasis;
use nmr::processing::{
    FourierExponentSign as Sign, FourierTransform, PhaseCorrection, PolarityState,
    ProcessingOperation as Op, ProcessingOptions, ProcessingPlan, Projection, SpectrumOperation,
    Window, ZeroFill,
};
use std::f64::consts::TAU;

fn wave(phase: f64) -> C {
    C::from_polar(1.0, phase)
}

// 4x4 tiles in disk x,y order. Unretained storage is deliberately nonzero.
fn jdf(
    kind: u8,
    shape: [usize; 2],
    origin: [f64; 2],
    signal: impl Fn(f64, f64) -> [f64; 4],
) -> Vec<u8> {
    let disk = [shape[1].next_multiple_of(4), shape[0].next_multiple_of(4)];
    let sections = if kind == 3 { 4 } else { 2 };
    let mut bytes = vec![0u8; 1360 + disk.iter().product::<usize>() * sections * 8];
    bytes[..8].copy_from_slice(b"JEOL.NMR");
    bytes[8] = 1;
    bytes[9] = 1;
    bytes[12] = 2;
    bytes[13] = 3;
    bytes[14] = 12;
    bytes[24..26].fill(kind);
    bytes[1284..1288].copy_from_slice(&1360_u32.to_be_bytes());
    for a in 0..2 {
        bytes[32 + 2 * a] = 1;
        bytes[33 + 2 * a] = 28;
        bytes[176 + 4 * a..180 + 4 * a].copy_from_slice(&(disk[a] as u32).to_be_bytes());
        bytes[240 + 4 * a..244 + 4 * a].copy_from_slice(&((shape[1 - a] - 1) as u32).to_be_bytes());
        bytes[272 + 8 * a..280 + 8 * a].copy_from_slice(&origin[1 - a].to_be_bytes());
        bytes[336 + 8 * a..344 + 8 * a]
            .copy_from_slice(&(origin[1 - a] + (shape[1 - a] - 1) as f64 / 32.0).to_be_bytes());
    }
    let mut index = 1360;
    for section in 0..sections {
        for tile_y in 0..disk[1] / 4 {
            for tile_x in 0..disk[0] / 4 {
                for y in tile_y * 4..tile_y * 4 + 4 {
                    for x in tile_x * 4..tile_x * 4 + 4 {
                        let value = if y < shape[0] && x < shape[1] {
                            signal(origin[0] + y as f64 / 32.0, origin[1] + x as f64 / 32.0)
                                [section]
                        } else {
                            1e9
                        };
                        bytes[index..index + 8].copy_from_slice(&value.to_le_bytes());
                        index += 8;
                    }
                }
            }
        }
    }
    bytes
}

fn read(bytes: &[u8]) -> nmr::Dataset {
    read_parts(Parts::new(bytes).allow_experimental_vendor_semantics(true))
        .unwrap()
        .into()
}
fn fft(axis: usize, sign: Sign) -> Op {
    Op::FourierTransform {
        axis,
        transform: FourierTransform::new(sign),
    }
}
fn apply(input: &nmr::Dataset, ops: Vec<Op>) -> nmr::Dataset {
    ProcessingPlan::new(ops)
        .unwrap()
        .preflight(input, ProcessingOptions::new())
        .unwrap()
        .execute()
        .unwrap()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 2e-10, "{a} != {b}");
}
fn snapshot(input: &nmr::Dataset) -> nmr::Dataset {
    let mut bytes = Vec::new();
    nmr::snapshot::write_snapshot(input, &mut bytes, Default::default()).unwrap();
    let restored = nmr::snapshot::read_snapshot(&mut bytes.as_slice(), Default::default())
        .unwrap()
        .restore(nmr::snapshot::AcceptRecordedHistory);
    assert_eq!(input.canonical_digests(), restored.canonical_digests());
    restored
}

#[test]
fn shared_complex_physical_dft_covers_crop_window_fill_both_signs_phase_and_replay() {
    let origin = [0.017, 0.023];
    let signal = |t1: f64, t2: f64| {
        C::new(1.3, -0.4) * wave(TAU * (-3.0 * t1 + 7.0 * t2))
            + C::new(-0.2, 0.6) * wave(TAU * (5.0 * t1 - 4.0 * t2))
    };
    let bytes = jdf(4, [5, 7], origin, |t1, t2| {
        let z = signal(t1, t2);
        [z.re, -z.im, 0.0, 0.0]
    });
    let input = snapshot(&read(&bytes));
    for sign in [Sign::Negative, Sign::Positive] {
        let sigma = if sign == Sign::Negative { -1.0 } else { 1.0 };
        let ops = vec![
            Op::Spectrum {
                axis: 0,
                operation: SpectrumOperation::RetainRange { start: 1, end: 5 },
            },
            Op::Spectrum {
                axis: 1,
                operation: SpectrumOperation::RetainRange { start: 2, end: 7 },
            },
            Op::Window {
                axis: 0,
                window: Window::exponential(0.7).unwrap(),
            },
            Op::Window {
                axis: 1,
                window: Window::exponential(1.1).unwrap(),
            },
            Op::ZeroFill {
                axis: 0,
                zero_fill: ZeroFill::new(9).unwrap(),
            },
            Op::ZeroFill {
                axis: 1,
                zero_fill: ZeroFill::new(10).unwrap(),
            },
            fft(1, sign),
            fft(0, sign),
            Op::PhaseCorrection {
                axis: 0,
                correction: PhaseCorrection::new(13.0, -27.0, 0.3).unwrap(),
            },
            Op::PhaseCorrection {
                axis: 1,
                correction: PhaseCorrection::new(-21.0, 31.0, 0.4).unwrap(),
            },
        ];
        let result = snapshot(&apply(&input, ops));
        let p = result.as_processed().unwrap();
        assert_eq!(p.descriptor().component_counts(), vec![1, 2]);
        assert!(matches!(
            p.descriptor().axes()[0].component_basis(),
            ComponentBasis::SharedComplex {
                conjugated: true,
                ..
            }
        ));
        for k1 in 0..9 {
            for k2 in 0..10 {
                let f1 = (k1 as f64 - 4.0) * 32.0 / 9.0;
                let f2 = (k2 as f64 - 5.0) * 32.0 / 10.0;
                let mut expected = C::new(0.0, 0.0);
                for n1 in 0..4 {
                    for n2 in 0..5 {
                        let t1 = origin[0] + (n1 + 1) as f64 / 32.0;
                        let t2 = origin[1] + (n2 + 2) as f64 / 32.0;
                        let weight = (-std::f64::consts::PI * (0.7 * n1 as f64 + 1.1 * n2 as f64)
                            / 32.0)
                            .exp();
                        expected +=
                            signal(t1, t2) * weight * wave(sigma * TAU * (f2 * t2 - f1 * t1));
                    }
                }
                let phase1 = 13.0 - 27.0 * (k1 as f64 / 9.0 - 0.3);
                let phase2 = -21.0 + 31.0 * (k2 as f64 / 10.0 - 0.4);
                expected *= wave((phase2 - phase1).to_radians());
                near(p.data().get(&[k1, k2], &[0, 0]).unwrap(), expected.re);
                near(p.data().get(&[k1, k2], &[0, 1]).unwrap(), expected.im);
            }
        }
        let replay = p
            .provenance()
            .history()
            .unwrap()
            .replay_raw(input.as_raw().unwrap(), ProcessingOptions::new())
            .unwrap();
        for (a, b) in replay.data().samples().iter().zip(p.data().samples()) {
            near(*a, *b);
        }
        for projection in [Projection::Real, Projection::Magnitude] {
            let projected = apply(
                &result,
                vec![Op::Projection {
                    projection,
                    polarity: PolarityState::Ambiguous180,
                }],
            );
            let scalar = projected.as_processed().unwrap();
            for (i, pair) in p.data().samples().chunks_exact(2).enumerate() {
                near(
                    scalar.data().samples()[i],
                    if projection == Projection::Magnitude {
                        pair[0].hypot(pair[1])
                    } else {
                        pair[0]
                    },
                );
            }
        }
    }
}

#[test]
fn shared_complex_retains_signed_peak_without_equal_mirror_in_either_fft_order() {
    let bytes = jdf(4, [8, 12], [0.0; 2], |t1, t2| {
        let z = wave(TAU * (-8.0 * t1 - 8.0 * t2));
        [z.re, -z.im, 0.0, 0.0]
    });
    let input = read(&bytes);
    for order in [[1, 0], [0, 1]] {
        let result = apply(
            &input,
            vec![fft(order[0], Sign::Negative), fft(order[1], Sign::Negative)],
        );
        let p = result.as_processed().unwrap();
        let peak = [6, 3]; // +8 Hz in F1, -8 Hz in F2.
        let mut leakage = 0.0;
        for y in 0..8 {
            for x in 0..12 {
                let z = C::new(
                    p.data().get(&[y, x], &[0, 0]).unwrap(),
                    p.data().get(&[y, x], &[0, 1]).unwrap(),
                );
                if [y, x] == peak {
                    near(z.re, 96.0);
                    near(z.im, 0.0);
                } else {
                    leakage += z.norm_sqr();
                }
            }
        }
        assert!(leakage.sqrt() / 96.0 < 1e-13);
    }
}

#[test]
fn hypercomplex_sections_retain_both_quadrature_signs_and_four_cartesian_amplitudes() {
    let a = C::new(1.2, -0.3);
    let b = C::new(0.8, 0.4);
    let bytes = jdf(3, [8, 12], [0.013, 0.021], |t1, t2| {
        let u = a * wave(TAU * (-8.0) * t1);
        let v = b * wave(TAU * 8.0 * t2);
        [u.re * v.re, -u.re * v.im, -u.im * v.re, u.im * v.im]
    });
    let input = read(&bytes);
    let result = apply(&input, vec![fft(1, Sign::Negative), fft(0, Sign::Negative)]);
    let p = result.as_processed().unwrap();
    for y in 0..8 {
        for x in 0..12 {
            for c1 in 0..2 {
                for c2 in 0..2 {
                    let expected = if [y, x] == [2, 9] {
                        96.0 * [a.re, a.im][c1] * [b.re, b.im][c2]
                    } else {
                        0.0
                    };
                    near(p.data().get(&[y, x], &[c1, c2]).unwrap(), expected);
                }
            }
        }
    }
}

#[test]
fn shared_complex_preflight_preserves_pairing_and_resource_bounds() {
    let input = read(&jdf(4, [8, 12], [0.0; 2], |t1, t2| {
        let z = wave(TAU * (t2 - t1));
        [z.re, -z.im, 0.0, 0.0]
    }));
    for ops in [
        vec![Op::Projection {
            projection: Projection::Real,
            polarity: PolarityState::Ambiguous180,
        }],
        vec![Op::Spectrum {
            axis: 1,
            operation: SpectrumOperation::Slice {
                index: 0,
                component: 1,
            },
        }],
        vec![Op::ComponentTransform { axis: 0 }],
    ] {
        assert!(
            ProcessingPlan::new(ops)
                .unwrap()
                .preflight(&input, ProcessingOptions::new())
                .is_err()
        );
    }
    let plan = ProcessingPlan::new(vec![fft(1, Sign::Negative), fft(0, Sign::Negative)]).unwrap();
    assert!(
        plan.preflight(&input, ProcessingOptions::new().max_working_bytes(1))
            .is_err()
    );
    let result = plan
        .preflight(&input, ProcessingOptions::new())
        .unwrap()
        .execute()
        .unwrap();
    assert_eq!(
        result.as_processed().unwrap().descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Uniform {
            start: -16.0,
            step: 4.0
        }
    );
}

#[test]
fn shared_complex_orientation_is_part_of_identity_and_controls_signed_frequency() {
    use nmr::axis::{AxisDomain, AxisUnit};
    use nmr::raw::{
        ComponentEvidence, DirectSamples, IndirectComponents, RawAxis, RawAxisKind,
        RawDatasetBuilder, RawMetadata,
    };
    let mut identities = Vec::new();
    for conjugated in [false, true] {
        let time_axis = |kind, points| {
            RawAxis::new(
                kind,
                AxisDomain::Time,
                Some(AxisUnit::Second),
                points,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0 / 32.0,
                },
            )
            .unwrap()
        };
        let input: nmr::Dataset = RawDatasetBuilder::new(
            vec![
                time_axis(
                    RawAxisKind::Indirect(IndirectComponents::SharedComplex {
                        conjugated,
                        evidence: ComponentEvidence::user_constructed(),
                    }),
                    8,
                ),
                time_axis(RawAxisKind::Direct(DirectSamples::Complex), 4),
            ],
            RawMetadata::default(),
        )
        .unwrap()
        .dense(
            (0..8)
                .flat_map(|y| (0..4).map(move |_| wave(TAU * y as f64 / 4.0)))
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .into();
        identities.push(input.canonical_digests());
        let output = apply(
            &snapshot(&input),
            vec![fft(1, Sign::Negative), fft(0, Sign::Negative)],
        );
        let p = output.as_processed().unwrap();
        for y in 0..8 {
            for x in 0..4 {
                let peak = [if conjugated { 2 } else { 6 }, 2];
                near(
                    p.data().get(&[y, x], &[0, 0]).unwrap(),
                    if [y, x] == peak { 32.0 } else { 0.0 },
                );
                near(p.data().get(&[y, x], &[0, 1]).unwrap(), 0.0);
            }
        }
    }
    assert_ne!(identities[0], identities[1]);
}
