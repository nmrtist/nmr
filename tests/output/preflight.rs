//! Downstream task acceptance for the recommended prepare / inspect / execute API.

use nmr::plot::{PlotData, PlotError};
use nmr::resource::{MemoryLimits, ResourceKind};

#[test]
fn plot_preflight_limits_all_owned_payload_before_materialization() {
    let input = nmr::read(crate::fixture_paths::fixture("jcamp_dx/scaled-dif.dx")).unwrap();
    let processed = input.as_processed().unwrap();
    let estimate = PlotData::preflight(processed, MemoryLimits::new())
        .unwrap()
        .resources();
    assert_eq!(estimate.output_bytes(), 32);
    assert_eq!(
        estimate.working_bytes(),
        estimate.output_bytes() + estimate.metadata_bytes()
    );
    let exact = MemoryLimits::new()
        .max_output_bytes(estimate.output_bytes())
        .max_metadata_bytes(estimate.metadata_bytes())
        .max_working_bytes(estimate.working_bytes());
    for (limits, resource) in [
        (exact.max_output_bytes(31), ResourceKind::OutputBytes),
        (
            exact.max_metadata_bytes(estimate.metadata_bytes() - 1),
            ResourceKind::MetadataBytes,
        ),
        (
            exact.max_working_bytes(estimate.working_bytes() - 1),
            ResourceKind::WorkingBytes,
        ),
    ] {
        assert!(
            matches!(PlotData::preflight(processed, limits), Err(PlotError::LimitExceeded(value)) if value.resource == resource)
        );
    }
    let plot = PlotData::preflight(processed, exact)
        .unwrap()
        .execute()
        .unwrap();
    assert_eq!(plot.data(), processed.data().samples());
    assert_eq!(plot.provenance(), processed.provenance());
}
