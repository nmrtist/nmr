use super::*;
use nmr::axis::AxisUnit;
use nmr::processing::{FrequencyFrame, PhaseMethod, ReferenceSource};

fn slice(axis: usize, index: usize) -> Op {
    Op::Spectrum {
        axis,
        operation: SpectrumOperation::Slice {
            index,
            component: 0,
        },
    }
}

// Specify the frequency-domain answer first; use independent finite inverse
// sums to synthesize the two time factors written to the JDF.
fn fixture() -> (nmr::Dataset, [Vec<C>; 2]) {
    let spectra = [32, 48].map(|n| {
        (0..n)
            .map(|k| {
                let mut z = C::default();
                for (center, height) in [(n / 4, 1.0), (3 * n / 4, 0.7)] {
                    let u = (k as f64 - center as f64) / 1.2;
                    z += C::new(1.0, -u) * (height / (1.0 + u * u));
                }
                z * wave(0.4 + 0.3 * k as f64 / n as f64)
            })
            .collect::<Vec<_>>()
    });
    let time = spectra.each_ref().map(|values| {
        let n = values.len();
        (0..n)
            .map(|t| {
                values
                    .iter()
                    .enumerate()
                    .map(|(k, z)| z * wave(TAU * (k as f64 - (n / 2) as f64) * t as f64 / n as f64))
                    .sum::<C>()
                    / n as f64
            })
            .collect::<Vec<_>>()
    });
    let raw = read(&jdf(4, [32, 48], [0.0; 2], |t1, t2| {
        let z =
            time[0][(t1 * 32.0).round() as usize].conj() * time[1][(t2 * 32.0).round() as usize];
        [z.re, -z.im, 0.0, 0.0]
    }));
    (raw, spectra)
}

fn ppm(input: &nmr::Dataset) -> nmr::Dataset {
    apply(
        input,
        (0..2)
            .map(|axis| Op::ResolveFrequencyFrame {
                axis,
                frame: FrequencyFrame::Ppm(ReferenceSource::Explicit(
                    nmr::raw::ChemicalShiftReference::user_constructed(
                        [4.2, 6.1][axis],
                        [125.0, 500.0][axis],
                    )
                    .unwrap(),
                )),
            })
            .collect(),
    )
}

fn replay_raw(input: &nmr::Dataset, output: &nmr::Dataset) {
    let p = output.as_processed().unwrap();
    let replay = p
        .provenance()
        .history()
        .unwrap()
        .replay_raw(input.as_raw().unwrap(), ProcessingOptions::new())
        .unwrap();
    assert_eq!(replay.data(), p.data());
    assert_eq!(replay.descriptor(), p.descriptor());
}

#[test]
fn both_representative_directions_keep_complex_values_phase_orientation_and_history() {
    let (raw, expected) = fixture();
    let full = ppm(&apply(
        &raw,
        vec![fft(1, Sign::Negative), fft(0, Sign::Negative)],
    ));
    for remaining in 0..2 {
        let removed = 1 - remaining;
        let fixed = expected[removed].len() / 4;
        let trace = snapshot(&apply(&full, vec![slice(removed, fixed)]));
        let p = trace.as_processed().unwrap();
        let original = full.as_processed().unwrap();
        let axis = &p.descriptor().axes()[0];
        let source = &original.descriptor().axes()[remaining];
        assert_eq!(axis.component_basis(), &ComponentBasis::Cartesian);
        assert_eq!(axis.coordinates(), source.coordinates());
        assert_eq!(axis.role(), source.role());
        assert_eq!(axis.label(), source.label());
        assert_eq!(axis.nucleus(), source.nucleus());
        assert_eq!(axis.frequency_evidence(), source.frequency_evidence());
        assert_eq!(axis.spectral_width_hz(), source.spectral_width_hz());
        let evidence = p.axis_evidence(0).unwrap();
        let source_evidence = original.axis_evidence(remaining).unwrap();
        assert_eq!(
            evidence.chemical_shift_reference(),
            source_evidence.chemical_shift_reference()
        );
        assert_eq!(evidence.group_delay(), source_evidence.group_delay());
        let oracle: Vec<_> = expected[remaining]
            .iter()
            .map(|z| {
                // In the surviving axis's local convention, both traces have the
                // same expression. The stored F1 column must be conjugated.
                let z = z * expected[removed][fixed].conj();
                [z.re, z.im]
            })
            .collect();
        for (actual, expected) in p.data().samples().iter().zip(oracle.iter().flatten()) {
            near(*actual, *expected);
        }
        replay_raw(&raw, &trace);
        let reference = crate::spectrum_support::spectrum(
            axis.coordinate_iter().unwrap().collect(),
            &oracle,
            AxisUnit::Ppm,
        );
        for method in [
            PhaseMethod::AbsorptivePeak,
            PhaseMethod::Entropy,
            PhaseMethod::NegativeMinimization,
            PhaseMethod::PeakRegression,
            PhaseMethod::RobustConsensus,
        ] {
            let estimate = method
                .prepare(&trace, 0, ProcessingOptions::new())
                .unwrap()
                .estimate()
                .unwrap();
            let reference_estimate = method
                .prepare(&reference, 0, ProcessingOptions::new())
                .unwrap()
                .estimate()
                .unwrap();
            let correction = estimate.correction();
            let reference_correction = reference_estimate.correction();
            assert!((correction.p0_degrees() - reference_correction.p0_degrees()).abs() < 1e-4);
            assert!((correction.p1_degrees() - reference_correction.p1_degrees()).abs() < 1e-4);
            let corrected_trace =
                snapshot(&estimate.apply(&trace, ProcessingOptions::new()).unwrap());
            replay_raw(&raw, &corrected_trace);
            let corrected_full = apply(
                &full,
                vec![Op::PhaseCorrection {
                    axis: remaining,
                    correction,
                }],
            );
            let after_slice = apply(&corrected_full, vec![slice(removed, fixed)]);
            for (a, b) in after_slice
                .as_processed()
                .unwrap()
                .data()
                .samples()
                .iter()
                .zip(corrected_trace.as_processed().unwrap().data().samples())
            {
                near(*a, *b);
            }
            // Check the actual 2D rotation, including its F1 sign, independently.
            for y in 0..32 {
                for x in 0..48 {
                    let k = [y, x][remaining];
                    let phase = correction.p0_degrees()
                        + correction.p1_degrees()
                            * (k as f64 / expected[remaining].len() as f64
                                - correction.pivot_fraction());
                    let z = expected[0][y].conj()
                        * expected[1][x]
                        * wave((if remaining == 0 { -phase } else { phase }).to_radians());
                    let data = corrected_full.as_processed().unwrap().data();
                    near(data.get(&[y, x], &[0, 0]).unwrap(), z.re);
                    near(data.get(&[y, x], &[0, 1]).unwrap(), z.im);
                }
            }
        }
    }
}

#[test]
fn shared_reference_shifts_both_axes_without_changing_samples_and_survives_slice_replay() {
    let (raw, _) = fixture();
    let hz = apply(&raw, vec![fft(1, Sign::Negative), fft(0, Sign::Negative)]);
    let full = ppm(&hz);
    for selected in 0..2 {
        let shifted = snapshot(&apply(
            &full,
            vec![Op::Spectrum {
                axis: selected,
                operation: SpectrumOperation::Reference { delta_ppm: 0.1 },
            }],
        ));
        let before = full.as_processed().unwrap();
        let after = shifted.as_processed().unwrap();
        assert_eq!(before.data(), after.data());
        assert_eq!(
            before.descriptor().axes()[1 - selected],
            after.descriptor().axes()[1 - selected]
        );
        for (a, b) in before.descriptor().axes()[selected]
            .coordinate_iter()
            .unwrap()
            .zip(
                after.descriptor().axes()[selected]
                    .coordinate_iter()
                    .unwrap(),
            )
        {
            near(b, a + 0.1);
        }
        near(
            after
                .axis_evidence(selected)
                .unwrap()
                .chemical_shift_reference()
                .unwrap()
                .carrier_ppm(),
            before
                .axis_evidence(selected)
                .unwrap()
                .chemical_shift_reference()
                .unwrap()
                .carrier_ppm()
                + 0.1,
        );
        replay_raw(&raw, &shifted);
        // Reversing and taking a column must preserve the descending physical
        // grid and the new reference on the surviving F1 or F2 axis.
        let reversed = apply(
            &shifted,
            vec![Op::Spectrum {
                axis: selected,
                operation: SpectrumOperation::Reverse,
            }],
        );
        let trace = snapshot(&apply(&reversed, vec![slice(1 - selected, 3)]));
        let reference = trace.as_processed().unwrap().axis_evidence(0).unwrap();
        assert_eq!(
            reference.chemical_shift_reference(),
            after
                .axis_evidence(selected)
                .unwrap()
                .chemical_shift_reference()
        );
        assert_eq!(
            trace.as_processed().unwrap().descriptor().axes()[0].coordinates(),
            reversed.as_processed().unwrap().descriptor().axes()[selected].coordinates()
        );
        replay_raw(&raw, &trace);
        for (input, delta) in [(&hz, 0.1), (&full, f64::NAN), (&full, f64::INFINITY)] {
            assert!(
                ProcessingPlan::new(vec![Op::Spectrum {
                    axis: selected,
                    operation: SpectrumOperation::Reference { delta_ppm: delta }
                }])
                .unwrap()
                .preflight(input, ProcessingOptions::new())
                .is_err()
            );
        }
    }
}

#[test]
fn paired_slices_enforce_parameters_working_limits_and_cancellation() {
    let (raw, _) = fixture();
    let full = apply(&raw, vec![fft(1, Sign::Negative), fft(0, Sign::Negative)]);
    for removed in 0..2 {
        let plan = ProcessingPlan::new(vec![slice(removed, 2)]).unwrap();
        let prepared = plan.preflight(&full, ProcessingOptions::new()).unwrap();
        let r = prepared.resources();
        assert!(
            plan.preflight(
                &full,
                ProcessingOptions::new().max_output_bytes(r.output_bytes() - 1)
            )
            .is_err()
        );
        assert!(
            plan.preflight(&full, ProcessingOptions::new().max_working_bytes(1))
                .is_err()
        );
        let limited = plan
            .preflight(
                &full,
                ProcessingOptions::new()
                    .max_output_bytes(r.output_bytes())
                    .max_working_bytes(r.working_bytes())
                    .max_metadata_bytes(r.metadata_bytes()),
            )
            .unwrap()
            .execute()
            .unwrap();
        assert_eq!(
            limited
                .as_processed()
                .unwrap()
                .descriptor()
                .component_counts(),
            [2]
        );
        let cancellation = nmr::CancellationToken::new();
        cancellation.cancel();
        let mut control = nmr::ExecutionContext::default().with_cancellation(cancellation);
        assert!(prepared.execute_with_context(&mut control).is_err());
        for (index, component) in [(usize::MAX, 0), (0, 1)] {
            let bad = ProcessingPlan::new(vec![Op::Spectrum {
                axis: removed,
                operation: SpectrumOperation::Slice { index, component },
            }])
            .unwrap();
            assert!(bad.preflight(&full, ProcessingOptions::new()).is_err());
        }
    }
}

#[test]
fn slice_materialization_handles_either_owner_axis_and_relative_orientation() {
    use crate::spectrum_support::{axis, dataset};
    for owner in 0..2 {
        for conjugated in [false, true] {
            let input = dataset(
                (0..2)
                    .map(|a| {
                        axis(
                            [3, 4][a],
                            AxisUnit::Hertz,
                            if a == owner {
                                ComponentBasis::Cartesian
                            } else {
                                ComponentBasis::SharedComplex {
                                    axis: nmr::AxisIndex::new(owner),
                                    conjugated,
                                }
                            },
                            AxisCoordinates::Explicit(
                                (0..[3, 4][a]).map(|i| 10.0 - i as f64).collect(),
                            ),
                        )
                        .with_label(Some(format!("physical axis {a}")))
                        .with_nucleus(Some(["13C", "1H"][a].into()))
                        .unwrap()
                        .with_frequency_evidence(Some(
                            nmr::axis::FrequencyEvidence::new(
                                Some([125.002, 500.005][a]),
                                Some([42.0, 71.0][a]),
                            )
                            .unwrap(),
                        ))
                        .unwrap()
                    })
                    .collect(),
                (1..=24).map(f64::from).collect(),
            );
            for remaining in 0..2 {
                let trace = snapshot(&apply(&input, vec![slice(1 - remaining, 1)]));
                for p in 0..[3, 4][remaining] {
                    let mut logical = [1, 1];
                    logical[remaining] = p;
                    for c in 0..2 {
                        let mut components = [0, 0];
                        components[owner] = c;
                        let expected = input
                            .as_processed()
                            .unwrap()
                            .data()
                            .get(&logical, &components)
                            .unwrap()
                            * if c == 1 && remaining != owner && conjugated {
                                -1.0
                            } else {
                                1.0
                            };
                        near(
                            trace
                                .as_processed()
                                .unwrap()
                                .data()
                                .get(&[p], &[c])
                                .unwrap(),
                            expected,
                        );
                    }
                }
                let p = trace.as_processed().unwrap();
                let survivor = &p.descriptor().axes()[0];
                let source = &input.as_processed().unwrap().descriptor().axes()[remaining];
                assert_eq!(survivor.label(), source.label());
                assert_eq!(survivor.nucleus(), source.nucleus());
                assert_eq!(survivor.frequency_evidence(), source.frequency_evidence());
                assert_eq!(survivor.coordinates(), source.coordinates());
                let replay = p
                    .provenance()
                    .history()
                    .unwrap()
                    .replay(&[&input], ProcessingOptions::new())
                    .unwrap();
                assert_eq!(replay.canonical_digests(), trace.canonical_digests());
            }
        }
    }
}
