use nmr::axis::{AxisDomain, AxisRole};
use nmr::raw::{IndirectComponents, RawAxisKind};

use crate::fixture_support::*;

fn bruker_1d_parameters() -> &'static str {
    "##TITLE= fixture 1D\n\
     ##$TD= 4\n\
     ##$PARMODE= 0\n\
     ##$AQ_mod= 3\n\
     ##$BYTORDA= 0\n\
     ##$DTYPA= 0\n\
     ##$SW_h= 8000\n\
     ##$SFO1= 400.13\n\
     ##$BF1= 400.0\n\
     ##$O1= 1880\n\
     ##$NUC1= <1H>\n\
     ##END=\n"
}

fn bruker_2d_parameters(fn_type: usize, fn_mode: usize, continuous: bool) -> (String, String) {
    let block = if continuous {
        "##$GO_block_size= <continuous>\n"
    } else {
        ""
    };
    let direct = format!(
        "##TITLE= fixture 2D\n\
         ##$PARMODE= 1\n\
         ##$AQSEQ= 0\n\
         ##$FnTYPE= {fn_type}\n\
         ##$AQ_mod= 3\n\
         ##$TD= 4\n\
         ##$BYTORDA= 0\n\
         ##$DTYPA= 0\n\
         {block}\
         ##$SW_h= 8000\n\
         ##$SFO1= 400.13\n\
         ##END=\n"
    );
    let indirect = format!(
        "##TITLE= fixture indirect\n\
         ##$TD= 4\n\
         ##$FnMODE= {fn_mode}\n\
         ##$NusTD= 8\n\
         ##$SW_h= 1000\n\
         ##$SFO1= 100.62\n\
         ##END=\n"
    );
    (direct, indirect)
}

#[test]
fn bruker_binary_fixtures_freeze_1d_and_both_dense_row_layouts() {
    let one_dimensional = include_bytes!("../../fixtures/bruker/one-dimensional.fid");
    let dataset = nmr::formats::bruker::read_parts(nmr::formats::bruker::Parts::new(
        one_dimensional,
        &[bruker_1d_parameters()],
    ))
    .unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), vec![2]);
    assert_eq!(dataset.descriptor().component_lanes(), vec![1]);
    assert_eq!(
        sample_bits(dataset.data().dense_samples().unwrap()),
        expected_bits(&[(10.0, -20.0), (30.0, -40.0)])
    );
    let direct = &dataset.descriptor().axes()[0];
    assert_eq!(direct.role(), AxisRole::DirectAcquisition);
    assert_eq!(direct.domain(), AxisDomain::Time);
    assert_eq!(direct.spectral_width_hz(), Some(8000.0));
    assert_eq!(
        direct.frequency_evidence().unwrap().observe_frequency_mhz(),
        Some(400.13)
    );

    for (bytes, continuous) in [
        (
            include_bytes!("../../fixtures/bruker/two-dimensional-kblock.ser").as_slice(),
            false,
        ),
        (
            include_bytes!("../../fixtures/bruker/two-dimensional-contiguous.ser").as_slice(),
            true,
        ),
    ] {
        let (direct, indirect) =
            bruker_2d_parameters(0, if continuous { 6 } else { 5 }, continuous);
        let dataset = nmr::formats::bruker::read_parts(nmr::formats::bruker::Parts::new(
            bytes,
            &[&direct, &indirect],
        ))
        .unwrap();
        assert_eq!(dataset.descriptor().logical_shape(), vec![2, 2]);
        assert_eq!(dataset.descriptor().component_lanes(), vec![2, 1]);
        assert!(matches!(
            dataset.descriptor().axes()[0].kind(),
            RawAxisKind::Indirect(IndirectComponents::Encoded(_))
        ));
        assert_eq!(
            sample_bits(dataset.data().read_trace(&[1]).unwrap().samples()),
            expected_bits(&[(21.0, 22.0), (23.0, 24.0), (31.0, 32.0), (33.0, 34.0)])
        );
    }
}

#[test]
fn bruker_incomplete_nus_fixture_preserves_schedule_order_and_all_lanes() {
    let (direct, indirect) = bruker_2d_parameters(2, 6, true);
    let parameters = [direct.as_str(), indirect.as_str()];
    let parts = nmr::formats::bruker::Parts::new(
        include_bytes!("../../fixtures/bruker/two-dimensional-contiguous.ser"),
        &parameters,
    )
    .nuslist("3\n1\n")
    .allow_experimental_vendor_semantics(true);
    let dataset = nmr::formats::bruker::read_parts(parts).unwrap();
    assert!(dataset.data().is_sparse());
    assert_eq!(dataset.descriptor().logical_shape(), vec![4, 2]);
    assert_eq!(
        dataset
            .sampling_schedule()
            .unwrap()
            .coordinates()
            .iter()
            .map(|coordinate| coordinate.as_slice())
            .collect::<Vec<_>>(),
        vec![&[3][..], &[1][..]]
    );
    assert_eq!(
        sample_bits(dataset.data().read_trace(&[3]).unwrap().samples()),
        expected_bits(&[(1.0, 2.0), (3.0, 4.0), (11.0, 12.0), (13.0, 14.0)])
    );
}

#[test]
fn bruker_complete_nus_fixture_preserves_schedule_order_and_all_lanes() {
    let (direct, indirect) = bruker_2d_parameters(2, 6, true);
    let indirect = indirect.replace("##$NusTD= 8", "##$NusTD= 4");
    let parameters = [direct.as_str(), indirect.as_str()];
    let parts = nmr::formats::bruker::Parts::new(
        include_bytes!("../../fixtures/bruker/two-dimensional-contiguous.ser"),
        &parameters,
    )
    .nuslist("1\n0\n")
    .allow_experimental_vendor_semantics(true);
    let dataset = nmr::formats::bruker::read_parts(parts).unwrap();
    assert!(!dataset.data().is_sparse());
    assert_eq!(dataset.descriptor().logical_shape(), vec![2, 2]);
    assert_eq!(
        dataset
            .sampling_schedule()
            .unwrap()
            .coordinates()
            .iter()
            .map(|coordinate| coordinate.as_slice())
            .collect::<Vec<_>>(),
        vec![&[1][..], &[0][..]]
    );
    assert_eq!(
        sample_bits(dataset.data().read_trace(&[1]).unwrap().samples()),
        expected_bits(&[(1.0, 2.0), (3.0, 4.0), (11.0, 12.0), (13.0, 14.0)])
    );
}
