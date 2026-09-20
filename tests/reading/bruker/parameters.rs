use nmr::formats::bruker::ParameterFile;
use nmr::raw::{ParameterErrorKind, ReadErrorReason};

use super::support::*;

#[test]
fn retains_array_valued_vendor_parameters() {
    let parameters =
        ParameterFile::parse("##$TD= 4\n##$D= (0..2)\n0.1 0.2\n0.3\n##$NUC1= <1H>\n##END=\n")
            .unwrap();

    assert_eq!(parameters.get("D"), Some("(0..2)\n0.1 0.2\n0.3"));
    assert_eq!(parameters.get("NUC1"), Some("<1H>"));
}

#[test]
fn parameter_table_keeps_sorted_lookup_exact_text_and_duplicate_rejection() {
    let mut text = String::from("##TITLE= table\r\n");
    for index in (0..128).rev() {
        text.push_str(&format!("##$P{index:03}= <value {index}>\r\n"));
    }
    text.push_str("##$ARRAY= (0..1)\r\n1\r\n$$ comment\r\n2\r\n##END=\r\n");
    let parameters = ParameterFile::parse(&text).unwrap();
    assert_eq!(parameters.raw_text(), text);
    assert_eq!(parameters.title(), Some("table"));
    assert_eq!(parameters.get("ARRAY"), Some("(0..1)\n1\n2"));
    assert_eq!(parameters.get("P000"), Some("<value 0>"));
    assert_eq!(parameters.get("P127"), Some("<value 127>"));
    assert_eq!(parameters.get("p000"), None);
    assert_eq!(parameters.get("missing"), None);
    let names: Vec<_> = parameters.iter().map(|(name, _)| name).collect();
    assert_eq!(names.len(), 129);
    assert_eq!(names[0], "ARRAY");
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    let error = ParameterFile::parse("##$Z= 1\n##$A= 2\n##$Z= 3\n").unwrap_err();
    assert_eq!(error.kind(), ParameterErrorKind::Duplicate);
    assert_eq!(error.parameter(), Some("Z"));
    assert!(
        ParameterFile::parse("$$ comment only\n")
            .unwrap()
            .iter()
            .next()
            .is_none()
    );
}

#[test]
fn parameter_continuations_preserve_normalization_across_long_values() {
    let mut text = String::from("##$A= \r\n\r\n$$ ignored\r\n");
    let expected = (0..4096)
        .map(|value| format!("值 {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    for value in 0..4096 {
        text.push_str(&format!("  值 {value}  \r\n$$ ignored\r\n"));
    }
    text.push_str("##GLOBAL= opaque\r\nnot A\r\n##$B=\r\n##END=\r\n");
    let parameters = ParameterFile::parse(&text).unwrap();
    assert_eq!(parameters.get("A"), Some(expected.as_str()));
    assert_eq!(parameters.get("B"), Some(""));
    assert_eq!(parameters.raw_text(), text);
    assert_eq!(parameters.clone(), parameters);
}

#[test]
fn retains_global_jcamp_title() {
    let parameters =
        ParameterFile::parse("##TITLE= acquisition title\n##$TD= 4\n##END=\n").unwrap();
    assert_eq!(parameters.title(), Some("acquisition title"));
}

#[test]
fn group_delay_preserves_zero_and_minus_one_sentinel_but_rejects_other_negatives() {
    for (text, expected) in [("0", Some(0.0)), ("-1", None)] {
        let acqus = parameter_text(0, 0, 4).replace("44.75", text);
        let dataset = read_from_parts(&[0; 16], &[&acqus], None).unwrap();
        assert_eq!(pending_delay(&dataset.descriptor().axes()[0]), expected);
    }

    let acqus = parameter_text(0, 0, 4).replace("44.75", "-2");
    let error = read_from_parts(&[0; 16], &[&acqus], None).unwrap_err();
    assert!(matches!(
        error.reason(),
        ReadErrorReason::InvalidMetadata { parameter: Some(parameter), .. }
            if parameter.parameter() == Some("GRPDLY")
                && parameter.kind() == ParameterErrorKind::Invalid
    ));
}

#[test]
fn historical_bruker_delay_table_resolves_supported_hardware_parameters() {
    for missing in [true, false] {
        let original = parameter_text(0, 0, 4);
        let parameters = if missing {
            original.replace("##$GRPDLY= 44.75\n", "")
        } else {
            original.replace("44.75", "-1")
        };
        let parameters = parameters.replace("##END=", "##$DSPFVS= 10\n##$DECIM= 2\n##END=");
        let dataset = read_from_parts(&[0; 16], &[&parameters], None).unwrap();
        assert_eq!(pending_delay(&dataset.descriptor().axes()[0]), Some(44.75));
    }
}

#[test]
fn rejects_malformed_parameter_record() {
    let error = ParameterFile::parse("##$TD 4\n").unwrap_err();
    assert_eq!(error.kind(), ParameterErrorKind::Malformed);
    assert_eq!(error.input_source().role(), Some("acqus"));
    assert_eq!(error.input_source().path(), None);
}

#[test]
fn retains_raw_parameters_only_in_source_provenance() {
    let acqus = parameter_text(0, 0, 4);
    let dataset = read_from_parts(&[0; 16], &[&acqus], None).unwrap();
    let parameters = dataset.provenance().source_metadata().as_bruker().unwrap();
    assert_eq!(parameters.direct().get("O1"), Some("1880"));
    assert_eq!(parameters.direct().get("BF1"), Some("400.0"));
    assert!(parameters.indirect(0).is_none());
}
