use super::support::*;
use crate::raw_support::*;
use std::fs;

#[test]
fn jeol_missing_embedded_nus_list_accepts_a_checked_declaration_without_editing_source() {
    let fixture = jeol_fixture_hypercomplex_nus();
    let directory = tempfile::tempdir().unwrap();
    let complete_path = directory.path().join("complete.jdf");
    let missing_path = directory.path().join("missing.jdf");
    fs::write(&complete_path, &fixture).unwrap();
    let mut missing = fixture.clone();
    put_be_u32(&mut missing, 1224, 0); // no embedded y coordinate list
    put_be_u32(&mut missing, 1256, 0);
    fs::write(&missing_path, &missing).unwrap();
    let options = nmr::ReadOptions::new().allow_experimental_vendor_semantics(true);
    assert!(options.read(&missing_path).is_err());
    let declaration = |rows: Vec<Vec<usize>>, lanes| {
        nmr::SamplingDeclaration::new(
            nmr::raw::AssertionId::try_new("jeol-user-list").unwrap(),
            "retained user schedule",
            vec![8],
            rows,
            nmr::SamplingIndexBase::One,
            vec![lanes],
        )
    };
    let declared = declaration(vec![vec![1], vec![2], vec![4], vec![8]], 2);
    let complete = options.read(&complete_path).unwrap();
    let input = options
        .clone()
        .sampling_declaration(declared.clone())
        .read(&missing_path)
        .unwrap();
    assert!(
        super::warnings::details(&input)
            .iter()
            .any(|detail| detail.starts_with("NUS"))
    );
    assert_eq!(
        super::warnings::details(&input),
        super::warnings::details(&complete)
    );
    assert_eq!(fs::read(&missing_path).unwrap(), missing);
    assert_eq!(
        input.as_raw().unwrap().data(),
        complete.as_raw().unwrap().data()
    );
    assert_eq!(
        input.as_raw().unwrap().descriptor().axes(),
        complete.as_raw().unwrap().descriptor().axes()
    );
    assert_eq!(
        input
            .as_raw()
            .unwrap()
            .sampling_schedule()
            .unwrap()
            .declaration(),
        Some(&declared)
    );
    assert!(
        options
            .clone()
            .sampling_declaration(declared)
            .read(&complete_path)
            .is_ok()
    );
    for (declaration, detail) in [
        (
            declaration(vec![vec![1], vec![4], vec![2], vec![8]], 2),
            "sampling declaration disagrees with vendor observation order or coordinates",
        ),
        (
            declaration(vec![vec![1], vec![2], vec![4], vec![8]], 1),
            "sampling declaration conflicts with vendor grid, lane count or observation count",
        ),
    ] {
        let error = options
            .clone()
            .sampling_declaration(declaration)
            .read(&complete_path)
            .unwrap_err();
        assert_eq!(error.kind(), nmr::ReadErrorKind::UnsupportedFeature);
        let nmr::ReadErrorReason::UnsupportedFeature {
            input,
            code,
            axis,
            evidence,
            ..
        } = error.reason()
        else {
            panic!("unexpected JEOL declaration failure: {error}");
        };
        assert_eq!(input, &nmr::InputSource::memory("sampling_declaration"));
        assert_eq!(*code, nmr::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT);
        assert_eq!(*axis, None);
        assert_eq!(evidence, &[detail]);
    }
    let repeated = options
        .sampling_declaration(declaration(vec![vec![1], vec![2], vec![2], vec![8]], 2))
        .read(&missing_path)
        .unwrap();
    let raw = repeated.as_raw().unwrap();
    assert!(raw.sampling_schedule().unwrap().has_repeated_coordinates());
    let traces = raw.data().sparse_traces().unwrap();
    assert_ne!(traces[1].samples(), traces[2].samples());
    assert_eq!(traces[1].coordinate(), traces[2].coordinate());
    let mut bytes = Vec::new();
    nmr::snapshot::write_snapshot(&repeated, &mut bytes, Default::default()).unwrap();
    drop(directory);
    let restored = nmr::snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(nmr::snapshot::AcceptRecordedHistory);
    assert_eq!(restored.as_raw().unwrap().data(), raw.data());
    assert_eq!(
        restored.as_raw().unwrap().sampling_schedule(),
        raw.sampling_schedule()
    );
}
