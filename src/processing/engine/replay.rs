//! Numeric replay belongs to execution, separate from immutable history values.

use crate::execution::ExecutionContext;
use crate::processed::{DerivedRawOrigin, ProcessedDataset, ProcessedOrigin, ProcessedProvenance};
use crate::processing::contracts::{
    error::*, history::*, operation::*, options::*, prepared_step::*, state::*,
};
use crate::processing::engine::execution::*;
use crate::processing::kernels::buffer::*;
use crate::processing::prepare::plan::*;
use crate::processing::prepare::resources::*;
use crate::raw::{RawDataset, SourceDigest};

use crate::processing::contracts::history::replay_record;

impl ProcessingHistory {
    /// Strictly replays from ordered aggregate starting points and retains their
    /// reading context. Current replay rejects any input count other than one.
    pub fn replay(
        &self,
        inputs: &[&crate::Dataset],
        options: ProcessingOptions,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.replay_with_context(inputs, options, &mut ExecutionContext::default())
    }
    /// Replays using shared cancellation, progress and work accounting.
    pub fn replay_with_context(
        &self,
        inputs: &[&crate::Dataset],
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        control.check_cancelled()?;
        if self.inputs().len() != 1 {
            return Err(ProcessingError::MissingCapability {
                capability: "single external replay input",
                axis: None,
            });
        }
        if inputs.len() != 1 {
            return Err(ProcessingError::InputCountMismatch {
                expected: 1,
                actual: inputs.len(),
            });
        }
        let input = inputs[0];
        let options =
            crate::processing::prepare::resources::reserve_context(input.metadata(), options)?;
        let output = match self.input() {
            HistoryInput::Raw { .. } => self.replay_raw_with_context(
                input
                    .as_raw()
                    .ok_or(ProcessingError::InputIdentityMismatch)?,
                options,
                control,
            )?,
            HistoryInput::Processed { .. } => self.replay_processed_with_context(
                input
                    .as_processed()
                    .ok_or(ProcessingError::InputIdentityMismatch)?,
                options,
                control,
            )?,
        };
        Ok(input.derived_processed(output))
    }

    /// Replays all resolved operations against the exact bound raw input.
    ///
    /// Opaque source metadata is not consulted. Automatic operations use their
    /// recorded resolved parameters rather than running inference again.
    pub fn replay_raw(
        &self,
        raw: &RawDataset,
        options: ProcessingOptions,
    ) -> Result<crate::processed::ProcessedDataset, ProcessingError> {
        self.replay_raw_with_context(raw, options, &mut ExecutionContext::default())
    }
    /// Replays using shared cancellation, progress and work accounting.
    pub fn replay_raw_with_context(
        &self,
        raw: &RawDataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::processed::ProcessedDataset, ProcessingError> {
        control.check_cancelled()?;
        self.validate_input_structure()?;
        replay_raw_history(self, raw, options, control)
    }

    /// Strictly replays from the original processed starting point.
    /// Success confirms identity checks and execution, not numerical comparison.
    pub fn replay_processed(
        &self,
        input: &crate::processed::ProcessedDataset,
        options: ProcessingOptions,
    ) -> Result<crate::processed::ProcessedDataset, ProcessingError> {
        self.replay_processed_with_context(input, options, &mut ExecutionContext::default())
    }
    /// Replays using shared cancellation, progress and work accounting.
    pub fn replay_processed_with_context(
        &self,
        input: &crate::processed::ProcessedDataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::processed::ProcessedDataset, ProcessingError> {
        control.check_cancelled()?;
        self.validate_input_structure()?;
        replay_processed_history(self, input, options, control)
    }
}

pub(crate) fn replay_raw_history(
    history: &ProcessingHistory,
    raw: &RawDataset,
    options: ProcessingOptions,
    control: &mut ExecutionContext<'_>,
) -> Result<ProcessedDataset, ProcessingError> {
    let options = reserve_prepared(
        history.initial_descriptor().axes().len(),
        history.records().len(),
        false,
        options,
    )?;
    let options = reserve_axes(
        checked_times(
            axis_reservation(
                raw_axis_storage(raw.descriptor().axes())?,
                std::iter::empty(),
                true,
            )?,
            3,
        )?
        .checked_add(checked_times(history_axis_bytes(history)?, 2)?)
        .ok_or(ProcessingError::SizeOverflow)?,
        options,
    )?;
    let options = reserve_states(raw.descriptor().axes().len(), raw_map_points(raw), options)?;
    let options = reserve_containers(
        crate::processing::prepare::memory::raw_replay(raw, history)?,
        options,
    )?;
    let input = ProcessingInput::from_dataset_with_context(raw, control)?;
    if history.raw_input_digests() != Some(input.digests()) {
        return Err(ProcessingError::InputIdentityMismatch);
    }
    if history.raw_input_normalization() != Some(raw.provenance().sample_normalization()) {
        return Err(ProcessingError::InputIdentityMismatch);
    }
    let HistoryInput::Raw {
        format, sources, ..
    } = history.input()
    else {
        return Err(ProcessingError::InputIdentityMismatch);
    };
    if *format != raw.provenance().format()
        || sources.len() != raw.provenance().sources().len()
        || sources
            .iter()
            .zip(raw.provenance().sources())
            .any(|(a, b)| !a.same_content(b))
    {
        return Err(ProcessingError::InputIdentityMismatch);
    }
    if format.is_some() && sources.is_empty()
        || sources
            .iter()
            .any(|source| matches!(source.digest(), SourceDigest::NotComputed))
    {
        return Err(ProcessingError::MissingCapability {
            capability: "complete raw source identity",
            axis: None,
        });
    }
    if !(1..=2).contains(&raw.descriptor().axes().len()) || !input.dense {
        return Err(ProcessingError::Mapping(
            "history replay requires dense rank-1 or rank-2 raw input",
        ));
    }
    if !input.strong_source_identity {
        return Err(ProcessingError::MissingCapability {
            capability: "complete source-data digest",
            axis: None,
        });
    }

    // Resolve and check the entire history before allocating expanded samples.
    let mut baseline = expand_raw_descriptor(raw.descriptor().axes())?;
    check_sample_peak(descriptor_bytes(&baseline)?, 0, options)?;
    if !baseline.same_axes_except_labels(history.initial_descriptor()) {
        return Err(ProcessingError::InputIdentityMismatch);
    }
    baseline = history.initial_descriptor().clone();
    let mut state = PlanState::from_raw(
        &baseline,
        raw.descriptor().axes(),
        raw.sampling_schedule(),
        raw.data().layout().absolute_origin(),
    )?;
    let mut steps = Vec::new();
    steps
        .try_reserve_exact(history.records().len())
        .map_err(|_| ProcessingError::AllocationFailure)?;
    for (step_index, record) in history.records().iter().enumerate() {
        control.check_cancelled()?;
        if let Some(version) = record.algorithm_version() {
            if version
                != crate::processing::contracts::history::algorithm_version(record.requested())
            {
                return Err(
                    ProcessingError::UnsupportedHistoryVersion(version.into()).located(
                        Some(step_index),
                        Some(record.target()),
                        ProcessingPhase::Replay,
                    ),
                );
            }
        }
        replay_record(&mut state, record).map_err(|e| {
            ProcessingError::from(e).located(
                Some(step_index),
                Some(record.target()),
                ProcessingPhase::Replay,
            )
        })?;
        if let ProcessingRecord::Applied {
            requested,
            resolved,
            input_descriptor,
            output_descriptor,
            ..
        } = record
        {
            check_working_limit(
                input_descriptor,
                output_descriptor,
                match requested.target() {
                    OperationTarget::Axis(axis) => Some(axis),
                    _ => None,
                },
                resolved,
                options,
            )?;
            check_transform_work(input_descriptor, resolved, options)?;
            steps.push(PreparedStep {
                step_index,
                axis: match requested.target() {
                    OperationTarget::Axis(axis) => Some(axis),
                    _ => None,
                },
                before: input_descriptor.clone(),
                after: output_descriptor.clone(),
                resolved: (**resolved).clone(),
            });
        }
    }
    let descriptor = state.descriptor()?;
    check_resource(
        crate::resource::ResourceKind::OutputBytes,
        descriptor_bytes(&descriptor)?,
        options.max_output_bytes,
    )?;
    let prepared = PreparedRawPlan {
        phase: ProcessingPhase::Replay,
        resources: estimate_resources(&descriptor, &steps, options)?,
        descriptor: descriptor.clone(),
        steps,
        operations: Vec::new(),
        options,
        input_digest: input.digests().dataset(),
    };
    control.ensure_work(prepared.estimated_work()?)?;
    let (_, samples) = expand_raw(control, raw)?;
    let data = execute(control, samples, baseline, &prepared, options)?;
    let provenance = ProcessedProvenance::derived(
        ProcessedOrigin::DerivedRaw(DerivedRawOrigin::new(
            raw.snapshot_with_digests(input.digests()),
            raw.descriptor().axes().len(),
        )),
        raw.provenance().sources().to_vec(),
        history.clone(),
    );
    ProcessedDataset::new_derived_with_context(control, descriptor, data, provenance)
}

pub(crate) fn replay_processed_history(
    history: &ProcessingHistory,
    input: &ProcessedDataset,
    options: ProcessingOptions,
    control: &mut ExecutionContext<'_>,
) -> Result<ProcessedDataset, ProcessingError> {
    let options = reserve_prepared(
        history.initial_descriptor().axes().len(),
        history.records().len(),
        false,
        options,
    )?;
    let options = reserve_axes(checked_times(history_axis_bytes(history)?, 3)?, options)?;
    let options = reserve_states(history.initial_descriptor().axes().len(), 0, options)?;
    let options = reserve_containers(
        crate::processing::prepare::memory::processed_replay(input, history)?,
        options,
    )?;
    let HistoryInput::Processed {
        digests,
        sources,
        read_record,
    } = history.input()
    else {
        return Err(ProcessingError::InputIdentityMismatch);
    };
    if input.provenance().history().is_some()
        || *digests != input.canonical_digests()
        || read_record.as_ref() != input.provenance().read_record()
        || sources.len() != input.provenance().sources().len()
        || sources
            .iter()
            .zip(input.provenance().sources())
            .any(|(a, b)| {
                a.kind() != b.kind()
                    || a.role() != b.role()
                    || a.id() != b.id()
                    || a.digest() != b.digest()
            })
    {
        return Err(ProcessingError::InputIdentityMismatch);
    }
    if matches!(input.provenance().origin(), ProcessedOrigin::Imported) && read_record.is_none()
        || !matches!(
            input.provenance().origin(),
            ProcessedOrigin::External(_) | ProcessedOrigin::Library(_)
        ) && !sources.is_empty()
            && (read_record.is_none()
                || sources.iter().any(|source| {
                    source.id().is_none()
                        || matches!(
                            source.digest(),
                            crate::provenance::SourceDigest::NotComputed
                        )
                }))
    {
        return Err(ProcessingError::MissingCapability {
            capability: "complete processed reading record",
            axis: None,
        });
    }
    validate_processing_axes(input)?;
    let mut state = input.processing_state().clone();
    if !input
        .descriptor()
        .same_axes_except_labels(history.initial_descriptor())
    {
        return Err(ProcessingError::InputIdentityMismatch);
    }
    // Labels are presentation metadata, excluded from the scientific binding.
    for (axis, recorded) in state
        .axes
        .iter_mut()
        .zip(history.initial_descriptor().axes())
    {
        axis.axis = recorded.clone();
    }
    let mut steps = Vec::new();
    steps
        .try_reserve_exact(history.records().len())
        .map_err(|_| ProcessingError::AllocationFailure)?;
    for (step_index, record) in history.records().iter().enumerate() {
        control.check_cancelled()?;
        if let Some(version) = record.algorithm_version() {
            if version
                != crate::processing::contracts::history::algorithm_version(record.requested())
            {
                return Err(
                    ProcessingError::UnsupportedHistoryVersion(version.into()).located(
                        Some(step_index),
                        Some(record.target()),
                        ProcessingPhase::Replay,
                    ),
                );
            }
        }
        replay_record(&mut state, record).map_err(|e| {
            ProcessingError::from(e).located(
                Some(step_index),
                Some(record.target()),
                ProcessingPhase::Replay,
            )
        })?;
        if let ProcessingRecord::Applied {
            requested,
            resolved,
            input_descriptor,
            output_descriptor,
            ..
        } = record
        {
            check_working_limit(
                input_descriptor,
                output_descriptor,
                match requested.target() {
                    OperationTarget::Axis(axis) => Some(axis),
                    _ => None,
                },
                resolved,
                options,
            )?;
            check_transform_work(input_descriptor, resolved, options)?;
            steps.push(PreparedStep {
                step_index,
                axis: match requested.target() {
                    OperationTarget::Axis(axis) => Some(axis),
                    _ => None,
                },
                before: input_descriptor.clone(),
                after: output_descriptor.clone(),
                resolved: (**resolved).clone(),
            });
        }
    }
    let descriptor = state.descriptor()?;
    check_resource(
        crate::resource::ResourceKind::OutputBytes,
        descriptor_bytes(&descriptor)?,
        options.max_output_bytes,
    )?;
    if history
        .segments()
        .last()
        .map(|segment| segment.end_record())
        != Some(history.records().len())
    {
        return Err(ProcessingError::Mapping("incomplete execution segments"));
    }
    // A completed intermediate segment remains owned by this call while the
    // next segment clones and transforms its samples. It is not borrowed input.
    let mut begin = 0;
    let mut source_descriptor = history.initial_descriptor();
    for (index, segment) in history.segments().iter().enumerate() {
        let end = segment.end_record();
        if end < begin || end > history.records().len() {
            return Err(ProcessingError::Mapping(
                "invalid execution segment boundary",
            ));
        }
        let source_bytes = descriptor_bytes(source_descriptor)?;
        let retained = if index == 0 { 0 } else { source_bytes };
        check_sample_peak(source_bytes, retained, options)?;
        for record in &history.records()[begin..end] {
            if let ProcessingRecord::Applied {
                requested,
                resolved,
                input_descriptor,
                output_descriptor,
                ..
            } = record
            {
                check_sample_peak(
                    numerical_working_bytes(
                        input_descriptor,
                        output_descriptor,
                        match requested.target() {
                            OperationTarget::Axis(axis) => Some(axis),
                            _ => None,
                        },
                        resolved,
                    )?,
                    retained,
                    options,
                )?;
                source_descriptor = output_descriptor;
            }
        }
        begin = end;
    }
    let mut current: Option<ProcessedDataset> = None;
    let mut step_iter = steps.into_iter();
    let mut begin = 0;
    for segment in history.segments() {
        let end = segment.end_record();
        if end < begin || end > history.records().len() {
            return Err(ProcessingError::Mapping(
                "invalid execution segment boundary",
            ));
        }
        let count = history.records()[begin..end]
            .iter()
            .filter(|record| matches!(record, ProcessingRecord::Applied { .. }))
            .count();
        let mut steps = try_vec_capacity(count)?;
        steps.extend(step_iter.by_ref().take(count));
        let source = current.as_ref().unwrap_or(input);
        let descriptor = steps
            .last()
            .map_or_else(|| source.descriptor().clone(), |step| step.after.clone());
        let prepared = PreparedRawPlan {
            phase: ProcessingPhase::Replay,
            resources: estimate_resources(&descriptor, &steps, options)?,
            descriptor: descriptor.clone(),
            steps,
            operations: Vec::new(),
            options,
            input_digest: source.canonical_digests().dataset(),
        };
        control.ensure_work(prepared.estimated_work()?)?;
        let samples = clone_samples(source.data().samples(), control)?;
        let data = execute(
            control,
            samples,
            if begin == 0 {
                history.initial_descriptor().clone()
            } else {
                source.descriptor().clone()
            },
            &prepared,
            options,
        )?;
        let provenance =
            ProcessedProvenance::derived_from(source.provenance(), history.prefix(end));
        current = Some(ProcessedDataset::new_derived_with_context(
            control, descriptor, data, provenance,
        )?);
        begin = end;
    }
    current.ok_or(ProcessingError::Mapping("missing execution segment"))
}
