use nmr::axis::{AxisCoordinates, AxisUnit};
use nmr::processed::Format as ProcessedFormat;
use nmr::{Format, ReadErrorKind, ReadLimits, ReadOptions, ReadResource};
use std::fs;

use crate::support::*;

#[test]
fn strict_jcamp_affn_materializes_a_calibrated_scalar_spectrum() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("spectrum.dx");
    fs::write(
        &path,
        "##TITLE=Example $$ retained title comment\n\
         ##JCAMP-DX=5.00 $$ conforming producer comment\n\
         ##DATA TYPE=NMR SPECTRUM\n\
         ##XUNITS=HZ\n\
         ##YUNITS=ARBITRARY UNITS\n\
         ##XFACTOR=0.1\n\
         ##YFACTOR=2\n\
         ##FIRSTX=100\n\
         ##LASTX=102\n\
         ##NPOINTS=3\n\
         ##DELTAX=1\n\
         ##.OBSERVE FREQUENCY=400\n\
         ##.OBSERVE NUCLEUS=<1H>\n\
         ##XYDATA=(X++(Y..Y))\n\
         1000 1 2 3\n\
         ##END=\n",
    )
    .unwrap();

    assert_eq!(
        nmr::detect(&path).unwrap(),
        Format::Processed(ProcessedFormat::JcampDx)
    );
    let loaded = nmr::read(&path).unwrap();
    let dataset = loaded.as_processed().unwrap();
    assert_eq!(dataset.data().samples(), &[2.0, 4.0, 6.0]);
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.unit(), Some(AxisUnit::Hertz));
    assert_eq!(axis.nucleus(), Some("1H"));
    assert_eq!(axis.spectral_width_hz(), None);
    assert_eq!(axis.coordinate_span(), Some(2.0));
    assert_eq!(
        axis.coordinates(),
        &AxisCoordinates::Uniform {
            start: 100.0,
            step: 1.0,
        }
    );
    let metadata = dataset.provenance().source_metadata().jcamp_dx().unwrap();
    assert!(
        metadata
            .labels()
            .contains(&(".OBSERVE NUCLEUS".to_owned(), "<1H>".to_owned()))
    );

    let from_parts = nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
        &fs::read(path).unwrap(),
    ))
    .unwrap();
    assert_eq!(from_parts.data().samples(), &[2.0, 4.0, 6.0]);
}

#[test]
fn jcamp_coordinates_cover_both_directions_and_do_not_invent_single_point_widths() {
    let descending = compressed_jcamp("1 2 3", 3)
        .replace("##FIRSTX=0", "##FIRSTX=2")
        .replace("##LASTX=2", "##LASTX=0")
        .replace("##DELTAX=1", "##DELTAX=-1")
        .replace("0 1 2 3", "2 1 2 3");
    let dataset = nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
        descending.as_bytes(),
    ))
    .unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(
        axis.coordinates(),
        &AxisCoordinates::Uniform {
            start: 2.0,
            step: -1.0,
        }
    );
    assert_eq!(axis.coordinate_span(), Some(2.0));
    assert_eq!(axis.spectral_width_hz(), None);

    let single = compressed_jcamp("7", 1).replace("##DELTAX=1", "##DELTAX=0");
    let contradictory = single.replace("##LASTX=0", "##LASTX=123");
    assert_eq!(
        nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
            contradictory.as_bytes()
        ))
        .unwrap_err()
        .kind(),
        ReadErrorKind::Corrupt
    );
    let dataset =
        nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(single.as_bytes()))
            .unwrap();
    let axis = &dataset.descriptor().axes()[0];
    assert_eq!(axis.coordinates(), &AxisCoordinates::Explicit(vec![0.0]));
    assert_eq!(axis.coordinate_span(), None);
    assert_eq!(axis.spectral_width_hz(), None);
}

#[test]
fn ppm_jcamp_preserves_declared_coordinates_without_invented_frequency() {
    for unit in ["HZ", "PPM"] {
        for encoded in ["1 2 3", "ABC"] {
            let text =
                compressed_jcamp(encoded, 3).replace("##XUNITS=HZ", &format!("##XUNITS={unit}"));
            let text = text
                .lines()
                .filter(|line| !line.starts_with("##.OBSERVE"))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("ppm.dx");
            fs::write(&path, &text).unwrap();
            let input = nmr::read(&path).unwrap();
            let p = input.as_processed().unwrap();
            assert_eq!(p.data().samples(), [1.0, 2.0, 3.0]);
            let a = &p.descriptor().axes()[0];
            assert_eq!(
                a.unit(),
                Some(if unit == "PPM" {
                    AxisUnit::Ppm
                } else {
                    AxisUnit::Hertz
                })
            );
            assert_eq!(
                a.coordinates(),
                &AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0
                }
            );
            assert_eq!(a.nucleus(), None);
            assert_eq!(
                a.frequency_evidence()
                    .and_then(|f| f.observe_frequency_mhz()),
                None
            );
        }
    }
}

#[test]
fn jcamp_packed_sqz_dif_dup_and_difdup_have_stable_decoder_state() {
    for (encoded, expected) in [
        ("+10+11+12", vec![10.0, 11.0, 12.0]),
        ("A0A1A2", vec![10.0, 11.0, 12.0]),
        ("A0JJ", vec![10.0, 11.0, 12.0]),
        ("A0A1T", vec![10.0, 11.0, 11.0]),
        ("A0A1U", vec![10.0, 11.0, 11.0, 11.0]),
        ("A0JT", vec![10.0, 11.0, 12.0]),
        ("A0JU", vec![10.0, 11.0, 12.0, 13.0]),
        ("1E+1 1.1E+1 1.2E+1", vec![10.0, 11.0, 12.0]),
    ] {
        let text = compressed_jcamp(encoded, expected.len());
        let dataset =
            nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(text.as_bytes()))
                .unwrap();
        assert_eq!(dataset.data().samples(), expected, "encoded={encoded}");
    }
}

#[test]
fn jcamp_multiline_dif_checks_x_and_y_and_requires_end() {
    let text = include_str!("../../fixtures/jcamp_dx/scaled-dif.dx");
    let dataset =
        nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(text.as_bytes()))
            .unwrap();
    assert_eq!(dataset.data().samples(), &[10.0, 11.0, 12.0, 13.0]);
    assert_eq!(
        dataset.descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Uniform {
            start: 100.0,
            step: 1.0,
        }
    );

    let bad_y_check = text.replace("1010 A1JJ", "1010 A2JJ");
    assert_eq!(
        nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
            bad_y_check.as_bytes(),
        ))
        .unwrap_err()
        .kind(),
        ReadErrorKind::Corrupt
    );

    let missing_end = text.replace("##END=\n", "");
    assert_eq!(
        nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
            missing_end.as_bytes(),
        ))
        .unwrap_err()
        .kind(),
        ReadErrorKind::Incomplete
    );

    let trailing_block = format!("{text}##TITLE=second block\n");
    assert_eq!(
        nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
            trailing_block.as_bytes(),
        ))
        .unwrap_err()
        .kind(),
        ReadErrorKind::Corrupt
    );

    for invalid in [
        text.replace("##XFACTOR=0.1", "##XFACTOR=0"),
        text.replace("##YFACTOR=1", "##YFACTOR=0"),
        text.replace("##END=", "##END=unexpected"),
    ] {
        assert_eq!(
            nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(
                invalid.as_bytes(),
            ))
            .unwrap_err()
            .kind(),
            ReadErrorKind::Corrupt
        );
    }
}

#[test]
fn jcamp_path_read_enforces_working_bytes_before_materialization() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("limited.dx");
    let text = compressed_jcamp("A0JJ", 3);
    fs::write(&path, &text).unwrap();
    assert_limit(
        ReadOptions::new()
            .limits(ReadLimits::new().max_working_bytes(text.len() - 1))
            .read(path)
            .unwrap_err(),
        ReadResource::WorkingBytes,
    );
}

#[test]
fn jcamp_working_budget_counts_line_index_without_token_array_amplification() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("budget.dx");
    for points in [3, 4096] {
        // Each one-byte SQZ token represents one f64 result. No token array is
        // needed: working space is source text plus the reserved line index.
        let text = compressed_jcamp(&"A".repeat(points), points);
        fs::write(&path, &text).unwrap();
        let required = text.len() + text.lines().count() * size_of::<&str>();
        assert_limit(
            ReadOptions::new()
                .limits(ReadLimits::new().max_working_bytes(required - 1))
                .read(&path)
                .unwrap_err(),
            ReadResource::WorkingBytes,
        );
        let loaded = ReadOptions::new()
            .limits(
                ReadLimits::new()
                    .max_working_bytes(required)
                    .max_materialized_bytes(points * 8),
            )
            .read(&path)
            .unwrap();
        let data = loaded.as_processed().unwrap().data();
        assert_eq!(data.shape(), [points]);
        assert_eq!(data.samples(), vec![1.0; points]);
        assert_limit(
            ReadOptions::new()
                .limits(
                    ReadLimits::new()
                        .max_working_bytes(required)
                        .max_materialized_bytes(points * 8 - 1),
                )
                .read(&path)
                .unwrap_err(),
            ReadResource::MaterializedBytes,
        );
    }
}
