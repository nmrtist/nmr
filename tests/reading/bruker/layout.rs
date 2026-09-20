use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisRole};
use nmr::processing::{ProcessingOperation, ProcessingPlan};
use nmr::raw::{DirectSamples, IndirectComponents, RawAxisKind, ReadErrorKind};

use super::support::*;

#[test]
fn reads_dense_states_tppi_ser_with_kblock_rows() {
    let (acqus, acqu2s) = two_dimensional_parameters(0, 5, 4, 4, false);
    let ser = encoded_ser(4, false);
    let dataset = read_from_parts(&ser, &[&acqus, &acqu2s], None).unwrap();

    assert_eq!(dataset.data().shape(), &[2, 2]);
    assert_eq!(dataset.data().component_lanes(), &[2, 1]);
    assert_eq!(dataset.data().storage_shape().unwrap(), vec![4, 2]);
    assert_eq!(
        dataset.data().read_trace(&[1]).unwrap().samples(),
        vec![
            Complex64::new(21.0, 22.0),
            Complex64::new(23.0, 24.0),
            Complex64::new(31.0, 32.0),
            Complex64::new(33.0, 34.0),
        ]
    );
    let indirect = &dataset.descriptor().axes()[0];
    assert_eq!(indirect.role(), AxisRole::IndirectAcquisition);
    assert_eq!(indirect.label(), Some("F1"));
    let IndirectComponents::Encoded(transform) = (match indirect.kind() {
        RawAxisKind::Indirect(components) => components,
        _ => panic!("expected an indirect axis"),
    }) else {
        panic!("FnMODE 5 must resolve to an encoded transform");
    };
    assert_eq!(transform.transform().modulation().period(), 2);
    assert_eq!(transform.transform().modulation().origin(), 0);
    assert_eq!(indirect.nucleus(), Some("13C"));
    assert_eq!(
        indirect.coordinates(),
        &AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        }
    );
    let direct = &dataset.descriptor().axes()[1];
    assert_eq!(direct.role(), AxisRole::DirectAcquisition);
    assert_eq!(direct.label(), Some("F2"));
    assert_eq!(direct.kind(), &RawAxisKind::Direct(DirectSamples::Complex));
    let parameters = dataset.provenance().source_metadata().as_bruker().unwrap();
    assert_eq!(parameters.files().len(), 2);
    assert_eq!(parameters.direct().get("TD"), Some("4"));
    assert_eq!(parameters.indirect(0).unwrap().get("FnMODE"), Some("5"));
}

#[test]
fn reads_continuous_echo_anti_echo_ser() {
    let (acqus, acqu2s) = two_dimensional_parameters(0, 6, 2, 2, true);
    let dataset = read_from_parts(&encoded_ser(2, true), &[&acqus, &acqu2s], None).unwrap();

    assert_eq!(dataset.data().shape(), &[1, 2]);
    let RawAxisKind::Indirect(IndirectComponents::Encoded(transform)) =
        dataset.descriptor().axes()[0].kind()
    else {
        panic!("FnMODE 6 must resolve to an encoded transform");
    };
    assert_eq!(
        transform.transform().coefficients(),
        &[
            Complex64::new(0.0, 1.0),
            Complex64::new(0.0, 1.0),
            Complex64::new(-1.0, 0.0),
            Complex64::new(1.0, 0.0),
        ]
    );
    assert_eq!(dataset.data().dense_samples().unwrap().len(), 4);
}

#[test]
fn reads_big_endian_float64_ser_and_resolves_absent_row_mode() {
    let (acqus, acqu2s) = two_dimensional_parameters(0, 6, 2, 2, false);
    let acqus = acqus
        .replace("##$DTYPA= 0", "##$DTYPA= 2")
        .replace("##$BYTORDA= 0", "##$BYTORDA= 1");
    let values = [1.25_f64, -2.5, 3.75, -4.0, 5.5, -6.25, 7.0, -8.5];
    let ser = values
        .into_iter()
        .flat_map(f64::to_be_bytes)
        .collect::<Vec<_>>();
    let dataset = read_from_parts(&ser, &[&acqus, &acqu2s], None).unwrap();

    assert_eq!(
        dataset.data().dense_samples().unwrap(),
        vec![
            Complex64::new(1.25, -2.5),
            Complex64::new(3.75, -4.0),
            Complex64::new(5.5, -6.25),
            Complex64::new(7.0, -8.5),
        ]
    );
}

#[test]
fn accepts_explicit_standard_kblock_rows() {
    let (acqus, acqu2s) = two_dimensional_parameters(0, 5, 2, 2, false);
    let acqus = acqus.replace(
        "##$DTYPA= 0",
        "##$DTYPA= 0\n##$GO_block_size= <Standard_KBlock_Format>",
    );
    let dataset = read_from_parts(&encoded_ser(2, false), &[&acqus, &acqu2s], None).unwrap();
    assert_eq!(dataset.data().shape(), &[1, 2]);
}

#[test]
fn rejects_unverified_bruker_multidimensional_layouts() {
    let (acqus, acqu2s) = two_dimensional_parameters(0, 2, 4, 4, true);
    let error = read_from_parts(&encoded_ser(4, true), &[&acqus, &acqu2s], None).unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::UnsupportedFeature);

    let (acqus_3d, acqu2s_3d, acqu3s_3d) = three_dimensional_parameters(1);
    let error = read_from_parts(
        &encoded_ser(16, true),
        &[&acqus_3d, &acqu2s_3d, &acqu3s_3d],
        None,
    )
    .unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::UnsupportedFeature);

    let (acqus, acqu2s) = two_dimensional_parameters(0, 5, 4, 4, true);
    let acqus = acqus.replace("##$AQ_mod= 3", "##$AQ_mod= 2");
    let error = read_from_parts(&encoded_ser(4, true), &[&acqus, &acqu2s], None).unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::UnsupportedFeature);
}

#[test]
fn states_fnmode4_preserves_both_lanes_without_tppi_modulation() {
    for mode in [4, 5] {
        let (a, b) = two_dimensional_parameters(0, mode, 4, 4, true);
        let raw = read_from_parts(&encoded_ser(4, true), &[&a, &b], None).unwrap();
        let out = ProcessingPlan::new(vec![ProcessingOperation::ComponentTransform { axis: 0 }])
            .unwrap()
            .apply_raw(&raw)
            .unwrap();
        let sign = if mode == 4 { 1.0 } else { -1.0 };
        for (row, base) in [(0, 1.0), (1, 21.0)] {
            let s = if row == 0 { 1.0 } else { sign };
            for lane in 0..2 {
                assert_eq!(
                    out.data().get(&[row, 0], &[lane, 0]).unwrap(),
                    s * (base + 10.0 * lane as f64)
                );
                assert_eq!(
                    out.data().get(&[row, 0], &[lane, 1]).unwrap(),
                    s * (base + 1.0 + 10.0 * lane as f64)
                );
            }
        }
        // Independent separable quadrature tone, encoded as a vendor ser.
        // The same cross-peak must survive both reader -> F2 -> F1 paths.
        let (a, b) = two_dimensional_parameters(0, mode, 8, 8, true);
        let a = a
            .replace("##$TD= 4\n", "##$TD= 16\n")
            .replace("##$DTYPA= 0", "##$DTYPA= 2");
        let a1 = Complex64::from_polar(2.0, 0.3);
        let a2 = Complex64::from_polar(3.0, -0.4);
        let mut bytes = vec![];
        for n1 in 0..4 {
            let z1 = a1 * Complex64::from_polar(1.0, std::f64::consts::TAU * n1 as f64 / 4.0);
            let modulation = if mode == 5 && n1 % 2 == 1 { -1.0 } else { 1.0 };
            for lane in [z1.re, z1.im] {
                for n2 in 0..8 {
                    let z = lane
                        * modulation
                        * a2
                        * Complex64::from_polar(1.0, std::f64::consts::TAU * 2.0 * n2 as f64 / 8.0);
                    bytes.extend(z.re.to_le_bytes());
                    bytes.extend(z.im.to_le_bytes());
                }
            }
        }
        let raw = read_from_parts(&bytes, &[&a, &b], None).unwrap();
        let fft = |axis| ProcessingOperation::FourierTransform {
            axis,
            transform: nmr::processing::FourierTransform::new(
                nmr::processing::FourierExponentSign::Negative,
            ),
        };
        let out = ProcessingPlan::new(vec![
            ProcessingOperation::ComponentTransform { axis: 0 },
            fft(1),
            fft(0),
        ])
        .unwrap()
        .apply_raw(&raw)
        .unwrap();
        for n1 in 0..4 {
            for n2 in 0..8 {
                for c1 in 0..2 {
                    for c2 in 0..2 {
                        let expected = if [n1, n2] == [3, 6] {
                            32.0 * [a1.re, a1.im][c1] * [a2.re, a2.im][c2]
                        } else {
                            0.0
                        };
                        let actual = out.data().get(&[n1, n2], &[c1, c2]).unwrap();
                        assert!((actual - expected).abs() < 2e-12);
                    }
                }
            }
        }
    }
}

#[test]
fn reads_qf_indirect_axis_and_derives_missing_swh_from_sw() {
    let (acqus, acqu2s) = two_dimensional_parameters(0, 1, 3, 3, true);
    let acqu2s = acqu2s
        .replace("##$SW_h= 1000", "##$SW= 10")
        .replace("##$SFO1= 100.62", "##$SFO1= 100");
    let dataset = read_from_parts(&encoded_ser(3, true), &[&acqus, &acqu2s], None).unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), vec![3, 2]);
    assert_eq!(dataset.descriptor().component_lanes(), vec![1, 1]);
    let axis = &dataset.descriptor().axes()[0];
    assert!(matches!(
        axis.kind(),
        RawAxisKind::Indirect(IndirectComponents::Scalar)
    ));
    assert_eq!(axis.spectral_width_hz(), Some(1000.0));
}

#[test]
fn reads_dense_three_dimensional_aqseq_321_in_canonical_axis_order() {
    let (acqus, acqu2s, acqu3s) = three_dimensional_parameters(0);
    let dataset =
        read_from_parts(&encoded_ser(16, true), &[&acqus, &acqu2s, &acqu3s], None).unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), vec![2, 2, 2]);
    assert_eq!(dataset.descriptor().component_lanes(), vec![2, 2, 1]);
    assert_eq!(
        dataset
            .descriptor()
            .axes()
            .iter()
            .map(|axis| axis.label())
            .collect::<Vec<_>>(),
        [Some("F1"), Some("F2"), Some("F3")]
    );
    assert_eq!(
        dataset
            .descriptor()
            .axes()
            .iter()
            .map(|axis| axis.nucleus())
            .collect::<Vec<_>>(),
        [Some("15N"), Some("13C"), Some("1H")]
    );
    let trace = dataset.data().read_trace(&[0, 0]).unwrap();
    assert_eq!(trace.samples()[0].re, 1.0);
    assert_eq!(trace.samples()[2].re, 11.0);
    assert_eq!(trace.samples()[4].re, 41.0);
    assert_eq!(trace.samples()[6].re, 51.0);
}

#[test]
fn three_dimensional_fnmode_product_is_bijective_for_all_supported_modes() {
    let expected_real_bits = (0..16)
        .flat_map(|row| [row * 10 + 1, row * 10 + 3])
        .map(|value| (value as f64).to_bits())
        .collect::<std::collections::BTreeSet<_>>();
    for slow_mode in [1, 5, 6] {
        for fast_mode in [1, 5, 6] {
            let (acqus, acqu2s, acqu3s) = three_dimensional_parameters(0);
            let acqu2s = acqu2s.replace("##$FnMODE= 6", &format!("##$FnMODE= {fast_mode}"));
            let acqu3s = acqu3s.replace("##$FnMODE= 5", &format!("##$FnMODE= {slow_mode}"));
            let dataset =
                read_from_parts(&encoded_ser(16, true), &[&acqus, &acqu2s, &acqu3s], None).unwrap();
            let lanes = |mode| if mode == 1 { 1 } else { 2 };
            assert_eq!(
                dataset.descriptor().component_lanes(),
                [lanes(slow_mode), lanes(fast_mode), 1]
            );
            assert_eq!(
                dataset.descriptor().logical_shape(),
                [4 / lanes(slow_mode), 4 / lanes(fast_mode), 2]
            );
            let observed = dataset
                .data()
                .dense_samples()
                .unwrap()
                .iter()
                .map(|sample| sample.re.to_bits())
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                observed, expected_real_bits,
                "FnMODE {slow_mode}/{fast_mode}"
            );
            assert_eq!(dataset.data().dense_samples().unwrap().len(), 32);
        }
    }
}

#[test]
fn rejects_invalid_bruker_ser_length_and_schedule() {
    let (acqus, acqu2s) = two_dimensional_parameters(0, 5, 4, 4, true);
    let mut truncated = encoded_ser(4, true);
    truncated.pop();
    assert_eq!(
        read_from_parts(&truncated, &[&acqus, &acqu2s], None)
            .unwrap_err()
            .kind(),
        ReadErrorKind::Truncated
    );
    let mut trailing = encoded_ser(4, true);
    trailing.push(0);
    assert_eq!(
        read_from_parts(&trailing, &[&acqus, &acqu2s], None)
            .unwrap_err()
            .kind(),
        ReadErrorKind::Corrupt
    );

    let (nus_acqus, nus_acqu2s) = two_dimensional_parameters(2, 5, 4, 8, true);
    assert_eq!(
        read_from_parts(
            &encoded_ser(4, true),
            &[&nus_acqus, &nus_acqu2s],
            Some("0\n")
        )
        .unwrap_err()
        .kind(),
        ReadErrorKind::InvalidMetadata
    );
}
