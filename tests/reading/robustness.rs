use nmr::raw::ReadErrorKind;
use nmr::raw::{OpenOptions, RawFormat as Format, ReadLimits};
use std::fs;

fn damaged_bytes(length: usize) -> Vec<u8> {
    let mut state = 0x9e37_79b9_u32 ^ length as u32;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

#[test]
fn arbitrary_damaged_inputs_do_not_panic() {
    for length in 0..=2048 {
        let bytes = damaged_bytes(length);
        let text = String::from_utf8_lossy(&bytes);
        let one_parameter = [text.as_ref()];
        let two_parameters = [text.as_ref(), text.as_ref()];

        let _ = nmr::formats::jeol::read_parts(nmr::formats::jeol::Parts::new(&bytes));
        let _ = nmr::formats::bruker::read_parts(nmr::formats::bruker::Parts::new(
            &bytes,
            &one_parameter,
        ));
        let _ = nmr::formats::bruker::read_parts(
            nmr::formats::bruker::Parts::new(&bytes, &two_parameters).nuslist(&text),
        );
        let _ = nmr::formats::varian::read_parts(
            nmr::formats::varian::Parts::new(&bytes, &text).sampling_schedule(&text),
        );
        let _ = nmr::formats::jcamp_dx::read_parts(nmr::formats::jcamp_dx::Parts::new(&bytes));
    }
}

fn mutation_limits() -> ReadLimits {
    ReadLimits::new()
        .max_working_bytes(1024 * 1024)
        .max_materialized_bytes(64 * 1024)
        .max_metadata_bytes(64 * 1024)
}

#[test]
fn structured_jcamp_record_and_compression_mutations_remain_bounded() {
    use nmr::formats::jcamp_dx::{Parts, read_parts_with_limits};
    let seed = include_str!("../fixtures/jcamp_dx/scaled-dif.dx");
    read_parts_with_limits(Parts::new(seed.as_bytes()), mutation_limits()).unwrap();
    let mut variants = Vec::new();
    for (record, original) in [
        ("NPOINTS", "4"),
        ("FIRSTX", "100"),
        ("LASTX", "103"),
        ("XFACTOR", "0.1"),
        ("YFACTOR", "1"),
        ("DELTAX", "1"),
    ] {
        for replacement in ["0", "-1", "1", "18446744073709551615", "NaN", "1e309", ""] {
            variants.push(seed.replace(
                &format!("##{record}={original}"),
                &format!("##{record}={replacement}"),
            ));
        }
        variants.push(seed.replace("##XYDATA", &format!("##{record}={original}\n##XYDATA")));
    }
    for replacement in [
        "1010 A2JJ",
        "1010 A1J%",
        "1010 A1S000000",
        "1010 A1JZ",
        "1010 ?",
        "1010 A1JJJ",
        "1010 A1",
        "",
    ] {
        variants.push(seed.replace("1010 A1JJ", replacement));
    }
    variants.push(seed.replace("##END=", ""));
    variants.push(seed.replace("##END=", "##END=\n##NPOINTS=99"));
    for end in seed.match_indices('\n').map(|(index, _)| index) {
        variants.push(seed[..end].to_owned());
    }
    let mut rejected = 0;
    for text in variants {
        match read_parts_with_limits(Parts::new(text.as_bytes()), mutation_limits()) {
            Ok(dataset) => {
                dataset.validate().unwrap();
                assert!(std::mem::size_of_val(dataset.data().samples()) <= 64 * 1024);
            }
            Err(_) => rejected += 1,
        }
    }
    assert!(rejected >= 40, "mutations must exercise rejection paths");
}

fn structured_binary_mutations(
    seed: &[u8],
    offsets: &[usize],
    read: impl Fn(&[u8]) -> Result<nmr::raw::RawDataset, nmr::ReadError>,
) {
    read(seed).unwrap();
    let mut rejected = 0;
    for &offset in offsets {
        for value in [0_u32, 1, 2, 4, 0x7fff_ffff, 0xffff_ffff] {
            for encoded in [value.to_be_bytes(), value.to_le_bytes()] {
                let mut bytes = seed.to_vec();
                bytes[offset..offset + 4].copy_from_slice(&encoded);
                if let Ok(dataset) = read(&bytes) {
                    if let Some(samples) = dataset.data().dense_samples() {
                        assert!(std::mem::size_of_val(samples) <= 64 * 1024);
                        assert!(
                            samples
                                .iter()
                                .all(|value| value.re.is_finite() && value.im.is_finite())
                        );
                    }
                } else {
                    rejected += 1;
                }
            }
        }
    }
    for length in [0, 8, 16, 31, seed.len() / 2, seed.len() - 1] {
        if read(&seed[..length]).is_err() {
            rejected += 1;
        }
    }
    assert!(rejected >= 20);
}

#[test]
fn structured_jeol_header_lengths_offsets_and_types_remain_bounded() {
    // Keep magic and the valid seed layout, then perturb specific header fields.
    structured_binary_mutations(
        include_bytes!("../fixtures/jeol/axis-3.jdf"),
        &[8, 24, 32, 176, 1212, 1216, 1284, 1288],
        |bytes| {
            nmr::formats::jeol::read_parts_with_limits(
                nmr::formats::jeol::Parts::new(bytes).allow_experimental_vendor_semantics(true),
                mutation_limits(),
            )
        },
    );
}

#[test]
fn structured_varian_header_and_parameter_mutations_remain_bounded() {
    let seed = include_bytes!("../fixtures/varian/complex.fid");
    let procpar = "np 1 1 4 0 0 2 1 0 1 64\n1 4\n0\n";
    structured_binary_mutations(seed, &[0, 4, 8, 12, 16, 20, 24, 28], |bytes| {
        nmr::formats::varian::read_parts_with_limits(
            nmr::formats::varian::Parts::new(bytes, procpar),
            mutation_limits(),
        )
    });
    for values in [
        "0",
        "1 NaN",
        "1 -1",
        "2 4 4",
        "18446744073709551615 4",
        "1 4\n0\nnp 1 1 4 0 0 2 1 0 1 64\n1 8",
    ] {
        let mutated = procpar.replace("1 4\n0", &format!("{values}\n0"));
        let _ = nmr::formats::varian::read_parts_with_limits(
            nmr::formats::varian::Parts::new(seed, &mutated),
            mutation_limits(),
        );
    }
}

#[test]
fn structured_bruker_shape_encoding_and_duplicate_metadata_remain_bounded() {
    let seed = include_bytes!("../fixtures/bruker/one-dimensional.fid");
    let acqus = "##$TD= 4\n##$PARMODE= 0\n##$AQ_mod= 3\n##$BYTORDA= 0\n##$DTYPA= 0\n##$SW_h= 8000\n##$SFO1= 400\n##END=\n";
    nmr::formats::bruker::read_parts_with_limits(
        nmr::formats::bruker::Parts::new(seed, &[acqus]),
        mutation_limits(),
    )
    .unwrap();
    for (record, old) in [
        ("TD", "4"),
        ("PARMODE", "0"),
        ("AQ_mod", "3"),
        ("BYTORDA", "0"),
        ("DTYPA", "0"),
        ("SW_h", "8000"),
    ] {
        for value in [
            "-1",
            "0",
            "1",
            "3",
            "18446744073709551615",
            "NaN",
            "(0..3)\n0 1 2 3",
        ] {
            let text = acqus.replace(
                &format!("##${record}= {old}"),
                &format!("##${record}= {value}"),
            );
            let _ = nmr::formats::bruker::read_parts_with_limits(
                nmr::formats::bruker::Parts::new(seed, &[&text]),
                mutation_limits(),
            );
        }
        let duplicate = acqus.replace("##END=", &format!("##${record}= 999\n##END="));
        let _ = nmr::formats::bruker::read_parts_with_limits(
            nmr::formats::bruker::Parts::new(seed, &[&duplicate]),
            mutation_limits(),
        );
    }
}

#[test]
fn combined_varian_metadata_is_limited() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("fid"), []).unwrap();
    fs::write(temp.path().join("procpar"), "1234").unwrap();
    fs::write(temp.path().join("sampling.sch"), "5678").unwrap();
    let error = OpenOptions::new()
        .limits(ReadLimits::new().max_metadata_bytes(7))
        .open(temp.path())
        .unwrap_err();
    assert_eq!(error.format(), Some(nmr::Format::Raw(Format::VarianRaw)));
    assert_eq!(error.kind(), ReadErrorKind::LimitExceeded);
}

#[test]
fn bruker_working_limit_is_checked_before_metadata_text_loading() {
    let temp = tempfile::tempdir().unwrap();
    let acqus = "##$TD= 4\n##$PARMODE= 0\n##$AQ_mod= 3\n##$BYTORDA= 0\n##$DTYPA= 0\n##$SW_h= 8000\n##$SFO1= 400\n##END=\n";
    fs::write(temp.path().join("fid"), [0; 16]).unwrap();
    fs::write(temp.path().join("acqus"), acqus).unwrap();
    let error = OpenOptions::new()
        .limits(ReadLimits::new().max_working_bytes(47))
        .open(temp.path())
        .unwrap_err();
    assert_eq!(error.format(), Some(nmr::Format::Raw(Format::BrukerRaw)));
    assert_eq!(error.kind(), ReadErrorKind::LimitExceeded);
}

#[test]
fn combined_bruker_metadata_is_limited_before_parsing() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("ser"), []).unwrap();
    fs::write(temp.path().join("acqus"), "1234").unwrap();
    fs::write(temp.path().join("acqu2s"), "5678").unwrap();
    fs::write(temp.path().join("nuslist"), "9012").unwrap();
    let error = OpenOptions::new()
        .allow_experimental_vendor_semantics(true)
        .limits(ReadLimits::new().max_metadata_bytes(11))
        .open(temp.path())
        .unwrap_err();
    assert_eq!(error.format(), Some(nmr::Format::Raw(Format::BrukerRaw)));
    assert_eq!(error.kind(), ReadErrorKind::LimitExceeded);
}
