use crate::group_delay_inputs as inputs;
use nmr::processed::ProcessedGroupDelay;
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

fn roundtrip(input: &nmr::Dataset) -> nmr::Dataset {
    let mut bytes = Vec::new();
    snapshot::write_snapshot(input, &mut bytes, Default::default()).unwrap();
    let restored = snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(snapshot::AcceptRecordedHistory);
    assert_eq!(restored.canonical_digests(), input.canonical_digests());
    restored
}

fn assert_processed(input: &nmr::Dataset, corrected: bool) {
    match input
        .as_processed()
        .unwrap()
        .axis_evidence(0)
        .unwrap()
        .group_delay()
    {
        ProcessedGroupDelay::Pending(delay) if !corrected => {
            assert!((delay.delay_points() - 71.625).abs() < 1e-9);
            assert_rule(delay.evidence(), TABLE_RULE);
        }
        ProcessedGroupDelay::Corrected {
            delay_points,
            evidence: Some(evidence),
        } if corrected => {
            assert!((delay_points - 71.625).abs() < 1e-9);
            assert_rule(evidence, TABLE_RULE);
        }
        other => panic!("unexpected filter state: {other:?}"),
    }
    let mut report = Vec::new();
    nmr::execution_report::write_json(input.as_processed().unwrap(), &[], &mut report, 1_000_000)
        .unwrap();
    let report = String::from_utf8(report).unwrap();
    assert!(report.contains(TABLE_RULE));
    assert!(report.contains("71.625"));
    assert!(report.contains("acqus"));
}

#[test]
fn table_evidence_survives_effective_state_snapshot_execution_and_offline_replay() {
    let directory = tempfile::tempdir().unwrap();
    inputs::write_input(directory.path(), inputs::TABLE_PARAMETERS).unwrap();
    let raw = nmr::read(directory.path()).unwrap();
    let pending = inputs::fft().apply(&raw).unwrap();
    let corrected = inputs::correction().apply(&pending).unwrap();
    inputs::write_input(directory.path(), "##$GRPDLY= 71.625\n").unwrap();
    let explicit = nmr::read(directory.path()).unwrap();
    let expected = inputs::correction()
        .apply(&inputs::fft().apply(&explicit).unwrap())
        .unwrap();
    assert_eq!(
        corrected.as_processed().unwrap().data(),
        expected.as_processed().unwrap().data()
    );
    directory.close().unwrap();
    let raw = roundtrip(&raw);
    assert_delay(&raw, 71.625, TABLE_RULE);
    for (dataset, is_corrected) in [(&pending, false), (&corrected, true)] {
        let restored = roundtrip(dataset);
        assert_processed(&restored, is_corrected);
        let history = restored
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        let replayed = history.replay(&[&raw], Default::default()).unwrap();
        assert_eq!(replayed.canonical_digests(), dataset.canonical_digests());
        assert!(history.replay(&[&explicit], Default::default()).is_err());
    }
}
