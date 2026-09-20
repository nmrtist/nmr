use crate::inputs;
use nmr::{ReadOptions, SamplingDeclaration, SamplingIndexBase, snapshot};

fn restored(input: &nmr::Dataset) -> nmr::Dataset {
    let mut bytes = Vec::new();
    snapshot::write_snapshot(input, &mut bytes, Default::default()).unwrap();
    snapshot::decode_snapshot(&bytes, Default::default())
        .unwrap()
        .restore(snapshot::AcceptRecordedHistory)
}

fn request(
    grid: Vec<usize>,
    rows: Vec<Vec<usize>>,
    base: SamplingIndexBase,
    lanes: Vec<usize>,
) -> SamplingDeclaration {
    SamplingDeclaration::new(
        nmr::raw::AssertionId::try_new("consumer-regression").unwrap(),
        "explicit synthetic table",
        grid,
        rows,
        base,
        lanes,
    )
}

fn sampling_failure(error: &nmr::ReadError, detail: &str) {
    assert_eq!(error.kind(), nmr::ReadErrorKind::UnsupportedFeature);
    let nmr::ReadErrorReason::UnsupportedFeature {
        input,
        code,
        axis,
        evidence,
        ..
    } = error.reason()
    else {
        panic!("unexpected sampling failure: {error}");
    };
    assert_eq!(input, &nmr::InputSource::memory("sampling_declaration"));
    assert_eq!(*code, nmr::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT);
    assert_eq!(*axis, None);
    assert_eq!(evidence, &[detail]);
}

#[test]
fn missing_bruker_schedule_accepts_only_checked_declarations_and_keeps_sources() {
    let dir = tempfile::tempdir().unwrap();
    inputs::bruker_nus(dir.path()).unwrap();
    let options = ReadOptions::new().allow_experimental_vendor_semantics(true);
    assert!(options.read(dir.path()).is_err());
    assert!(
        ReadOptions::new()
            .sampling_declaration(inputs::declaration())
            .read(dir.path())
            .is_err()
    );
    let input = options
        .clone()
        .sampling_declaration(inputs::declaration())
        .read(dir.path())
        .unwrap();
    assert!(!dir.path().join("nuslist").exists());
    let raw = input.as_raw().unwrap();
    let schedule = raw.sampling_schedule().unwrap();
    assert_eq!(schedule.grid(), [4]);
    assert_eq!(
        schedule
            .coordinates()
            .iter()
            .map(|c| c.as_slice()[0])
            .collect::<Vec<_>>(),
        [3, 1]
    );
    assert_eq!(schedule.declaration(), Some(&inputs::declaration()));
    assert_eq!(raw.provenance().sources().len(), 3);
    let offline = restored(&input);
    assert_eq!(
        offline.as_raw().unwrap().sampling_schedule(),
        Some(schedule)
    );
    assert_eq!(offline.as_raw().unwrap().data(), raw.data());
    for (case, declaration) in [
        request(
            vec![5],
            vec![vec![3], vec![1]],
            SamplingIndexBase::Zero,
            vec![2],
        ),
        request(
            vec![4],
            vec![vec![4], vec![1]],
            SamplingIndexBase::Zero,
            vec![2],
        ),
        request(
            vec![4],
            vec![vec![0], vec![1]],
            SamplingIndexBase::One,
            vec![2],
        ),
        request(vec![4], vec![vec![3]], SamplingIndexBase::Zero, vec![2]),
        request(
            vec![4],
            vec![vec![3], vec![1]],
            SamplingIndexBase::Zero,
            vec![1],
        ),
        request(
            vec![4],
            vec![vec![3, 0], vec![1]],
            SamplingIndexBase::Zero,
            vec![2],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let error = options
            .clone()
            .sampling_declaration(declaration)
            .read(dir.path())
            .unwrap_err();
        sampling_failure(
            &error,
            if matches!(case, 1 | 2) {
                "sampling declaration index is outside the declared grid"
            } else {
                "sampling declaration conflicts with vendor grid, lane count or observation count"
            },
        );
    }
    std::fs::write(dir.path().join("nuslist"), inputs::NUSLIST).unwrap();
    let vendor = options.read(dir.path()).unwrap();
    assert_eq!(vendor.as_raw().unwrap().data(), raw.data());
    let agreed = options
        .clone()
        .sampling_declaration(inputs::declaration())
        .read(dir.path())
        .unwrap();
    assert_eq!(agreed.sources().len(), 4);
    let conflict = request(
        vec![4],
        vec![vec![1], vec![3]],
        SamplingIndexBase::Zero,
        vec![2],
    );
    sampling_failure(
        &options
            .sampling_declaration(conflict)
            .read(dir.path())
            .unwrap_err(),
        "sampling declaration disagrees with vendor observation order or coordinates",
    );
}

#[test]
fn repeated_declarations_preserve_observation_identity_and_resource_limits() {
    let dir = tempfile::tempdir().unwrap();
    inputs::bruker_nus(dir.path()).unwrap();
    let options = ReadOptions::new().allow_experimental_vendor_semantics(true);
    let duplicate = request(
        vec![4],
        vec![vec![3], vec![3]],
        SamplingIndexBase::Zero,
        vec![2],
    );
    let input = options
        .clone()
        .sampling_declaration(duplicate.clone())
        .read(dir.path())
        .unwrap();
    let raw = input.as_raw().unwrap();
    assert!(raw.sampling_schedule().unwrap().has_repeated_coordinates());
    let traces = raw.data().sparse_traces().unwrap();
    assert_eq!(traces[0].coordinate(), traces[1].coordinate());
    assert_eq!(traces[0].ordinal().get(), 0);
    assert_eq!(traces[1].ordinal().get(), 1);
    assert_ne!(traces[0].samples(), traces[1].samples());
    assert_eq!(restored(&input).as_raw().unwrap().data(), raw.data());
    let mut charges = Vec::new();
    for (limits, expected_resource) in [
        (
            nmr::ReadLimits::new().max_metadata_bytes(1),
            nmr::ReadResource::MetadataBytes,
        ),
        (
            nmr::ReadLimits::new().max_working_bytes(1),
            nmr::ReadResource::WorkingBytes,
        ),
    ] {
        let error = options
            .clone()
            .sampling_declaration(duplicate.clone())
            .limits(limits)
            .read(dir.path())
            .unwrap_err();
        assert_eq!(error.kind(), nmr::ReadErrorKind::LimitExceeded);
        let nmr::ReadErrorReason::LimitExceeded {
            resource,
            limit,
            required,
            ..
        } = error.reason()
        else {
            panic!("unexpected sampling budget failure: {error}");
        };
        assert_eq!(*resource, expected_resource);
        assert_eq!(*limit, 1);
        assert!(*required > 1);
        charges.push(*required);
    }
    assert_eq!(charges[1], 3 * charges[0]);
    // The adapter still needs its own metadata/workspace after the declaration
    // charge; granting exactly that charge must not release it prematurely.
    for limits in [
        nmr::ReadLimits::new().max_metadata_bytes(charges[0]),
        nmr::ReadLimits::new().max_working_bytes(charges[1]),
    ] {
        assert_eq!(
            options
                .clone()
                .sampling_declaration(duplicate.clone())
                .limits(limits)
                .read(dir.path())
                .unwrap_err()
                .kind(),
            nmr::ReadErrorKind::LimitExceeded
        );
    }
    let token = nmr::CancellationToken::new();
    token.cancel();
    let mut control = nmr::ExecutionContext::default().with_cancellation(token);
    assert!(matches!(
        options
            .sampling_declaration(duplicate)
            .read_with_context(dir.path(), &mut control)
            .unwrap_err()
            .reason(),
        nmr::ReadErrorReason::Execution(nmr::execution::ExecutionError::Cancelled)
    ));
}
