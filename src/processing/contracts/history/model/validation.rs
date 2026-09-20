use super::super::ComponentAccumulationOrder;
use crate::processed::model::ProcessedDescriptor;
use crate::processed::model::ProcessedOrigin;
use crate::processed::model::ProcessedValidationError;
use crate::processing::contracts::error::ProcessingError;
use crate::processing::contracts::operation::AttemptFailure;
use crate::processing::contracts::operation::ProcessingOperation;
use crate::processing::contracts::operation::ProcessingRequest;
use crate::processing::contracts::operation::ResolvedOperation;
use crate::processing::contracts::state::PlanState;
use crate::processing::contracts::state::StateError;
use crate::processing::contracts::state::expand_raw_descriptor;
use crate::processing::contracts::state::transition;
use crate::processing::contracts::state::transition_auto_phase;
use crate::provenance::SourceFile;

use super::super::{ProcessingRecord, algorithm_version};
use super::{HistoryInput, ProcessingHistory};
impl ProcessingHistory {
    pub(crate) fn validate_recorded(&self) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        self.validate_input_structure()
            .map_err(|e| ModelError::Validation(e.to_string()))?;
        self.initial_descriptor
            .validate()
            .map_err(|e| ModelError::Validation(e.to_string()))?;
        for record in &self.records {
            if let Some(version) = record.algorithm_version() {
                if version != algorithm_version(record.requested()) {
                    return Err(ModelError::UnsupportedHistoryVersion(version.into()));
                }
            }
        }
        let mut previous = 0;
        for segment in &self.segments {
            if segment.end_record() <= previous || segment.end_record() > self.records.len() {
                return Err(ModelError::Structure);
            }
            previous = segment.end_record();
        }
        if previous != self.records.len() {
            return Err(ModelError::Structure);
        }
        Ok(())
    }
    pub(crate) fn validate_recorded_evidence(
        &self,
        origin: &ProcessedOrigin,
        sources: &[SourceFile],
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        if self.input().sources() != sources {
            return Err(ModelError::Structure);
        }
        let mut state = match origin {
            ProcessedOrigin::Library(boundary) => boundary.state.clone(),
            ProcessedOrigin::DerivedRaw(raw) => PlanState::from_raw(
                &self.initial_descriptor,
                raw.snapshot().descriptor().axes(),
                raw.snapshot().sampling_schedule(),
                raw.snapshot().absolute_origin(),
            )
            .map_err(|e| ModelError::Validation(ProcessingError::from(e).to_string()))?,
            _ => PlanState::from_descriptor(&self.initial_descriptor),
        };
        if let HistoryInput::Processed {
            digests,
            read_record,
            ..
        } = self.input()
        {
            crate::canonical_digest::check_processed_evidence(
                &self.initial_descriptor,
                &state,
                *digests,
                Some(control.cancellation()),
            )?;
            if let Some(record) = read_record {
                record.validate_recorded(&self.initial_descriptor, *digests, sources)?;
            }
        }
        let mut segments = self.segments.iter().peekable();
        for (index, record) in self.records.iter().enumerate() {
            control.check_cancelled()?;
            replay_record(&mut state, record)
                .map_err(|e| ModelError::Validation(ProcessingError::from(e).to_string()))?;
            if segments
                .peek()
                .is_some_and(|segment| segment.end_record() == index + 1)
            {
                let segment = segments.next().expect("peeked segment");
                crate::canonical_digest::check_processed_evidence(
                    &state.descriptor().map_err(|e| {
                        ModelError::Validation(ProcessingError::from(e).to_string())
                    })?,
                    &state,
                    segment.output_digests(),
                    Some(control.cancellation()),
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn validate_input_structure(&self) -> Result<(), ProcessingError> {
        if self.inputs.len() != 1 {
            return Err(ProcessingError::MissingCapability {
                capability: "single external replay input",
                axis: None,
            });
        }
        if self.axis_lineage.iter().copied().ne(output_lineage(
            self.initial_descriptor.axes().len(),
            &self.records,
        )) {
            return Err(ProcessingError::Mapping(
                "history input-axis references are invalid",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static HISTORY_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn output_lineage(
    rank: usize,
    records: &[ProcessingRecord],
) -> Vec<crate::provenance::InputAxisRef> {
    let mut lineage: Vec<_> = (0..rank)
        .map(crate::provenance::InputAxisRef::single)
        .collect();
    for record in records {
        if let ProcessingRequest::Explicit(ProcessingOperation::Spectrum { axis, operation }) =
            record.requested()
        {
            if operation.removes_axis() && *axis < lineage.len() {
                lineage.remove(*axis);
            }
        }
    }
    lineage
}

pub(crate) fn validate_derived_history(
    history: &ProcessingHistory,
    descriptor: &ProcessedDescriptor,
    origin: &ProcessedOrigin,
    sources: &[SourceFile],
) -> Result<PlanState, ProcessedValidationError> {
    #[cfg(test)]
    HISTORY_VALIDATIONS.with(|count| count.set(count.get() + 1));
    if history.inputs.len() != 1
        || history.axis_lineage.iter().copied().ne(output_lineage(
            history.initial_descriptor.axes().len(),
            &history.records,
        ))
    {
        return Err(ProcessedValidationError::InvalidProcessingHistory);
    }
    let mut state = match origin {
        ProcessedOrigin::DerivedRaw(origin) => {
            let expected = expand_raw_descriptor(origin.snapshot().descriptor().axes())
                .map_err(|_| ProcessedValidationError::InvalidProcessingHistory)?;
            if history.raw_input_digests() != Some(origin.snapshot().canonical_digests())
                || history.raw_input_normalization()
                    != Some(origin.snapshot().sample_normalization())
                || !expected.same_axes_except_labels(history.initial_descriptor())
                || origin.axis_lineage()
                    != (0..expected.axes().len())
                        .map(crate::provenance::InputAxisRef::single)
                        .collect::<Vec<_>>()
                        .as_slice()
                || sources != origin.snapshot().sources()
            {
                return Err(ProcessedValidationError::InvalidProcessingHistory);
            }
            PlanState::from_raw(
                history.initial_descriptor(),
                origin.snapshot().descriptor().axes(),
                origin.snapshot().sampling_schedule(),
                origin.snapshot().absolute_origin(),
            )
            .map_err(|_| ProcessedValidationError::InvalidProcessingHistory)?
        }
        ProcessedOrigin::Library(boundary) => {
            let HistoryInput::Processed {
                digests,
                sources: recorded,
                read_record,
            } = history.input()
            else {
                return Err(ProcessedValidationError::InvalidProcessingHistory);
            };
            if *digests != boundary.canonical_digests()
                || history.initial_descriptor() != boundary.descriptor()
                || recorded != sources
                || read_record.is_some()
            {
                return Err(ProcessedValidationError::InvalidProcessingHistory);
            }
            boundary.state.clone()
        }
        ProcessedOrigin::External(boundary) => {
            let HistoryInput::Processed {
                digests,
                sources: recorded,
                read_record,
            } = history.input()
            else {
                return Err(ProcessedValidationError::InvalidProcessingHistory);
            };
            if *digests != boundary.canonical_digests()
                || history.initial_descriptor() != boundary.descriptor()
                || recorded != sources
                || read_record.is_some()
                || sources != boundary.parent().provenance().sources()
            {
                return Err(ProcessedValidationError::InvalidProcessingHistory);
            }
            PlanState::from_descriptor(history.initial_descriptor())
        }
        ProcessedOrigin::Imported
        | ProcessedOrigin::DeclaredRaw { .. }
        | ProcessedOrigin::Unknown => {
            if matches!(history.input(), HistoryInput::Raw { .. }) {
                return Err(ProcessedValidationError::InvalidProcessingHistory);
            }
            PlanState::from_descriptor(history.initial_descriptor())
        }
    };
    for record in history.records() {
        replay_record(&mut state, record)
            .map_err(|_| ProcessedValidationError::InvalidProcessingHistory)?;
    }
    if state
        .descriptor()
        .map_err(|_| ProcessedValidationError::InvalidProcessingHistory)?
        != *descriptor
    {
        return Err(ProcessedValidationError::InvalidProcessingHistory);
    }
    Ok(state)
}

pub(crate) fn replay_record(
    state: &mut PlanState,
    record: &ProcessingRecord,
) -> Result<(), StateError> {
    match record {
        ProcessingRecord::Applied {
            requested,
            resolved,
            input_descriptor,
            output_descriptor,
            algorithm_version,
            accumulation_order,
            ..
        } => {
            if state.descriptor()? != *input_descriptor
                || *algorithm_version != self::algorithm_version(requested)
                || *accumulation_order
                    != matches!(
                        requested,
                        ProcessingRequest::Explicit(ProcessingOperation::ComponentTransform { .. })
                    )
                    .then_some(ComponentAccumulationOrder::LaneAscending)
            {
                return Err(StateError::Mapping(
                    "processing history input contract disagrees",
                ));
            }
            if let ProcessingRequest::BaselineEstimate { axis, method } = requested {
                crate::processing::contracts::baseline::validate_resolution(
                    state, *axis, *method, resolved,
                )?;
                transition(
                    state,
                    &ProcessingOperation::Spectrum {
                        axis: *axis,
                        operation: crate::processing::SpectrumOperation::Baseline(*method),
                    },
                )?;
            } else if let ProcessingRequest::AutoPhase { axis, .. }
            | ProcessingRequest::PhaseMethod { axis, .. } = requested
            {
                let correction = match (requested, resolved.as_ref()) {
                    (
                        ProcessingRequest::AutoPhase { .. },
                        ResolvedOperation::PhaseCorrection(c),
                    ) => Some(c),
                    (
                        ProcessingRequest::PhaseMethod { .. },
                        ResolvedOperation::AutomaticPhase {
                            correction,
                            objective,
                            evaluations,
                        },
                    ) if objective.is_finite() && *evaluations > 0 => Some(correction),
                    _ => None,
                };
                let Some(correction) = correction else {
                    return Err(StateError::Mapping(
                        "automatic phase history has an invalid resolution",
                    ));
                };
                if !correction.p0_degrees().is_finite()
                    || !correction.p1_degrees().is_finite()
                    || !correction.pivot_fraction().is_finite()
                    || !(0.0..=1.0).contains(&correction.pivot_fraction())
                {
                    return Err(StateError::InvalidParameter("phase correction"));
                }
                transition_auto_phase(state, *axis)?;
            } else {
                let ProcessingRequest::Explicit(operation) = requested else {
                    return Err(StateError::Mapping("unsupported processing request"));
                };
                let replayed = transition(state, operation)?;
                if replayed != **resolved {
                    return Err(StateError::Mapping(
                        "processing history resolved parameters disagree",
                    ));
                }
            }
            if state.descriptor()? != *output_descriptor {
                return Err(StateError::Mapping(
                    "processing history output contract disagrees",
                ));
            }
        }
        ProcessingRecord::Attempted {
            requested, failure, ..
        } => {
            if !attempt_failure_is_valid(requested, *failure) {
                return Err(StateError::Mapping("processing attempt record is invalid"));
            }
            let ProcessingRequest::AutoPhase { axis, .. } = requested else {
                return Err(StateError::Mapping("unsupported processing attempt"));
            };
            state.record_phase_failure(*axis)?;
        }
    }
    Ok(())
}

fn attempt_failure_is_valid(requested: &ProcessingRequest, failure: AttemptFailure) -> bool {
    matches!(
        (requested, failure),
        (
            ProcessingRequest::AutoPhase {
                failure_policy:
                    crate::processing::contracts::operation::PhaseFailurePolicy::ContinueUnphasedReal,
                ..
            },
            AttemptFailure::NoUsableSignal
                | AttemptFailure::ObjectiveUndefined
                | AttemptFailure::OptimizationDidNotConverge
        )
    )
}
