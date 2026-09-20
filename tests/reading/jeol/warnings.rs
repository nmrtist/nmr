use super::support::*;
use nmr::{Dataset, ReadOptions, ReadWarning};

fn load(bytes: &[u8]) -> Dataset {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("warning.jdf");
    std::fs::write(&path, bytes).unwrap();
    assert!(ReadOptions::new().read(&path).is_err());
    ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read(path)
        .unwrap()
}

pub(super) fn details(dataset: &Dataset) -> &[String] {
    let warnings: Vec<_> = dataset
        .warnings()
        .iter()
        .filter_map(|warning| match warning {
            ReadWarning::ExperimentalVendorSemantics { details, .. } => Some(details),
            _ => None,
        })
        .collect();
    assert_eq!(warnings.len(), 1);
    assert!(!warnings[0].is_empty());
    warnings[0]
}

#[test]
fn audited_layouts_keep_scoped_warnings_and_synthetic_layouts_are_not_promoted() {
    for (bytes, axes, coverage) in [
        (
            include_bytes!("../../fixtures/jeol/axis-1.jdf").as_slice(),
            "[1]",
            "synthetic regression",
        ),
        (
            include_bytes!("../../fixtures/jeol/axis-3.jdf").as_slice(),
            "[3]",
            "selected proton",
        ),
        (
            include_bytes!("../../fixtures/jeol/axes-3-1.jdf").as_slice(),
            "[3, 1]",
            "synthetic regression",
        ),
        (
            include_bytes!("../../fixtures/jeol/axes-3-3.jdf").as_slice(),
            "[3, 3]",
            "selected HSQC",
        ),
        (
            include_bytes!("../../fixtures/jeol/axes-4-4.jdf").as_slice(),
            "[4, 4]",
            "selected COSY",
        ),
    ] {
        let dataset = load(bytes);
        let warnings = details(&dataset);
        assert!(warnings[0].contains(axes));
        assert!(warnings[0].contains(coverage));
        assert_eq!(warnings.len(), 1);
    }
}

#[test]
fn nus_warning_and_details_survive_offline_snapshot_restore() {
    let dataset = load(&jeol_fixture_hypercomplex_nus());
    assert!(
        details(&dataset)
            .iter()
            .any(|detail| detail.starts_with("NUS"))
    );
    let mut bytes = Vec::new();
    nmr::snapshot::write_snapshot(&dataset, &mut bytes, Default::default()).unwrap();
    let restored = nmr::snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(nmr::snapshot::AcceptRecordedHistory);
    assert_eq!(restored.warnings(), dataset.warnings());
}

#[test]
fn raw_audits_do_not_qualify_processed_frequency_semantics() {
    let bytes = crate::support::processed_jeol_with_axis_evidence(
        3,
        &[1.0, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0],
    );
    let dataset = load(&bytes);
    assert!(dataset.as_processed().is_some());
    assert!(details(&dataset)[0].contains("synthetic regression"));
    assert!(
        details(&dataset)
            .iter()
            .any(|detail| detail.starts_with("Processed"))
    );
    assert!(
        !details(&dataset)
            .iter()
            .any(|detail| detail.starts_with("NUS"))
    );
}

#[test]
fn filter_concern_only_applies_to_an_enabled_vendor_filter() {
    for enabled in ["true", "false"] {
        let bytes = jeol_with_parameter_records(
            &jeol_fixture_f64(),
            &[jeol_parameter_record(
                "digital_filter",
                nmr::formats::jeol::ParameterValue::String(enabled.into()),
                0,
            )],
        );
        let dataset = load(&bytes);
        assert_eq!(
            details(&dataset)
                .iter()
                .any(|detail| detail.starts_with("Digital-filter")),
            enabled == "true"
        );
    }
}
