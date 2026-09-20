use crate::execution::{ExecutionContext, ExecutionStage};
use crate::processed::{
    DerivedRawOrigin, ProcessedDataset, ProcessedDescriptor, ProcessedOrigin, ProcessedProvenance,
};
use crate::processing::contracts::{
    error::*, history::*, operation::*, options::*, prepared_step::*, state::*,
};
use crate::processing::engine::execution::*;
use crate::processing::prepare::resources::*;
use crate::provenance::{CanonicalDatasetDigests, CanonicalDigest};
use crate::raw::{RawDataset, RawDescriptor, SamplingSchedule, SourceDigest, SourceKind};

/// Canonical sample-free input used by processing preflight.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessingInput {
    pub(in crate::processing) descriptor: RawDescriptor,
    pub(in crate::processing) sampling: Option<SamplingSchedule>,
    pub(in crate::processing) absolute_origin: Box<[usize]>,
    pub(in crate::processing) digests: CanonicalDatasetDigests,
    pub(in crate::processing) dense: bool,
    pub(in crate::processing) strong_source_identity: bool,
}

impl ProcessingInput {
    /// Builds canonical processing input without retaining format metadata.
    pub fn from_dataset(dataset: &RawDataset) -> Result<Self, ProcessingError> {
        Self::from_dataset_with_context(dataset, &mut ExecutionContext::default())
    }
    /// Computes a sample-free input with cancellable canonical hashing.
    pub fn from_dataset_with_context(
        dataset: &RawDataset,
        control: &mut ExecutionContext<'_>,
    ) -> Result<Self, ProcessingError> {
        control.check_cancelled()?;
        // RawDataset is immutable and validated by its constructors.
        let strong_source_identity = dataset.provenance().sources().iter().all(|source| {
            !matches!(source.kind(), SourceKind::Data)
                || matches!(source.digest(), SourceDigest::Sha256(_))
        });
        Ok(Self {
            descriptor: dataset.descriptor().clone(),
            sampling: dataset.sampling_schedule().cloned(),
            absolute_origin: dataset.data().layout().absolute_origin().into(),
            digests: crate::canonical_digest::dataset_digests_controlled(
                dataset,
                Some(control.cancellation()),
            )?,
            dense: !dataset.data().is_sparse(),
            strong_source_identity,
        })
    }
    /// Returns canonical acquisition semantics.
    pub fn descriptor(&self) -> &RawDescriptor {
        &self.descriptor
    }
    /// Returns absolute sampling coordinates in observation order.
    pub fn sampling_schedule(&self) -> Option<&SamplingSchedule> {
        self.sampling.as_ref()
    }
    /// Returns the frozen canonical identities.
    pub fn digests(&self) -> CanonicalDatasetDigests {
        self.digests
    }
}

/// A non-empty immutable sequence of processing requests.
/// Construction checks non-emptiness; [`Self::preflight`] is the authoritative
/// validation of request parameters, input state and resource bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessingPlan {
    pub(in crate::processing) operations: Vec<ProcessingOperation>,
}

impl ProcessingPlan {
    /// Resolves and executes with shared cancellation, progress and work accounting.
    pub fn apply_with_context(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.preflight_with_context(input, options, control)?
            .execute_with_context(control)
    }

    /// Previews a plan without consuming numerical execution budget.
    pub fn preflight_with_context<'a>(
        &self,
        input: &'a crate::Dataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<PreparedPlan<'a>, ProcessingError> {
        control.begin(ExecutionStage::Preflight, None, None)?;
        let prepared = self
            .preflight_inner(input, options, control)
            .map_err(|e| e.located(None, None, ProcessingPhase::Preflight))?;
        control.check_cancelled()?;
        Ok(prepared)
    }
    /// Resolves operations while borrowing an immutable aggregate input.
    /// Execution consumes the prepared plan and accepts no replacement input.
    pub fn preflight<'a>(
        &self,
        input: &'a crate::Dataset,
        options: ProcessingOptions,
    ) -> Result<PreparedPlan<'a>, ProcessingError> {
        self.preflight_with_context(input, options, &mut ExecutionContext::default())
    }
    pub(in crate::processing) fn preflight_inner<'a>(
        &self,
        input: &'a crate::Dataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<PreparedPlan<'a>, ProcessingError> {
        let rank = match input.descriptor() {
            crate::dataset::DescriptorRef::Raw(value) => value.axes().len(),
            crate::dataset::DescriptorRef::Processed(value) => value.axes().len(),
        };
        let mut options = reserve_prepared(rank, self.operations.len(), true, options)?;
        let axis_bytes = if let Some(raw) = input.as_raw() {
            reserved_raw_axis_bytes(raw, &self.operations)?
        } else {
            reserved_processed_axis_bytes(
                input.as_processed().expect("one representation"),
                &self.operations,
            )?
        };
        options = reserve_axes(axis_bytes, options)?;
        let map_points = if let Some(raw) = input.as_raw() {
            raw_map_points(raw)
        } else {
            input
                .as_processed()
                .expect("one representation")
                .processing_state()
                .observation_ordinals
                .as_ref()
                .map_or(0, |map| map.len())
        };
        options = reserve_states(rank, map_points, options)?;
        options = reserve_context(input.metadata(), options)?;
        options = reserve_containers(
            if let Some(raw) = input.as_raw() {
                crate::processing::prepare::memory::raw_apply(raw, self.operations.len())?
            } else {
                crate::processing::prepare::memory::processed_apply(
                    input.as_processed().expect("one representation"),
                    self.operations.len(),
                )?
            },
            options,
        )?;
        let (resolved, history) = if let Some(raw) = input.as_raw() {
            let input_description = ProcessingInput {
                descriptor: raw.descriptor().clone(),
                sampling: raw.sampling_schedule().cloned(),
                absolute_origin: raw.data().layout().absolute_origin().into(),
                digests: input.canonical_digests(),
                dense: !raw.data().is_sparse(),
                strong_source_identity: raw.provenance().sources().iter().all(|source| {
                    !matches!(source.kind(), SourceKind::Data)
                        || matches!(source.digest(), SourceDigest::Sha256(_))
                }),
            };
            (
                self.preflight_raw_with_context(&input_description, options, control)?,
                None,
            )
        } else {
            let history = history_state(
                input
                    .as_processed()
                    .expect("dataset has one representation"),
            )?;
            let resolved = preflight(
                control,
                history.state.clone(),
                &self.operations,
                options,
                input.canonical_digests().dataset(),
            )?;
            (resolved, Some(history))
        };
        Ok(PreparedPlan {
            input,
            resolved,
            history,
        })
    }
    /// Creates a plan, rejecting an empty operation list.
    pub fn new(operations: Vec<ProcessingOperation>) -> Result<Self, ProcessingError> {
        if operations.is_empty() {
            return Err(ProcessingError::EmptyPlan);
        }
        Ok(Self { operations })
    }

    /// Returns operations in execution order.
    pub fn operations(&self) -> &[ProcessingOperation] {
        &self.operations
    }

    /// Resolves raw operations and output descriptors without borrowing sample storage.
    /// The returned plan checks the supplied raw dataset identity at execution.
    pub fn preflight_raw(
        &self,
        input: &ProcessingInput,
        options: ProcessingOptions,
    ) -> Result<PreparedRawPlan, ProcessingError> {
        self.preflight_raw_with_context(input, options, &mut ExecutionContext::default())
    }
    /// Previews raw processing under cooperative control without consuming work.
    pub fn preflight_raw_with_context(
        &self,
        input: &ProcessingInput,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<PreparedRawPlan, ProcessingError> {
        control.check_cancelled()?;
        let rank = input.descriptor.axes().len();
        let options = reserve_prepared(rank, self.operations.len(), true, options)?;
        let options = reserve_axes(
            raw_plan_axis_bytes(input.descriptor.axes(), &self.operations)?,
            options,
        )?;
        let options = reserve_states(
            rank,
            input
                .sampling
                .as_ref()
                .map_or(0, |_| input.descriptor.axes()[0].points()),
            options,
        )?;
        if !(1..=2).contains(&rank) {
            return Err(ProcessingError::UnsupportedRank { rank });
        }
        if !input.dense {
            return Err(ProcessingError::MissingCapability {
                capability: "dense sampling",
                axis: None,
            });
        }
        if !input.strong_source_identity {
            return Err(ProcessingError::MissingCapability {
                capability: "complete source-data digest",
                axis: None,
            });
        }
        let baseline = expand_raw_descriptor(input.descriptor.axes())?;
        let state = PlanState::from_raw(
            &baseline,
            input.descriptor.axes(),
            input.sampling_schedule(),
            &input.absolute_origin,
        )?;
        preflight(
            control,
            state,
            &self.operations,
            options,
            input.digests.dataset(),
        )
    }

    /// Processes either representation and retains aggregate import context.
    pub fn apply(&self, input: &crate::Dataset) -> Result<crate::Dataset, ProcessingError> {
        self.apply_with_options(input, ProcessingOptions::default())
    }

    /// Processes either representation while preserving aggregate import context.
    pub fn apply_with_options(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.apply_with_context(input, options, &mut ExecutionContext::default())
    }

    /// Expands and processes a dense rank-1 or rank-2 raw dataset.
    pub fn apply_raw(&self, raw: &RawDataset) -> Result<ProcessedDataset, ProcessingError> {
        self.apply_raw_with_options(raw, ProcessingOptions::default())
    }

    /// Expands and processes a raw dataset with explicit resource limits.
    pub fn apply_raw_with_options(
        &self,
        raw: &RawDataset,
        options: ProcessingOptions,
    ) -> Result<ProcessedDataset, ProcessingError> {
        self.apply_raw_with_context(raw, options, &mut ExecutionContext::default())
    }

    /// Executes with cooperative cancellation, progress and a shared numerical budget.
    pub fn apply_raw_with_context(
        &self,
        raw: &RawDataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<ProcessedDataset, ProcessingError> {
        let options = reserve_prepared(
            raw.descriptor().axes().len(),
            self.operations.len(),
            true,
            options,
        )?;
        let options = reserve_axes(reserved_raw_axis_bytes(raw, &self.operations)?, options)?;
        let options = reserve_states(raw.descriptor().axes().len(), raw_map_points(raw), options)?;
        let options = reserve_containers(
            crate::processing::prepare::memory::raw_apply(raw, self.operations.len())?,
            options,
        )?;
        let input = ProcessingInput::from_dataset_with_context(raw, control)?;
        let prepared = self.preflight_raw_with_context(&input, options, control)?;
        prepared.apply_with_context(raw, control)
    }

    /// Applies additional processing to a dense processed dataset.
    pub fn apply_processed(
        &self,
        dataset: &ProcessedDataset,
    ) -> Result<ProcessedDataset, ProcessingError> {
        self.apply_processed_with_options(dataset, ProcessingOptions::default())
    }

    /// Applies additional processing with explicit resource limits.
    pub fn apply_processed_with_options(
        &self,
        dataset: &ProcessedDataset,
        options: ProcessingOptions,
    ) -> Result<ProcessedDataset, ProcessingError> {
        self.apply_processed_with_context(dataset, options, &mut ExecutionContext::default())
    }

    /// Executes with cooperative cancellation, progress and a shared numerical budget.
    pub fn apply_processed_with_context(
        &self,
        dataset: &ProcessedDataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<ProcessedDataset, ProcessingError> {
        // Immutable datasets already carry aggregate-validated current state.
        // Continuing a plan does not need to replay the old history or rescan samples.
        let options = reserve_prepared(
            dataset.descriptor().axes().len(),
            self.operations.len(),
            true,
            options,
        )?;
        let options = reserve_axes(
            reserved_processed_axis_bytes(dataset, &self.operations)?,
            options,
        )?;
        let options = reserve_states(
            dataset.descriptor().axes().len(),
            dataset
                .processing_state()
                .observation_ordinals
                .as_ref()
                .map_or(0, |map| map.len()),
            options,
        )?;
        let options = reserve_containers(
            crate::processing::prepare::memory::processed_apply(dataset, self.operations.len())?,
            options,
        )?;
        let history = history_state(dataset)?;
        let prepared =
            preflight_processed(control, history.state.clone(), &self.operations, options)?;
        apply_processed_prepared(control, dataset, prepared, history)
    }
}

/// A prepared execution borrowing its immutable aggregate input.
///
/// The input must outlive execution:
/// ```compile_fail
/// # use nmr::{Dataset, processing::{ProcessingPlan, ProcessingOptions}};
/// fn invalid(plan: ProcessingPlan, input: Dataset) {
///     let prepared = plan.preflight(&input, ProcessingOptions::default()).unwrap();
///     drop(input);
///     prepared.execute().unwrap();
/// }
/// ```
/// Execution consumes the prepared object:
/// ```compile_fail
/// # use nmr::{Dataset, processing::{ProcessingPlan, ProcessingOptions}};
/// fn invalid(plan: ProcessingPlan, input: &Dataset) {
///     let prepared = plan.preflight(input, ProcessingOptions::default()).unwrap();
///     prepared.execute().unwrap();
///     prepared.execute().unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct PreparedPlan<'a> {
    pub(in crate::processing) input: &'a crate::Dataset,
    pub(in crate::processing) resolved: PreparedRawPlan,
    pub(in crate::processing) history: Option<HistoryState>,
}

impl PreparedPlan<'_> {
    /// Complete deterministic work reservation for this explicit segment.
    pub fn estimated_work(&self) -> Result<u128, ProcessingError> {
        self.resolved.estimated_work()
    }

    /// Borrows the frozen requests, parameters and descriptor transitions in order.
    pub fn steps(&self) -> impl ExactSizeIterator<Item = PreparedStepView<'_>> {
        self.resolved.steps()
    }

    /// Returns conservative bounds for the borrowed input and resolved operations.
    pub fn resources(&self) -> crate::resource::ResourceEstimate {
        self.resolved.resources
    }
    /// Returns the descriptor resolved for the output.
    pub fn descriptor(&self) -> &ProcessedDescriptor {
        self.resolved.descriptor()
    }
    /// Returns the immutable input's canonical dataset digest.
    pub fn input_digest(&self) -> CanonicalDigest {
        self.input.canonical_digests().dataset()
    }
    /// Executes the already-resolved steps on the borrowed input exactly once.
    pub fn execute(self) -> Result<crate::Dataset, ProcessingError> {
        self.execute_with_context(&mut ExecutionContext::default())
    }

    /// Executes with cooperative cancellation and a caller-owned work budget.
    pub fn execute_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        control.ensure_work(self.resolved.estimated_work()?)?;
        control.observe_payload(self.resources().working_bytes());
        let output = match self.input.as_raw() {
            Some(raw) => self
                .resolved
                .apply_bound(raw, self.input.canonical_digests(), control)?,
            None => apply_processed_prepared(
                control,
                self.input
                    .as_processed()
                    .expect("dataset has one representation"),
                self.resolved,
                self.history.expect("processed preparation retains history"),
            )?,
        };
        Ok(self.input.derived_processed(output))
    }
}

/// Low-level resolved operations bound to one canonical raw dataset identity.
/// Owns the resolved steps without borrowing samples; inspect the output descriptor
/// before applying to a raw dataset with the matching identity. Execution rechecks
/// allocations for the supplied dataset's display metadata and provenance.
/// Prefer [`ProcessingPlan::preflight`] and [`PreparedPlan::execute`] for a borrowed aggregate.
#[derive(Clone, Debug)]
pub struct PreparedRawPlan {
    pub(in crate::processing) phase: ProcessingPhase,
    pub(in crate::processing) descriptor: ProcessedDescriptor,
    pub(in crate::processing) steps: Vec<PreparedStep>,
    pub(in crate::processing) operations: Vec<ProcessingOperation>,
    pub(in crate::processing) options: ProcessingOptions,
    pub(in crate::processing) input_digest: CanonicalDigest,
    pub(in crate::processing) resources: crate::resource::ResourceEstimate,
}

impl PreparedRawPlan {
    /// Conservative numerical work, excluding I/O and memory estimates.
    pub fn estimated_work(&self) -> Result<u128, ProcessingError> {
        self.steps.iter().try_fold(
            (descriptor_bytes(self.steps.first().map_or(&self.descriptor, |s| &s.before))? / 8)
                as u128,
            |total, step| {
                total
                    .checked_add(step_work(step)?)
                    .ok_or(ProcessingError::SizeOverflow)
            },
        )
    }
    /// Borrows the frozen requests, parameters and descriptor transitions in order.
    pub fn steps(&self) -> impl ExactSizeIterator<Item = PreparedStepView<'_>> {
        self.operations
            .iter()
            .zip(&self.steps)
            .map(|(requested, prepared)| PreparedStepView {
                requested,
                prepared,
            })
    }

    /// Returns the conservative resource bounds known during raw preflight.
    /// Application rechecks additional provenance and display metadata in the raw value.
    pub fn resources(&self) -> crate::resource::ResourceEstimate {
        self.resources
    }

    /// Returns the descriptor produced after all prepared operations.
    pub fn descriptor(&self) -> &ProcessedDescriptor {
        &self.descriptor
    }
    /// Returns the canonical dataset digest accepted by this plan.
    pub fn input_digest(&self) -> CanonicalDigest {
        self.input_digest
    }

    /// Executes these resolved operations only on the dataset used for preflight.
    pub fn apply(&self, raw: &RawDataset) -> Result<ProcessedDataset, ProcessingError> {
        self.apply_with_context(raw, &mut ExecutionContext::default())
    }

    /// Executes this bound raw plan using the caller's execution context.
    pub fn apply_with_context(
        &self,
        raw: &RawDataset,
        control: &mut ExecutionContext<'_>,
    ) -> Result<ProcessedDataset, ProcessingError> {
        control.ensure_work(self.estimated_work()?)?;
        control.observe_payload(self.resources.working_bytes());
        let digests = raw.canonical_digests();
        if digests.dataset() != self.input_digest {
            return Err(ProcessingError::InputIdentityMismatch);
        }
        self.apply_bound(raw, digests, control)
    }

    pub(in crate::processing) fn apply_bound(
        &self,
        raw: &RawDataset,
        digests: CanonicalDatasetDigests,
        control: &mut ExecutionContext<'_>,
    ) -> Result<ProcessedDataset, ProcessingError> {
        let options = reserve_containers(
            crate::processing::prepare::memory::raw_apply(raw, self.operations.len())?,
            self.options,
        )?;
        let baseline = self
            .steps
            .first()
            .ok_or(ProcessingError::EmptyPlan)?
            .before
            .clone();
        // A raw caller may supply larger display metadata under the same
        // canonical identity. Recheck its new raw baselines before constructing them.
        let stored = axis_storage(baseline.axes().iter())?;
        let retained_baseline = axis_reservation(stored, std::iter::empty(), true)?;
        let operations = axis_reservation(stored, self.operations.iter(), false)?;
        let current_baseline = axis_reservation(
            raw_axis_storage(raw.descriptor().axes())?,
            std::iter::empty(),
            true,
        )?;
        let required_axes = checked_sum(&[
            retained_baseline,
            checked_times(operations, 2)?,
            checked_times(current_baseline, 3)?,
        ])?;
        let options = reserve_axes(required_axes, options)?;
        if options.axis_bytes > self.options.axis_bytes
            || options.container_bytes > self.options.container_bytes
        {
            for step in &self.steps {
                check_working_limit(
                    &step.before,
                    &step.after,
                    step.axis,
                    &step.resolved,
                    options,
                )?;
            }
        }
        if !baseline.same_axes_except_labels(&expand_raw_descriptor(raw.descriptor().axes())?) {
            return Err(ProcessingError::InputIdentityMismatch);
        }
        let origin = ProcessedOrigin::DerivedRaw(DerivedRawOrigin::new(
            raw.snapshot_with_digests(digests),
            baseline.axes().len(),
        ));
        let (_, samples) = expand_raw(control, raw)?;
        let data = execute(control, samples, baseline.clone(), self, options)?;
        let records = self
            .operations
            .iter()
            .zip(&self.steps)
            .map(|(requested, prepared)| {
                ProcessingRecord::applied(
                    requested.clone(),
                    prepared.resolved.clone(),
                    prepared.before.clone(),
                    prepared.after.clone(),
                    Vec::new(),
                )
            })
            .collect();
        let history = ProcessingHistory::new(
            baseline,
            HistoryInput::Raw {
                digests,
                normalization: raw.provenance().sample_normalization().clone(),
                format: raw.provenance().format(),
                sources: raw.provenance().sources().to_vec(),
            },
            records,
        );
        let provenance =
            ProcessedProvenance::derived(origin, raw.provenance().sources().to_vec(), history);
        ProcessedDataset::new_derived_with_context(
            control,
            self.descriptor.clone(),
            data,
            provenance,
        )
    }
}

pub(in crate::processing) fn preflight(
    control: &mut ExecutionContext<'_>,
    mut state: PlanState,
    operations: &[ProcessingOperation],
    options: ProcessingOptions,
    input_digest: CanonicalDigest,
) -> Result<PreparedRawPlan, ProcessingError> {
    let options = reserve_prepared(state.axes.len(), operations.len(), true, options)?;
    let options = reserve_axes(
        axis_reservation(
            axis_storage(state.axes.iter().map(|state| &state.axis))?,
            operations.iter(),
            false,
        )?,
        options,
    )?;
    let options = reserve_states(
        state.axes.len(),
        state
            .observation_ordinals
            .as_ref()
            .map_or(0, |map| map.len()),
        options,
    )?;
    #[cfg(test)]
    PREPARED_RESERVES.with(|count| count.set(count.get() + 1));
    let mut steps = Vec::new();
    steps
        .try_reserve_exact(operations.len())
        .map_err(|_| ProcessingError::AllocationFailure)?;
    for (step_index, operation) in operations.iter().enumerate() {
        let step = (|| -> Result<PreparedStep, ProcessingError> {
            control.check_cancelled()?;
            let before = state.descriptor()?;
            let resolved = transition(&mut state, operation)?;
            let after = state.descriptor()?;
            check_working_limit(&before, &after, operation.axis(), &resolved, options)?;
            check_transform_work(&before, &resolved, options)?;
            Ok(PreparedStep {
                step_index,
                axis: operation.axis(),
                before,
                after,
                resolved,
            })
        })()
        .map_err(|error| {
            error.located(
                Some(step_index),
                Some(operation.target()),
                ProcessingPhase::Preflight,
            )
        })?;
        steps.push(step);
    }
    let descriptor = state.descriptor()?;
    check_resource(
        crate::resource::ResourceKind::OutputBytes,
        descriptor_bytes(&descriptor)?,
        options.max_output_bytes,
    )?;
    Ok(PreparedRawPlan {
        phase: ProcessingPhase::Execution,
        resources: estimate_resources(&descriptor, &steps, options)?,
        descriptor,
        steps,
        operations: operations.to_vec(),
        options,
        input_digest,
    })
}

pub(in crate::processing) fn check_transform_work(
    before: &ProcessedDescriptor,
    resolved: &ResolvedOperation,
    options: ProcessingOptions,
) -> Result<(), ProcessingError> {
    if let ResolvedOperation::ComponentTransform { transform, .. } = resolved {
        let logical = before
            .axes()
            .iter()
            .try_fold(1u128, |value, axis| {
                value.checked_mul(axis.points() as u128)
            })
            .ok_or(ProcessingError::SizeOverflow)?;
        let work = logical
            .checked_mul(transform.input_lanes() as u128)
            .and_then(|value| value.checked_mul(2))
            .ok_or(ProcessingError::SizeOverflow)?;
        if work > options.max_transform_work {
            return Err(ProcessingError::WorkLimit);
        }
    }
    Ok(())
}

pub(in crate::processing) fn preflight_processed(
    control: &mut ExecutionContext<'_>,
    state: PlanState,
    operations: &[ProcessingOperation],
    options: ProcessingOptions,
) -> Result<PreparedRawPlan, ProcessingError> {
    preflight(control, state, operations, options, CanonicalDigest::zero())
}
