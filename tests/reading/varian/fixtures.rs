use nmr::raw::{DirectSamples, RawAxisKind, RawFormat, ReadErrorKind};

use crate::fixture_support::*;

fn varian_procpar(np: usize) -> String {
    format!("np 1 1 4 0 0 2 1 0 1 64\n1 {np}\n0\n")
}

fn varian_parameters(records: &[(&str, &[&str])]) -> String {
    let mut text = String::new();
    for (name, values) in records {
        let real = values.iter().all(|value| value.parse::<f64>().is_ok());
        let basic_type = if real { 1 } else { 2 };
        text.push_str(&format!(
            "{name} {basic_type} {basic_type} 4 0 0 2 1 0 1 64\n"
        ));
        text.push_str(&values.len().to_string());
        for value in *values {
            text.push(' ');
            if value.is_empty() || value.contains(',') {
                text.push('"');
                text.push_str(value);
                text.push('"');
            } else {
                text.push_str(value);
            }
        }
        text.push_str("\n0\n");
    }
    text
}

#[test]
fn varian_binary_fixtures_freeze_real_and_complex_sample_bits() {
    for (bytes, np, encoding, expected) in [
        (
            include_bytes!("../../fixtures/varian/complex.fid").as_slice(),
            4,
            DirectSamples::Complex,
            vec![(2.0, -4.0), (6.0, -8.0)],
        ),
        (
            include_bytes!("../../fixtures/varian/real.fid").as_slice(),
            4,
            DirectSamples::Real,
            vec![(2.0, 0.0), (4.0, 0.0), (6.0, 0.0), (8.0, 0.0)],
        ),
    ] {
        let procpar = varian_procpar(np);
        let dataset =
            nmr::formats::varian::read_parts(nmr::formats::varian::Parts::new(bytes, &procpar))
                .unwrap();
        assert_eq!(dataset.provenance().format(), Some(RawFormat::VarianRaw));
        assert_eq!(
            dataset.descriptor().axes()[0].kind(),
            &RawAxisKind::Direct(encoding)
        );
        assert_eq!(
            sample_bits(dataset.data().dense_samples().unwrap()),
            expected_bits(&expected)
        );
    }
}

#[test]
fn varian_fixture_rejects_multiple_indirect_component_arrays() {
    let bytes = include_bytes!("../../fixtures/varian/eight-trace.fid");
    for array in ["phase2,phase", "phase,phase2"] {
        let parameters = varian_parameters(&[
            ("np", &["2"]),
            ("ni", &["1"]),
            ("ni2", &["2"]),
            ("array", &[array]),
            ("phase", &["1", "2"]),
            ("phase2", &["1", "2"]),
        ]);
        let error =
            nmr::formats::varian::read_parts(nmr::formats::varian::Parts::new(bytes, &parameters))
                .unwrap_err();
        assert_eq!(error.kind(), ReadErrorKind::UnsupportedFeature);
    }
}

#[test]
fn varian_fixture_rejects_nus_until_component_pair_semantics_are_proven() {
    let bytes = include_bytes!("../../fixtures/varian/eight-trace.fid");
    let complete_parameters = varian_parameters(&[("np", &["2"]), ("ni", &["8"])]);
    let complete_schedule = "7\n2\n5\n0\n6\n1\n4\n3\n";
    let complete = nmr::formats::varian::read_parts(
        nmr::formats::varian::Parts::new(bytes, &complete_parameters)
            .sampling_schedule(complete_schedule),
    )
    .unwrap_err();
    assert_eq!(complete.kind(), ReadErrorKind::UnsupportedFeature);

    let incomplete_parameters = varian_parameters(&[("np", &["2"]), ("ni", &["10"])]);
    let incomplete = nmr::formats::varian::read_parts(
        nmr::formats::varian::Parts::new(bytes, &incomplete_parameters)
            .sampling_schedule(complete_schedule),
    )
    .unwrap_err();
    assert_eq!(incomplete.kind(), ReadErrorKind::UnsupportedFeature);
}

#[test]
fn varian_two_block_fixture_requires_an_explicit_phase_layout() {
    let bytes = include_bytes!("../../fixtures/varian/two-block.fid");
    let parameters = varian_parameters(&[("np", &["4"]), ("ni", &["2"])]);
    let error =
        nmr::formats::varian::read_parts(nmr::formats::varian::Parts::new(bytes, &parameters))
            .unwrap_err();
    assert_eq!(error.kind(), ReadErrorKind::UnsupportedFeature);
}
