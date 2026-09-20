use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::PolarityState;
use nmr::processing::ProcessingRequest;
use nmr::processing::{
    AttemptFailure, NormalizedAcmeV1, PhaseFailurePolicy, PhaseOptimizationError,
    ProcessingOperation, ProcessingOptions, ProcessingRecord, Projection, WorkLedger,
};

fn frequency_dataset(samples: Vec<f64>) -> ProcessedDataset {
    let points = samples.len() / 2;
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        points,
        AxisCoordinates::Uniform {
            start: -(points as f64) / 2.0,
            step: 1.0,
        },
        ComponentBasis::Cartesian,
    )
    .unwrap();
    ProcessedDataset::new(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        ProcessedData::new(vec![points], vec![2], samples).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap()
}

#[test]
fn normalized_acme_quality_boundaries_are_typed_and_do_not_charge_candidates() {
    let profile = NormalizedAcmeV1::new();
    let mut work = WorkLedger::new(u128::MAX);
    assert_eq!(
        profile
            .optimize(
                &[Complex64::new(0.0, 0.0); 8],
                PolarityState::Ambiguous180,
                &mut work,
            )
            .unwrap_err(),
        PhaseOptimizationError::NoUsableSignal
    );
    assert_eq!(work.used(), 0);

    assert_eq!(
        profile
            .optimize(
                &[Complex64::new(1.0, 1.0); 8],
                PolarityState::Ambiguous180,
                &mut work,
            )
            .unwrap_err(),
        PhaseOptimizationError::ObjectiveUndefined
    );
    assert_eq!(work.used(), 0);

    assert_eq!(
        profile
            .optimize(
                &[Complex64::new(f64::MAX, f64::MAX), Complex64::new(1.0, 0.0)],
                PolarityState::Ambiguous180,
                &mut work,
            )
            .unwrap_err(),
        PhaseOptimizationError::NumericalInvariantViolation
    );
}

#[test]
fn entropy_search_is_bounded_deterministic_and_rejects_unverified_quality() {
    let trace = (0..256)
        .map(|index| {
            let u = (index as f64 - 103.0) / 4.0;
            let canonical = Complex64::new(1.0, -u) / (1.0 + u * u);
            let phase = (-37.0 + 60.0 * (index as f64 / 256.0 - 1.0)).to_radians();
            canonical * Complex64::from_polar(1.0, phase)
        })
        .collect::<Vec<_>>();
    let polarity = PolarityState::UserAssertedPositive;
    let bound = NormalizedAcmeV1::MAX_WORK_PASSES as u128 * trace.len() as u128;
    let mut first_work = WorkLedger::new(bound);
    let first = NormalizedAcmeV1.optimize(&trace, polarity, &mut first_work);
    let mut second_work = WorkLedger::new(bound);
    let second = NormalizedAcmeV1.optimize(&trace, polarity, &mut second_work);
    assert_eq!(first, Err(PhaseOptimizationError::QualityUnverified));
    assert_eq!(first, second);
    assert!(first_work.used() <= bound);
    assert_eq!(first_work.used(), second_work.used());
}

#[test]
fn insufficient_work_is_fatal_not_a_quality_fallback() {
    let trace = (0..16)
        .map(|index| Complex64::new(index as f64, (index * index) as f64))
        .collect::<Vec<_>>();
    let mut work = WorkLedger::new(599 * trace.len() as u128);
    assert_eq!(
        NormalizedAcmeV1::new()
            .optimize(&trace, PolarityState::Ambiguous180, &mut work)
            .unwrap_err(),
        PhaseOptimizationError::WorkLimit
    );
    assert_eq!(work.used(), 0); // The complete bound is checked before allocating or entering numerical work.
}

#[test]
fn continue_policy_records_attempt_then_unphased_projection_in_order() {
    let dataset = frequency_dataset(vec![0.0; 16]);
    let mut work = WorkLedger::new(u128::MAX);
    let output = NormalizedAcmeV1::new()
        .apply_processed_with_context(
            &dataset,
            nmr::processing::AutoPhaseRequest {
                axis: nmr::AxisIndex::new(0),
                polarity: PolarityState::Ambiguous180,
                failure_policy: PhaseFailurePolicy::ContinueUnphasedReal,
            },
            ProcessingOptions::default(),
            &mut nmr::ExecutionContext::new(&mut work),
        )
        .unwrap();
    assert_eq!(output.descriptor().component_counts(), [1]);
    let records = output.provenance().history().unwrap().records();
    assert_eq!(records.len(), 2);
    assert!(matches!(
        &records[0],
        ProcessingRecord::Attempted {
            requested: ProcessingRequest::AutoPhase { .. },
            failure: AttemptFailure::NoUsableSignal,
            ..
        }
    ));
    assert!(matches!(
        &records[1],
        ProcessingRecord::Applied {
            requested: ProcessingRequest::Explicit(ProcessingOperation::Projection {
                projection: Projection::UnphasedReal,
                ..
            }),
            ..
        }
    ));
    output.validate().unwrap();
}

#[test]
fn unverified_quality_is_never_hidden_by_unphased_fallback() {
    let mut samples = Vec::new();
    for index in 0..256 {
        let u = (index as f64 - 103.0) / 4.0;
        let phase = (-37.0 + 60.0 * (index as f64 / 256.0 - 1.0)).to_radians();
        let value = Complex64::new(1.0, -u) / (1.0 + u * u) * Complex64::from_polar(1.0, phase);
        samples.extend([value.re, value.im]);
    }
    let dataset = frequency_dataset(samples);
    for policy in [
        PhaseFailurePolicy::Fail,
        PhaseFailurePolicy::ContinueUnphasedReal,
    ] {
        assert!(matches!(
            NormalizedAcmeV1.apply_processed_with_context(
                &dataset,
                nmr::processing::AutoPhaseRequest {
                    axis: nmr::AxisIndex::new(0),
                    polarity: PolarityState::UserAssertedPositive,
                    failure_policy: policy
                },
                ProcessingOptions::new(),
                &mut nmr::ExecutionContext::new(&mut WorkLedger::new(u128::MAX))
            ),
            Err(nmr::processing::ProcessingError::PhaseOptimization(
                PhaseOptimizationError::QualityUnverified
            ))
        ));
    }
}

#[test]
fn successful_auto_phase_records_the_request_and_resolved_parameters() {
    let mut samples = Vec::new();
    for index in 0..64 {
        let u = (index as f64 - 25.0) / 3.0;
        samples.push(1.0 / (1.0 + u * u));
        samples.push(-u / (1.0 + u * u));
    }
    let dataset = frequency_dataset(samples);
    let mut work = WorkLedger::new(u128::MAX);
    let output = NormalizedAcmeV1::new()
        .apply_processed_with_context(
            &dataset,
            nmr::processing::AutoPhaseRequest {
                axis: nmr::AxisIndex::new(0),
                polarity: PolarityState::UserAssertedPositive,
                failure_policy: PhaseFailurePolicy::Fail,
            },
            ProcessingOptions::default(),
            &mut nmr::ExecutionContext::new(&mut work),
        )
        .unwrap();
    let records = output.provenance().history().unwrap().records();
    assert_eq!(records.len(), 1);
    assert!(matches!(
        &records[0],
        ProcessingRecord::Applied {
            requested: ProcessingRequest::AutoPhase { .. },
            resolved,
            ..
        } if matches!(resolved.as_ref(), nmr::processing::ResolvedOperation::PhaseCorrection(_))
    ));
    output.validate().unwrap();
}

#[test]
fn cancellation_during_phase_search_keeps_entered_work() {
    let trace: Vec<_> = (0..129)
        .map(|i| nmr::Complex64::new(i as f64, (i * i) as f64))
        .collect();
    let token = nmr::CancellationToken::new();
    let signal = token.clone();
    let mut progress = move |event: nmr::execution::ProgressEvent| {
        if event.stage == nmr::execution::ExecutionStage::AutoPhase && event.completed > 129 * 4 {
            signal.cancel();
        }
    };
    let mut work = WorkLedger::new(1_000_000);
    let error = NormalizedAcmeV1
        .optimize_with_context(
            &trace,
            PolarityState::Ambiguous180,
            &mut nmr::ExecutionContext::new(&mut work)
                .with_cancellation(token)
                .with_progress(&mut progress),
        )
        .unwrap_err();
    assert_eq!(
        error,
        PhaseOptimizationError::Execution(nmr::execution::ExecutionError::Cancelled)
    );
    assert!(work.used() > 0 && work.used() < 1_000_000);
}
