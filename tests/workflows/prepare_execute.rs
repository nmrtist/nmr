//! Downstream task acceptance for the recommended prepare / inspect / execute API.

use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::plot::PlotData;
use nmr::processing::{
    DensePipeline, DirectDelayMode, ExpectedPolarity, FrequencyFrame, ProcessingError,
    ProcessingOperation as Op, ProcessingOptions, ProcessingPlan, Projection, WorkLedger,
};
use nmr::raw::{
    DirectSamples, GroupDelayState, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata,
};
use nmr::{Complex64, Dataset};

fn raw(delay: GroupDelayState) -> Dataset {
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        4,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        },
    )
    .unwrap()
    .with_group_delay(delay)
    .unwrap();
    Dataset::from_raw(
        RawDatasetBuilder::new(vec![axis], RawMetadata::default())
            .unwrap()
            .dense(vec![
                Complex64::new(1.0, 0.0),
                Complex64::default(),
                Complex64::default(),
                Complex64::default(),
            ])
            .unwrap(),
    )
}

#[test]
fn failed_consuming_conversion_returns_the_original_input_and_read_context() {
    let path = crate::fixture_paths::fixture("jcamp_dx/scaled-dif.dx");
    let input = nmr::read(&path).unwrap();
    let digest = input.canonical_digests();
    let metadata = input.metadata().clone();
    let input = input.into_raw().unwrap_err();
    assert_eq!(input.canonical_digests(), digest);
    assert_eq!(input.metadata(), &metadata);
    let plan = ProcessingPlan::new(vec![Op::ReverseAxis { axis: 0 }]).unwrap();
    let result = plan.apply(&input).unwrap();
    assert_eq!(result.metadata(), input.metadata());
    assert_eq!(
        result.as_dense_processed().unwrap().samples(),
        &[13.0, 12.0, 11.0, 10.0]
    );
    let low = plan.apply_processed(input.as_processed().unwrap()).unwrap();
    assert_eq!(low.canonical_digests(), result.canonical_digests());
    assert_eq!(
        low.provenance().history(),
        result.as_processed().unwrap().provenance().history()
    );
    let raw = raw(GroupDelayState::NotApplicable);
    let original = raw.canonical_digests();
    assert_eq!(
        raw.into_processed().unwrap_err().canonical_digests(),
        original
    );
}

#[test]
fn fixed_recipe_and_explicit_processing_share_results_and_limits() {
    let input = raw(GroupDelayState::NotApplicable);
    let policy = DensePipeline::new(
        vec![None]
            .into_iter()
            .zip(vec![Some(FrequencyFrame::Hertz)])
            .enumerate()
            .map(
                |(index, (phase, frequency_frame))| nmr::processing::DenseAxisConfig {
                    axis: nmr::AxisIndex::new(index),
                    phase,
                    frequency_frame,
                },
            )
            .collect(),
        nmr::processing::DensePipelineOptions {
            direct_delay: DirectDelayMode::NoDelay,
            projection: Projection::Real,
            expected_polarity: ExpectedPolarity::Signed,
            descending_ppm: false,
        },
    )
    .unwrap();
    let recipe = policy.plan(input.as_raw().unwrap()).unwrap();
    let explicit = recipe.apply(&input).unwrap();
    let automatic = policy.apply(&input, ProcessingOptions::new()).unwrap();
    assert_eq!(explicit.canonical_digests(), automatic.canonical_digests());
    assert_eq!(
        explicit.as_processed().unwrap().provenance().history(),
        automatic.as_processed().unwrap().provenance().history()
    );
    assert!(matches!(
        policy
            .apply(&input, ProcessingOptions::new().max_metadata_bytes(0))
            .map_err(ProcessingError::into_root_cause),
        Err(ProcessingError::LimitExceeded(_))
    ));
    let plot = PlotData::from_processed(explicit.as_processed().unwrap()).unwrap();
    assert_eq!(plot.data(), &[1.0; 8]);
    let folder = tempfile::tempdir().unwrap();
    let target = folder.path().join("spectrum.npz");
    nmr::export::export_npz(&plot, &target, &mut WorkLedger::processing_default()).unwrap();
    assert!(std::fs::metadata(target).unwrap().len() > 0);
}
