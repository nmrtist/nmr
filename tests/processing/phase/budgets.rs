//! Downstream task acceptance for the recommended prepare / inspect / execute API.

use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::{
    NormalizedAcmeV1, PhaseFailurePolicy, PhaseOptimizationError, PolarityState, ProcessingError,
    ProcessingOptions, WorkLedger,
};
use nmr::resource::{MemoryLimits, ResourceKind};
use nmr::{Complex64, Dataset};

fn spectrum(values: Vec<Complex64>) -> Dataset {
    let count = values.len();
    Dataset::from_processed(
        ProcessedDataset::new(
            ProcessedDescriptor::new(vec![
                ProcessedAxis::new(
                    AxisRole::Signal,
                    AxisDomain::Frequency,
                    Some(AxisUnit::Hertz),
                    count,
                    AxisCoordinates::Uniform {
                        start: 0.0,
                        step: 1.0,
                    },
                    ComponentBasis::Cartesian,
                )
                .unwrap(),
            ])
            .unwrap(),
            ProcessedData::new(
                vec![count],
                vec![2],
                values.into_iter().flat_map(|z| [z.re, z.im]).collect(),
            )
            .unwrap(),
            ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn automatic_analysis_has_exact_budget_boundaries_and_unverified_quality_is_fatal() {
    let input = spectrum(vec![Complex64::default(); 16]);
    let profile = NormalizedAcmeV1::new();
    let prepare = |options| {
        profile.preflight(
            &input,
            nmr::processing::AutoPhaseRequest {
                axis: nmr::AxisIndex::new(0),
                polarity: PolarityState::Ambiguous180,
                failure_policy: PhaseFailurePolicy::ContinueUnphasedReal,
            },
            options,
        )
    };
    let prepared = prepare(ProcessingOptions::new()).unwrap();
    let estimate = prepared.resources();
    let work = prepared.estimated_work();
    assert!(estimate.metadata_bytes() > 0);
    let mut low = WorkLedger::new(work - 1);
    assert_eq!(
        prepared
            .execute_with_context(&mut nmr::ExecutionContext::new(&mut low))
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::WorkLimit
    );
    assert_eq!(low.used(), 0);
    assert!(
        matches!(prepare(ProcessingOptions::new().max_metadata_bytes(0)).map_err(ProcessingError::into_root_cause), Err(ProcessingError::LimitExceeded(value)) if value.resource == ResourceKind::MetadataBytes)
    );
    let output = prepare(
        ProcessingOptions::new().memory(
            MemoryLimits::new()
                .max_metadata_bytes(estimate.metadata_bytes())
                .max_output_bytes(estimate.output_bytes())
                .max_working_bytes(estimate.working_bytes()),
        ),
    )
    .unwrap()
    .execute_with_context(&mut nmr::ExecutionContext::new(&mut WorkLedger::new(work)))
    .unwrap();
    assert_eq!(output.as_dense_processed().unwrap().samples(), &[0.0; 16]);
    assert_eq!(
        output
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap()
            .records()
            .len(),
        2
    );

    let unverified = spectrum(
        (0..64)
            .map(|k| Complex64::new((-((k as f64 - 25.0) / 4.0).powi(2)).exp(), 0.0))
            .collect(),
    );
    assert_eq!(
        profile
            .apply_with_context(
                &unverified,
                nmr::processing::AutoPhaseRequest {
                    axis: nmr::AxisIndex::new(0),
                    polarity: PolarityState::UserAssertedPositive,
                    failure_policy: PhaseFailurePolicy::ContinueUnphasedReal
                },
                ProcessingOptions::new(),
                &mut nmr::ExecutionContext::new(&mut WorkLedger::processing_default())
            )
            .unwrap_err()
            .into_root_cause(),
        ProcessingError::PhaseOptimization(PhaseOptimizationError::QualityUnverified)
    );
}
