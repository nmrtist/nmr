use crate::datasets::*;

#[test]
fn owned_narrowing_retains_addresses_context_and_cached_identity() {
    for input in [
        raw(),
        nmr::read(crate::fixture_paths::fixture("jcamp_dx/scaled-dif.dx")).unwrap(),
        roundtrip(&raw()),
        roundtrip(&nmr::read(crate::fixture_paths::fixture("jcamp_dx/scaled-dif.dx")).unwrap()),
    ] {
        let metadata = input.metadata().clone();
        let digest = input.canonical_digests();
        let pointer = if let Some(raw) = input.as_raw() {
            raw.data().dense_samples().unwrap().as_ptr() as usize
        } else {
            input.as_dense_processed().unwrap().samples().as_ptr() as usize
        };
        let input = if input.as_raw().is_some() {
            let input = input.into_processed().unwrap_err();
            let narrowed = input.into_raw().unwrap();
            assert_eq!(narrowed.dataset().metadata(), &metadata);
            assert_eq!(narrowed.raw().descriptor().axes().len(), 1);
            narrowed.into_dataset()
        } else {
            let input = input.into_raw().unwrap_err();
            let narrowed = input.into_processed().unwrap();
            assert_eq!(narrowed.dataset().metadata(), &metadata);
            assert_eq!(narrowed.processed().descriptor().axes().len(), 1);
            narrowed.into_dataset()
        };
        let after = if let Some(raw) = input.as_raw() {
            raw.data().dense_samples().unwrap().as_ptr() as usize
        } else {
            input.as_dense_processed().unwrap().samples().as_ptr() as usize
        };
        assert_eq!(pointer, after);
        assert_eq!(input.metadata(), &metadata);
        assert_eq!(input.canonical_digests(), digest);
    }
}
