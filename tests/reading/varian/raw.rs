use super::support::*;
use crate::raw_support::*;
use nmr::Complex64;
use nmr::axis::AxisCoordinates;
use nmr::formats::varian::ReadAssertions;
use nmr::raw::{
    AssertionId, LinearComponentTransform, PeriodicLaneModulation, ResolutionAuthority,
    UnsupportedFeatureCode,
};
use nmr::raw::{
    DirectSamples, IndirectComponents, ObservationOrdinal, OpenOptions, RawAxisKind,
    RawDataset as Acquisition, RawFormat as Format, ReadErrorKind, ReadErrorReason, ReadLimits,
};
use std::fs;

fn phase_parameters(f1coef: Option<&[&str]>) -> String {
    let mut records: Vec<(&str, &[&str])> = vec![
        ("np", &["2"]),
        ("ni", &["2"]),
        ("arraydim", &["4"]),
        ("array", &["phase"]),
        ("phase", &["1", "2"]),
        ("sw", &["4000"]),
        ("sw1", &["1000"]),
    ];
    if let Some(values) = f1coef {
        records.push(("f1coef", values));
    }
    procpar(&records)
}

fn encoded_transform(dataset: &Acquisition) -> &nmr::raw::ResolvedComponentTransform {
    let RawAxisKind::Indirect(IndirectComponents::Encoded(transform)) =
        dataset.descriptor().axes()[0].kind()
    else {
        panic!("expected a resolved indirect transform");
    };
    transform
}

fn identity_assertion_transform() -> LinearComponentTransform {
    LinearComponentTransform::try_new(
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        PeriodicLaneModulation::identity(2).unwrap(),
    )
    .unwrap()
}

#[test]
fn varian_direct_rule_applies_scale_sign_and_source_encoding() {
    let dataset = read_from_parts(DatasetParts::Varian {
        fid: &varian_fixture(),
        procpar: &procpar(&[("np", &["4"])]),
        schedule: None,
    })
    .unwrap();
    assert_eq!(dataset.provenance().format(), Some(Format::VarianRaw));
    assert_eq!(
        dataset.descriptor().axes()[0].kind(),
        &RawAxisKind::Direct(DirectSamples::Complex)
    );
    assert_eq!(
        dataset.data().dense_samples().unwrap(),
        &[Complex64::new(2.0, -4.0), Complex64::new(6.0, -8.0)]
    );
    let normalization = dataset.provenance().sample_normalization();
    assert_eq!(
        normalization.algorithm_version(),
        "source-sample-normalization.v1"
    );
    assert_eq!(normalization.source_block_scale_factors(), &[2.0]);
    assert_eq!(normalization.stored_imaginary_multiplier(), -1);
}

#[test]
fn varian_ppm_carrier_uses_spectral_reference_offset() {
    let dataset = read_from_parts(DatasetParts::Varian {
        fid: &varian_fixture(),
        procpar: &procpar(&[
            ("np", &["4"]),
            ("sw", &["4000"]),
            ("rfl", &["200"]),
            ("reffrq", &["400"]),
            ("tof", &["-999"]),
        ]),
        schedule: None,
    })
    .unwrap();
    let reference = dataset.descriptor().axes()[0]
        .chemical_shift_reference()
        .unwrap();
    assert_eq!(reference.carrier_ppm(), 4.5);
    assert_eq!(reference.reference_frequency_mhz(), 400.0);
}

#[test]
fn varian_direct_rule_does_not_infer_complex_from_even_np() {
    let mut fid = varian_fixture();
    fid[26..28].copy_from_slice(&133_u16.to_be_bytes());
    let dataset = read_from_parts(DatasetParts::Varian {
        fid: &fid,
        procpar: &procpar(&[("np", &["4"])]),
        schedule: None,
    })
    .unwrap();
    assert_eq!(
        dataset.descriptor().axes()[0].kind(),
        &RawAxisKind::Direct(DirectSamples::Real)
    );
    assert_eq!(dataset.descriptor().logical_shape(), vec![4]);
}

#[test]
fn varian_reference_position_is_applied_on_each_axis() {
    for (position, expected) in [
        (Some("1000"), 7.0),
        (Some("-1000"), 2.0),
        (Some("0"), 4.5),
        (None, 4.5),
    ] {
        let mut records: Vec<(&str, &[&str])> = vec![
            ("np", &["2"]),
            ("ni", &["2"]),
            ("arraydim", &["4"]),
            ("array", &["phase"]),
            ("phase", &["1", "2"]),
            ("sw", &["4000"]),
            ("rfl", &["200"]),
            ("reffrq", &["400"]),
            ("sw1", &["4000"]),
            ("rfl1", &["200"]),
            ("reffrq1", &["400"]),
        ];
        let values = [position.unwrap_or("0")];
        if position.is_some() {
            records.extend([("rfp", values.as_slice()), ("rfp1", values.as_slice())]);
        }
        let dataset = read_from_parts(DatasetParts::Varian {
            fid: &varian_numbered_traces(4),
            procpar: &procpar(&records),
            schedule: None,
        })
        .unwrap();
        assert_eq!(dataset.descriptor().axes().len(), 2);
        for axis in dataset.descriptor().axes() {
            let reference = axis.chemical_shift_reference().unwrap();
            assert_eq!(reference.carrier_ppm(), expected);
            assert_eq!(
                reference.evidence().derivation()[0].as_str(),
                "openvnmrj.5e20f6f.chemical-shift-reference.v1"
            );
            // Independent centered-Hz grid: -2000, -1000, 0, 1000 Hz.
            for (hz, offset) in [(-2000.0, -5.0), (-1000.0, -2.5), (0.0, 0.0), (1000.0, 2.5)] {
                assert_eq!(reference.ppm(hz).unwrap(), expected + offset);
            }
        }
    }
}

#[test]
fn varian_reference_defaults_missing_offsets_but_rejects_invalid_values() {
    let base: Vec<(&str, &[&str])> = vec![("np", &["4"]), ("sw", &["4000"]), ("reffrq", &["400"])];
    let read = |records: &[(&str, &[&str])]| {
        read_from_parts(DatasetParts::Varian {
            fid: &varian_fixture(),
            procpar: &procpar(records),
            schedule: None,
        })
    };
    let dataset = read(&base).unwrap();
    assert_eq!(
        dataset.descriptor().axes()[0]
            .chemical_shift_reference()
            .unwrap()
            .carrier_ppm(),
        5.0
    );
    for name in ["rfl", "rfp"] {
        for value in ["NaN", "inf", "invalid"] {
            let values = [value];
            let mut records = base.clone();
            records.push((name, &values));
            assert_eq!(
                read(&records).unwrap_err().kind(),
                ReadErrorKind::InvalidMetadata
            );
        }
    }
}

#[test]
fn varian_direct_array_rule_keeps_an_ordinary_parameter_out_of_component_lanes() {
    let parameters = procpar(&[
        ("np", &["2"]),
        ("arraydim", &["2"]),
        ("array", &["d2"]),
        ("d2", &["0.1", "0.2"]),
    ]);
    let dataset = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(2),
        procpar: &parameters,
        schedule: None,
    })
    .unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), vec![2, 1]);
    assert_eq!(dataset.descriptor().component_lanes(), vec![1, 1]);
    assert_eq!(
        dataset.descriptor().axes()[0].kind(),
        &RawAxisKind::Parameter
    );
    assert_eq!(
        dataset.descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Explicit(vec![0.1, 0.2])
    );
}

#[test]
fn default_and_explicit_coefficients_preserve_their_source_signs() {
    let default = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(4),
        procpar: &phase_parameters(None),
        schedule: None,
    })
    .unwrap();
    assert_eq!(
        encoded_transform(&default).transform().coefficients(),
        &[
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]
    );
    let ResolutionAuthority::FormatRule(default_rule) =
        encoded_transform(&default).evidence().authority()
    else {
        panic!("format resolver must own this evidence");
    };
    assert_eq!(default_rule.as_str(), "varian.2d-phase-default-ptype.v1");

    let explicit = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(4),
        procpar: &phase_parameters(Some(&["1", "0", "1", "0", "0", "-1", "0", "1"])),
        schedule: None,
    })
    .unwrap();
    assert_eq!(
        encoded_transform(&explicit).transform().coefficients(),
        &[
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, -1.0),
            Complex64::new(0.0, 1.0),
        ]
    );
    let ResolutionAuthority::FormatRule(explicit_rule) =
        encoded_transform(&explicit).evidence().authority()
    else {
        panic!("format resolver must own this evidence");
    };
    assert_eq!(explicit_rule.as_str(), "varian.2d-phase-explicit-f1coef.v1");
    assert_ne!(default_rule, explicit_rule);

    // The coefficient evidence and matrix must survive offline restoration.
    let input = nmr::Dataset::from_raw(explicit);
    let mut bytes = Vec::new();
    nmr::snapshot::write_snapshot(&input, &mut bytes, Default::default()).unwrap();
    let restored = nmr::snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(nmr::snapshot::AcceptRecordedHistory);
    assert_eq!(input.as_raw(), restored.as_raw());
}

#[test]
fn explicit_ztocsy_coefficients_preserve_negative_sine_without_sequence_inference() {
    let parameters = phase_parameters(Some(&["1", "0", "0", "0", "0", "0", "-1", "0"]));
    let dataset = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(4),
        procpar: &parameters,
        schedule: None,
    })
    .unwrap();
    assert_eq!(
        encoded_transform(&dataset).transform().coefficients(),
        &[
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(-1.0, 0.0),
        ]
    );
}

#[test]
fn missing_empty_and_explicit_default_f1coef_have_identical_transforms() {
    let fid = varian_numbered_traces(4);
    let mut transforms = Vec::new();
    for values in [
        None,
        Some(&[][..]),
        Some(&["1", "0", "0", "0", "0", "0", "1", "0"][..]),
    ] {
        let dataset = read_from_parts(DatasetParts::Varian {
            fid: &fid,
            procpar: &phase_parameters(values),
            schedule: None,
        })
        .unwrap();
        transforms.push(encoded_transform(&dataset).transform().clone());
    }
    assert_eq!(transforms[0], transforms[1]);
    assert_eq!(transforms[0], transforms[2]);
}

#[test]
fn varian_2d_analytic_peaks_are_not_mirrored_by_f1coef() {
    use nmr::processing::{
        FourierExponentSign, FourierTransform, ProcessingOperation, ProcessingPlan,
    };
    use std::f64::consts::TAU;

    const N: usize = 8;
    // Independent acquisition models: States C/S, zTOCSY C/-S, and
    // echo/antiecho (C+iS)/2, (C-iS)/2. The stored direct lane is conjugated.
    for (encoding, coef) in [
        (0, None),
        (0, Some(&[][..])),
        (0, Some(&["1", "0", "0", "0", "0", "0", "1", "0"][..])),
        (1, Some(&["1", "0", "0", "0", "0", "0", "-1", "0"][..])),
        (2, Some(&["1 0 1 0 0 -1 0 1"][..])),
    ] {
        for (k1, k2) in [(1_isize, 1_isize), (-2, 1), (1, -2)] {
            let trace_bytes = N * 8;
            let block_bytes = 28 + trace_bytes;
            let mut fid = vec![0_u8; 32 + 2 * N * block_bytes];
            for (offset, value) in [
                (0, 2 * N),
                (4, 1),
                (8, 2 * N),
                (12, 4),
                (16, trace_bytes),
                (20, block_bytes),
                (28, 1),
            ] {
                put_be_u32(&mut fid, offset, value as u32);
            }
            fid[24..26].copy_from_slice(&1_u16.to_be_bytes());
            fid[26..28].copy_from_slice(&153_u16.to_be_bytes()); // data/float/complex/acquisition
            for t1 in 0..N {
                let theta1 = TAU * k1 as f64 * t1 as f64 / N as f64;
                let c = theta1.cos();
                let s = theta1.sin();
                let lanes = match encoding {
                    0 => [Complex64::new(c, 0.0), Complex64::new(s, 0.0)],
                    1 => [Complex64::new(c, 0.0), Complex64::new(-s, 0.0)],
                    _ => [Complex64::new(c, s) * 0.5, Complex64::new(c, -s) * 0.5],
                };
                for (lane, amplitude) in lanes.into_iter().enumerate() {
                    for t2 in 0..N {
                        let direct =
                            Complex64::from_polar(1.0, TAU * k2 as f64 * t2 as f64 / N as f64);
                        let stored = (amplitude * direct).conj();
                        let offset = 32 + (2 * t1 + lane) * block_bytes + 28 + t2 * 8;
                        fid[offset..offset + 4].copy_from_slice(&(stored.re as f32).to_be_bytes());
                        fid[offset + 4..offset + 8]
                            .copy_from_slice(&(stored.im as f32).to_be_bytes());
                    }
                }
            }
            let mut records: Vec<(&str, &[&str])> = vec![
                ("np", &["16"]),
                ("ni", &["8"]),
                ("arraydim", &["16"]),
                ("array", &["phase"]),
                ("phase", &["1", "2"]),
                ("sw", &["8"]),
                ("sw1", &["8"]),
            ];
            if let Some(values) = coef {
                records.push(("f1coef", values));
            }
            let raw = read_from_parts(DatasetParts::Varian {
                fid: &fid,
                procpar: &procpar(&records),
                schedule: None,
            })
            .unwrap();
            for sign in [FourierExponentSign::Negative, FourierExponentSign::Positive] {
                let output = ProcessingPlan::new(vec![
                    ProcessingOperation::ComponentTransform { axis: 0 },
                    ProcessingOperation::FourierTransform {
                        axis: 1,
                        transform: FourierTransform::new(sign),
                    },
                    ProcessingOperation::FourierTransform {
                        axis: 0,
                        transform: FourierTransform::new(sign),
                    },
                ])
                .unwrap()
                .apply_raw(&raw)
                .unwrap();
                let orientation = if sign == FourierExponentSign::Negative {
                    1
                } else {
                    -1
                };
                let bin =
                    |k: isize| ((orientation * k + N as isize / 2).rem_euclid(N as isize)) as usize;
                for f1 in 0..N {
                    for f2 in 0..N {
                        let magnitude = [[0, 0], [0, 1], [1, 0], [1, 1]]
                            .into_iter()
                            .map(|components| {
                                output.data().get(&[f1, f2], &components).unwrap().powi(2)
                            })
                            .sum::<f64>()
                            .sqrt();
                        let expected = if [f1, f2] == [bin(k1), bin(k2)] {
                            (N * N) as f64
                        } else {
                            0.0
                        };
                        assert!(
                            (magnitude - expected).abs() < 1e-5,
                            "encoding={encoding}, sign={sign:?}, peak=({k1},{k2}), bin=({f1},{f2}): {magnitude} != {expected}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn quoted_openvnmrj_f1coef_string_uses_the_same_explicit_rule() {
    let parameters = procpar(&[
        ("np", &["2"]),
        ("ni", &["2"]),
        ("array", &["phase"]),
        ("phase", &["1", "2"]),
        ("arraydim", &["4"]),
        ("f1coef", &["1 0 1 0 0 -1 0 1"]),
    ]);
    let dataset = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(4),
        procpar: &parameters,
        schedule: None,
    })
    .unwrap();
    assert_eq!(
        encoded_transform(&dataset).transform().coefficients(),
        &[
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, -1.0),
            Complex64::new(0.0, 1.0),
        ]
    );
}

#[test]
fn inactive_nonempty_f1coef_is_still_explicit_and_records_its_flag() {
    let mut parameters = phase_parameters(Some(&["1", "0", "1", "0", "0", "-1", "0", "1"]));
    parameters = parameters.replace("f1coef 1 1 4 0 0 2 1 0 1 64", "f1coef 1 1 4 0 0 2 1 0 0 64");
    let dataset = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(4),
        procpar: &parameters,
        schedule: None,
    })
    .unwrap();
    assert_eq!(
        encoded_transform(&dataset).transform().coefficients(),
        &[
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, -1.0),
            Complex64::new(0.0, 1.0),
        ]
    );
    assert!(
        encoded_transform(&dataset)
            .evidence()
            .facts()
            .iter()
            .any(|fact| {
                matches!(
                    fact,
                    nmr::raw::NormalizationFact::SourceCoefficients {
                        active: false,
                        values: 8
                    }
                )
            })
    );
}

#[test]
fn malformed_or_degenerate_explicit_coefficients_fail_structurally() {
    for values in [
        &["1", "0", "0"][..],
        &["1", "0", "0", "0", "0", "0", "NaN", "0"][..],
    ] {
        let error = read_from_parts(DatasetParts::Varian {
            fid: &varian_numbered_traces(4),
            procpar: &phase_parameters(Some(values)),
            schedule: None,
        })
        .unwrap_err();
        assert_eq!(error.kind(), ReadErrorKind::InvalidMetadata);
    }

    let degenerate = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(4),
        procpar: &phase_parameters(Some(&["1", "0", "1", "0", "-1", "0", "-1", "0"])),
        schedule: None,
    })
    .unwrap_err();
    assert_eq!(degenerate.kind(), ReadErrorKind::UnsupportedFeature);
}

#[test]
fn unsupported_phase_orders_arrays_nus_and_extra_indirect_axes_fail_closed() {
    let cases = [
        procpar(&[
            ("np", &["2"]),
            ("ni", &["2"]),
            ("arraydim", &["4"]),
            ("array", &["phase"]),
            ("phase", &["2", "1"]),
        ]),
        procpar(&[
            ("np", &["2"]),
            ("ni", &["2"]),
            ("arraydim", &["4"]),
            ("array", &["phase,d2"]),
            ("phase", &["1", "2"]),
            ("d2", &["0", "1"]),
        ]),
        procpar(&[
            ("np", &["2"]),
            ("ni", &["2"]),
            ("ni2", &["2"]),
            ("arraydim", &["4"]),
            ("array", &["phase"]),
            ("phase", &["1", "2"]),
        ]),
    ];
    for parameters in cases {
        let error = read_from_parts(DatasetParts::Varian {
            fid: &varian_numbered_traces(4),
            procpar: &parameters,
            schedule: None,
        })
        .unwrap_err();
        assert_eq!(error.kind(), ReadErrorKind::UnsupportedFeature);
        assert!(matches!(
            error.reason(),
            ReadErrorReason::UnsupportedFeature { code, .. }
                if *code == UnsupportedFeatureCode::VARIAN_UNSUPPORTED_COMPONENT_LAYOUT
        ));
    }

    let nus = read_from_parts(DatasetParts::Varian {
        fid: &varian_numbered_traces(4),
        procpar: &phase_parameters(None),
        schedule: Some("0\n1\n"),
    })
    .unwrap_err();
    assert_eq!(nus.kind(), ReadErrorKind::UnsupportedFeature);
}

#[test]
fn caller_assertions_cannot_override_header_or_resolved_layout_facts() {
    let assertion = ReadAssertions::try_new(
        AssertionId::try_new("test.direct-conflict").unwrap(),
        DirectSamples::Real,
        vec![0],
        identity_assertion_transform(),
    )
    .unwrap();
    let error = nmr::formats::varian::read_parts(
        nmr::formats::varian::Parts::new(&varian_fixture(), &procpar(&[("np", &["4"])]))
            .assertions(assertion),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::AssertionConflict);

    let permutation_conflict = ReadAssertions::try_new(
        AssertionId::try_new("test.permutation-conflict").unwrap(),
        DirectSamples::Complex,
        vec![1, 0, 2, 3],
        identity_assertion_transform(),
    )
    .unwrap();
    let parameters = phase_parameters(None);
    let error = nmr::formats::varian::read_parts(
        nmr::formats::varian::Parts::new(&varian_numbered_traces(4), &parameters)
            .assertions(permutation_conflict),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::AssertionConflict);
}

#[test]
fn complete_caller_assertion_fills_only_missing_varian_layout_facts() {
    let assertion = ReadAssertions::try_new(
        AssertionId::try_new("test.missing-varian-layout.v1").unwrap(),
        DirectSamples::Complex,
        vec![2, 3, 0, 1],
        identity_assertion_transform(),
    )
    .unwrap();
    let parameters = procpar(&[("np", &["2"]), ("ni", &["2"]), ("arraydim", &["4"])]);
    let fid = varian_numbered_traces(4);
    let dataset = nmr::formats::varian::read_parts(
        nmr::formats::varian::Parts::new(&fid, &parameters).assertions(assertion.clone()),
    )
    .unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), [2, 1]);
    assert_eq!(dataset.descriptor().component_lanes(), [2, 1]);
    assert!(matches!(
        dataset.descriptor().layout_evidence().authority(),
        ResolutionAuthority::CallerAssertion(id)
            if id.as_str() == "test.missing-varian-layout.v1"
    ));
    assert_eq!(
        dataset.read_trace(&[0]).unwrap().samples(),
        [Complex64::new(2.0, 2.0), Complex64::new(3.0, 3.0)]
    );
    assert_eq!(
        dataset.read_trace(&[1]).unwrap().samples(),
        [Complex64::new(0.0, 0.0), Complex64::new(1.0, 1.0)]
    );

    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("fid"), fid).unwrap();
    fs::write(directory.path().join("procpar"), parameters).unwrap();
    let reader = OpenOptions::new()
        .varian_assertions(assertion)
        .open(directory.path())
        .unwrap();
    assert_eq!(
        reader.read_trace(&[0]).unwrap().samples(),
        dataset.read_trace(&[0]).unwrap().samples()
    );
    assert_eq!(reader.into_dataset().unwrap().data(), dataset.data());
}

#[test]
fn caller_assertion_rejects_unproved_ordinal_modulation_after_trace_reordering() {
    let modulation = PeriodicLaneModulation::try_new(
        2,
        2,
        vec![Complex64::new(1.0, 0.0); 4],
        nmr::raw::ModulationIndexDomain::ObservationOrdinal,
        0,
    )
    .unwrap();
    let transform = LinearComponentTransform::try_new(
        2,
        identity_assertion_transform().coefficients().to_vec(),
        modulation,
    )
    .unwrap();
    let assertion = ReadAssertions::try_new(
        AssertionId::try_new("test.unproved-ordinal-layout.v1").unwrap(),
        DirectSamples::Complex,
        vec![2, 3, 0, 1],
        transform,
    )
    .unwrap();
    let parameters = procpar(&[("np", &["2"]), ("ni", &["2"]), ("arraydim", &["4"])]);
    let error = nmr::formats::varian::read_parts(
        nmr::formats::varian::Parts::new(&varian_numbered_traces(4), &parameters)
            .assertions(assertion),
    )
    .unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::AssertionConflict);
}

#[test]
fn caller_assertion_rejects_non_bijective_trace_permutations() {
    let error = ReadAssertions::try_new(
        AssertionId::try_new("test.bad-permutation.v1").unwrap(),
        DirectSamples::Complex,
        vec![0, 0],
        identity_assertion_transform(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        nmr::raw::EvidenceValidationError::InvalidTracePermutation
    );
}

#[test]
fn varian_lazy_trace_and_materialization_match_in_memory_bits_and_finalize_digest() {
    let parameters = phase_parameters(None);
    let fid = varian_numbered_traces(4);
    let expected = read_from_parts(DatasetParts::Varian {
        fid: &fid,
        procpar: &parameters,
        schedule: None,
    })
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("fid"), fid).unwrap();
    fs::write(directory.path().join("procpar"), parameters).unwrap();
    let reader = nmr::raw::open(directory.path()).unwrap();
    assert_eq!(
        reader.read_trace(&[1]).unwrap().samples(),
        expected.data().read_trace(&[1]).unwrap().samples()
    );
    let ordinal = ObservationOrdinal::new(1);
    assert_eq!(
        reader.read_observation(ordinal).unwrap().samples(),
        expected.read_observation(ordinal).unwrap().samples()
    );
    let region = nmr::raw::Region::new([1, 0], [1, 1]).unwrap();
    assert_eq!(
        reader
            .read_region(&region, usize::MAX)
            .unwrap()
            .data()
            .dense_samples(),
        expected
            .data()
            .read_region(&region, usize::MAX)
            .unwrap()
            .data()
            .dense_samples()
    );
    let materialized = reader.into_dataset().unwrap();
    assert_eq!(materialized.data(), expected.data());
    assert_eq!(
        materialized.provenance().sources().len(),
        expected.provenance().sources().len()
    );
    for (path, parts) in materialized
        .provenance()
        .sources()
        .iter()
        .zip(expected.provenance().sources())
    {
        assert_eq!(path.digest(), parts.digest());
        assert_eq!(path.id(), parts.id());
        assert_eq!(path.role(), parts.role());
        assert!(parts.locator().is_none());
    }
    assert!(matches!(
        materialized.provenance().sources()[0].digest(),
        nmr::raw::SourceDigest::Sha256(_)
    ));
}

#[test]
fn varian_materialization_rejects_a_source_changed_after_open() {
    use std::io::Write;

    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("fid"), varian_fixture()).unwrap();
    fs::write(directory.path().join("procpar"), procpar(&[("np", &["4"])])).unwrap();
    let reader = nmr::raw::open(directory.path()).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(directory.path().join("fid"))
        .unwrap()
        .write_all(&[0, 0, 0, 0])
        .unwrap();
    assert_eq!(
        reader.into_dataset().unwrap_err().kind(),
        ReadErrorKind::SourceChanged
    );
}

#[test]
fn varian_materialization_rejects_changed_parameter_evidence() {
    use std::io::Write;

    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("fid"), varian_fixture()).unwrap();
    fs::write(directory.path().join("procpar"), procpar(&[("np", &["4"])])).unwrap();
    let reader = nmr::raw::open(directory.path()).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(directory.path().join("procpar"))
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    assert_eq!(
        reader.into_dataset().unwrap_err().kind(),
        ReadErrorKind::SourceChanged
    );
}

#[test]
fn varian_snapshot_rejects_changed_scale_even_when_file_identity_is_restored() {
    use std::io::{Seek, SeekFrom, Write};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fid");
    fs::write(&path, varian_fixture()).unwrap();
    fs::write(directory.path().join("procpar"), procpar(&[("np", &["4"])])).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let reader = nmr::raw::open(directory.path()).unwrap();
    let mut file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::Start(32)).unwrap();
    file.write_all(&2_i16.to_be_bytes()).unwrap();
    file.set_modified(modified).unwrap();
    drop(file);
    assert_eq!(
        reader.into_dataset().unwrap_err().kind(),
        ReadErrorKind::SourceChanged
    );
}

#[test]
fn varian_open_enforces_component_transform_limits() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("fid"), varian_numbered_traces(4)).unwrap();
    fs::write(directory.path().join("procpar"), phase_parameters(None)).unwrap();
    let cases = [
        (
            ReadLimits::new().max_component_lanes(1),
            nmr::raw::ReadResource::ComponentLanes,
        ),
        (
            ReadLimits::new().max_transform_coefficients(3),
            nmr::raw::ReadResource::TransformCoefficients,
        ),
        (
            ReadLimits::new().max_modulation_period(0),
            nmr::raw::ReadResource::ModulationPeriod,
        ),
        (
            ReadLimits::new().max_transform_work(7),
            nmr::raw::ReadResource::TransformWork,
        ),
    ];
    for (limits, expected_resource) in cases {
        let error = OpenOptions::new()
            .limits(limits)
            .open(directory.path())
            .unwrap_err();
        assert!(matches!(
            error.reason(),
            ReadErrorReason::LimitExceeded { resource, .. } if *resource == expected_resource
        ));
    }
}

#[test]
fn varian_binary_corruption_remains_corrupt_not_unsupported_metadata() {
    let mut trailing = varian_fixture();
    trailing.push(0);
    let error = read_from_parts(DatasetParts::Varian {
        fid: &trailing,
        procpar: &procpar(&[("np", &["4"])]),
        schedule: None,
    })
    .unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::Corrupt);
}
