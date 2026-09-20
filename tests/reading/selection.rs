use nmr::processed::Format as ProcessedFormat;
use nmr::raw::RawFormat;
use nmr::{Format, ReadErrorKind, ReadOptions, ReadPreference};
use std::fs;

use super::support::*;

#[test]
fn resolver_applies_preference_hint_and_selected_path_rules() {
    let temporary = tempfile::tempdir().unwrap();
    let experiment = temporary.path().join("subject").join("7");
    let procno = experiment.join("pdata").join("3");
    fs::create_dir_all(&procno).unwrap();
    fs::write(experiment.join("fid"), []).unwrap();
    fs::write(experiment.join("acqus"), "##$PARMODE= 0\n").unwrap();
    fs::write(procno.join("procs"), processed_parameters(1)).unwrap();
    write_i32(&procno.join("1r"), &[1]);

    assert_eq!(
        nmr::detect(&experiment).unwrap_err().kind(),
        ReadErrorKind::Ambiguous
    );
    assert_eq!(
        nmr::read(&experiment).unwrap_err().kind(),
        ReadErrorKind::Ambiguous
    );
    assert_eq!(
        ReadOptions::new()
            .preference(ReadPreference::PreferProcessed)
            .detect(&experiment)
            .unwrap(),
        Format::Processed(ProcessedFormat::BrukerTopSpin)
    );
    assert_eq!(
        ReadOptions::new()
            .preference(ReadPreference::PreferRaw)
            .detect(&experiment)
            .unwrap(),
        Format::Raw(RawFormat::BrukerRaw)
    );
    assert_eq!(
        nmr::detect(experiment.join("fid")).unwrap(),
        Format::Raw(RawFormat::BrukerRaw)
    );
    assert_eq!(
        nmr::detect(experiment.join("pdata")).unwrap(),
        Format::Processed(ProcessedFormat::BrukerTopSpin)
    );

    let mismatch = ReadOptions::new()
        .format(Format::Raw(RawFormat::VarianRaw))
        .detect(&procno)
        .unwrap_err();
    assert_eq!(mismatch.kind(), ReadErrorKind::FormatMismatch);
}

#[test]
fn resolver_prefers_structurally_complete_candidates_before_representation_kind() {
    for (name, two_d) in [("one-dimensional", false), ("two-dimensional", true)] {
        let temporary = tempfile::tempdir().unwrap();
        let experiment = temporary.path().join(name);
        let procno = experiment.join("pdata").join("1");
        fs::create_dir_all(&procno).unwrap();
        if two_d {
            write_complete_bruker_raw_2d(&experiment);
            fs::write(procno.join("procs"), processed_parameters(2)).unwrap();
            fs::write(procno.join("proc2s"), processed_parameters(2)).unwrap();
        } else {
            write_complete_bruker_raw_1d(&experiment);
            fs::write(procno.join("procs"), processed_parameters(2)).unwrap();
        }

        for preference in [
            ReadPreference::PreferProcessed,
            ReadPreference::PreferRaw,
            ReadPreference::RequireUnique,
        ] {
            let loaded = ReadOptions::new()
                .preference(preference)
                .read(&experiment)
                .unwrap();
            assert_eq!(
                loaded.source_format(),
                Some(Format::Raw(RawFormat::BrukerRaw))
            );
        }

        assert_eq!(
            nmr::read(&procno).unwrap_err().kind(),
            ReadErrorKind::Incomplete
        );
        assert_eq!(
            ReadOptions::new()
                .format(Format::Processed(ProcessedFormat::BrukerTopSpin))
                .read(&experiment)
                .unwrap_err()
                .kind(),
            ReadErrorKind::Incomplete
        );
    }
}

#[test]
fn structurally_complete_unsupported_processed_candidate_does_not_fallback_to_raw() {
    let temporary = tempfile::tempdir().unwrap();
    let experiment = temporary.path().join("unsupported-processed");
    let procno = experiment.join("pdata").join("1");
    fs::create_dir_all(&procno).unwrap();
    write_complete_bruker_raw_1d(&experiment);

    let mut direct = processed_parameters_with_storage(2, 1, 0, 1);
    direct.push_str("##$XDIM= 1\n");
    let mut indirect = processed_parameters(2);
    indirect.push_str("##$XDIM= 2\n");
    fs::write(procno.join("procs"), direct).unwrap();
    fs::write(procno.join("proc2s"), indirect).unwrap();
    write_i32(&procno.join("2rr"), &[1, 2, 3, 4]);

    let error = ReadOptions::new()
        .preference(ReadPreference::PreferProcessed)
        .read(&experiment)
        .unwrap_err();
    assert_eq!(
        error.format(),
        Some(Format::Processed(ProcessedFormat::BrukerTopSpin))
    );
    assert_eq!(error.kind(), ReadErrorKind::UnsupportedFeature);
}

#[test]
fn same_kind_ambiguity_incomplete_and_missing_paths_are_typed() {
    let temporary = tempfile::tempdir().unwrap();
    let pdata = temporary.path().join("pdata");
    for procno in ["1", "2"] {
        let root = pdata.join(procno);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("procs"), processed_parameters(1)).unwrap();
        write_i32(&root.join("1r"), &[1]);
    }
    assert_eq!(
        nmr::detect(&pdata).unwrap_err().kind(),
        ReadErrorKind::Ambiguous
    );
    assert_eq!(
        nmr::raw::detect(&pdata).unwrap_err().kind(),
        ReadErrorKind::Unrecognized
    );

    let incomplete = temporary.path().join("incomplete");
    fs::create_dir(&incomplete).unwrap();
    write_i32(&incomplete.join("1r"), &[1]);
    assert_eq!(
        nmr::detect(&incomplete).unwrap(),
        Format::Processed(ProcessedFormat::BrukerTopSpin)
    );
    assert_eq!(
        nmr::read(&incomplete).unwrap_err().kind(),
        ReadErrorKind::Incomplete
    );

    assert_eq!(
        nmr::detect(temporary.path().join("absent"))
            .unwrap_err()
            .kind(),
        ReadErrorKind::Io
    );
}

#[test]
fn detect_selects_a_recognized_layout_without_promising_read_support() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let mut direct = processed_parameters_with_storage(2, 1, 0, 1);
    direct.push_str("##$XDIM= 1\n");
    let mut indirect = processed_parameters(2);
    indirect.push_str("##$XDIM= 2\n");
    fs::write(root.join("procs"), direct).unwrap();
    fs::write(root.join("proc2s"), indirect).unwrap();
    write_i32(&root.join("2rr"), &[1, 2, 3, 4]);

    assert_eq!(
        nmr::detect(root.join("2rr")).unwrap(),
        Format::Processed(ProcessedFormat::BrukerTopSpin)
    );
    assert_eq!(
        nmr::read(root.join("2rr")).unwrap_err().kind(),
        ReadErrorKind::UnsupportedFeature
    );
}

#[test]
fn complete_bruker_1d_and_2d_candidates_are_ambiguous_unless_primary_is_selected() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let mut direct = processed_parameters(2);
    direct.push_str("##$XDIM= 2\n");
    fs::write(root.join("procs"), direct).unwrap();
    fs::write(
        root.join("proc2s"),
        "##$SI= 2\n##$XDIM= 2\n##$DTYPP= 0\n##$BYTORDP= 0\n##$NC_proc= 1\n",
    )
    .unwrap();
    write_i32(&root.join("1r"), &[1, 2]);
    write_i32(&root.join("2rr"), &[1, 2, 3, 4]);

    assert_eq!(
        nmr::detect(root).unwrap_err().kind(),
        ReadErrorKind::Ambiguous
    );
    assert_eq!(
        nmr::detect(root.join("1r")).unwrap(),
        Format::Processed(ProcessedFormat::BrukerTopSpin)
    );
    assert_eq!(
        nmr::detect(root.join("2rr")).unwrap(),
        Format::Processed(ProcessedFormat::BrukerTopSpin)
    );
}
