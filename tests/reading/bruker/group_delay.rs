use crate::group_delay_inputs as inputs;
use nmr::raw::{GroupDelayState, NormalizationEvidence, ResolutionAuthority};
use nmr::snapshot;

const TABLE_RULE: &str = "bruker.dspfvs-decim-table.v1";

fn assert_rule(evidence: &NormalizationEvidence, expected: &str) {
    assert!(
        matches!(evidence.authority(), ResolutionAuthority::FormatRule(rule) if rule.as_str() == expected)
    );
    assert_eq!(
        evidence.derivation()[0].as_str(),
        "canonical.group-delay.v1"
    );
}

fn assert_delay(input: &nmr::Dataset, expected: f64, rule: &str) {
    match input.as_raw().unwrap().descriptor().axes()[0].group_delay() {
        GroupDelayState::Pending(delay) => {
            assert!((delay.delay_points() - expected).abs() < 1e-9);
            assert_rule(delay.evidence(), rule);
        }
        other => panic!("expected {expected} points, got {other:?}"),
    }
}

#[test]
fn public_reader_uses_original_table_for_missing_and_sentinel_grpdly() {
    let directory = tempfile::tempdir().unwrap();
    for grpdly in ["", "##$GRPDLY= -1\n"] {
        for (version, decimation, expected) in [
            (10, 2, 44.75),
            (11, 16, 72.25),
            (12, 16, 71.625),
            (13, 96, 2.9947916666666665),
        ] {
            let parameters = format!("{grpdly}##$DSPFVS= {version}\n##$DECIM= {decimation}\n");
            inputs::write_input(directory.path(), &parameters).unwrap();
            let raw = nmr::read(directory.path()).unwrap();
            assert_delay(&raw, expected, TABLE_RULE);
            let restored = roundtrip(&raw);
            assert_delay(&restored, expected, TABLE_RULE);
            let parameters = restored
                .as_raw()
                .unwrap()
                .provenance()
                .source_metadata()
                .as_bruker()
                .unwrap()
                .direct();
            assert_eq!(parameters.get("DSPFVS"), Some(version.to_string().as_str()));
            assert_eq!(
                parameters.get("DECIM"),
                Some(decimation.to_string().as_str())
            );
            assert_eq!(
                parameters.get("GRPDLY"),
                if grpdly.is_empty() { None } else { Some("-1") }
            );
            assert_eq!(
                restored.as_raw().unwrap().provenance().source_metadata(),
                raw.as_raw().unwrap().provenance().source_metadata()
            );
        }
    }
}

#[test]
fn explicit_grpdly_keeps_original_precision_zero_and_priority() {
    let directory = tempfile::tempdir().unwrap();
    for (value, expected) in [("67.98", 67.98), ("0", 0.0)] {
        for hardware in [
            "##$DSPFVS= 21\n##$DECIM= 2080\n",
            "##$DSPFVS= 12\n##$DECIM= 16\n",
            "##$DSPFVS= invalid\n##$DECIM= invalid\n",
        ] {
            inputs::write_input(directory.path(), &format!("##$GRPDLY= {value}\n{hardware}"))
                .unwrap();
            assert_delay(
                &nmr::read(directory.path()).unwrap(),
                expected,
                "bruker.direct.v1",
            );
        }
    }
}

#[test]
fn unsupported_or_incomplete_hardware_is_unknown_without_integer_wraparound() {
    let directory = tempfile::tempdir().unwrap();
    for grpdly in ["", "##$GRPDLY= -1\n"] {
        for hardware in [
            "",
            "##$DSPFVS= 12\n",
            "##$DECIM= 16\n",
            "##$DSPFVS= 21\n##$DECIM= 2080\n",
            "##$DSPFVS= 13\n##$DECIM= 128\n",
            "##$DSPFVS= 12\n##$DECIM= 0\n",
            "##$DSPFVS= 12\n##$DECIM= -16\n",
            "##$DSPFVS= -12\n##$DECIM= 16\n",
            "##$DSPFVS= 4294967308\n##$DECIM= 16\n",
            "##$DSPFVS= 12\n##$DECIM= 4294967312\n",
        ] {
            inputs::write_input(directory.path(), &format!("{grpdly}{hardware}")).unwrap();
            let input = nmr::read(directory.path()).unwrap();
            assert_eq!(
                input.as_raw().unwrap().descriptor().axes()[0].group_delay(),
                &GroupDelayState::Unknown
            );
            assert!(
                inputs::correction()
                    .apply(&inputs::fft().apply(&input).unwrap())
                    .is_err()
            );
        }
    }
}

#[test]
fn malformed_hardware_and_illegal_grpdly_remain_checked_errors() {
    use nmr::raw::{ParameterErrorKind, ReadErrorReason};
    let directory = tempfile::tempdir().unwrap();
    for (parameters, field) in [
        ("##$GRPDLY= -2\n##$DSPFVS= 12\n##$DECIM= 16\n", "GRPDLY"),
        ("##$GRPDLY= NaN\n", "GRPDLY"),
        ("##$GRPDLY= inf\n", "GRPDLY"),
        ("##$GRPDLY= -1\n##$DSPFVS= 12.5\n##$DECIM= 16\n", "DSPFVS"),
        ("##$GRPDLY= -1\n##$DSPFVS= 12\n##$DECIM= invalid\n", "DECIM"),
        ("##$DECIM= 16.5\n", "DECIM"),
        ("##$DSPFVS= invalid\n", "DSPFVS"),
    ] {
        inputs::write_input(directory.path(), parameters).unwrap();
        let error = nmr::read(directory.path()).unwrap_err();
        assert!(
            matches!(error.reason(), ReadErrorReason::InvalidMetadata { parameter: Some(parameter), .. }
            if parameter.parameter() == Some(field) && parameter.kind() == ParameterErrorKind::Invalid),
            "{error:?}"
        );
    }
}

fn roundtrip(input: &nmr::Dataset) -> nmr::Dataset {
    let mut bytes = Vec::new();
    snapshot::write_snapshot(input, &mut bytes, Default::default()).unwrap();
    let restored = snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(snapshot::AcceptRecordedHistory);
    assert_eq!(restored.canonical_digests(), input.canonical_digests());
    restored
}
