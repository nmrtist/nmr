use crate::Complex64;
use crate::execution::{ExecutionContext, ExecutionStage, ProgressTotal};
use crate::internal::numeric::scaled_l2;
use crate::processed::{ComponentBasis, ProcessedDataset};
use crate::processing::contracts::polarity::PolarityState;
use crate::processing::contracts::{
    error::*, history::*, operation::*, options::*, prepared_step::*, profile::*, state::*,
};
use crate::processing::engine::execution::*;
use crate::processing::kernels::acme::PhaseOptimizationError;
use crate::processing::kernels::buffer::*;
use crate::processing::kernels::tensor::*;
use crate::processing::prepare::resources::*;
use crate::provenance::CanonicalDigest;

/// A bounded automatic-phase analysis borrowing its immutable aggregate input.
///
/// Resource bounds cover selection, optimization, correction or the requested
/// fallback, and new history/context. Preparation does not inspect samples or
/// assert phase quality. Execution consumes this object and checks the supplied
/// work ledger before selecting a trace or copying history.
#[derive(Debug)]
pub struct PreparedAutoPhase<'a> {
    pub(in crate::processing) input: &'a crate::Dataset,
    pub(in crate::processing) profile: NormalizedAcmeV1,
    pub(in crate::processing) axis: usize,
    pub(in crate::processing) polarity: PolarityState,
    pub(in crate::processing) failure_policy: PhaseFailurePolicy,
    pub(in crate::processing) budget: AutoPhaseBudget,
}

impl PreparedAutoPhase<'_> {
    /// Returns the conservative capacity bounds, excluding borrowed input storage.
    pub fn resources(&self) -> crate::resource::ResourceEstimate {
        self.budget.resources
    }

    /// Returns the maximum selection, optimizer and correction work for this call.
    pub fn estimated_work(&self) -> u128 {
        self.budget.work
    }

    /// Returns the immutable input identity accepted by this prepared analysis.
    pub fn input_digest(&self) -> CanonicalDigest {
        self.input.canonical_digests().dataset()
    }

    /// Returns the selected current descriptor position.
    pub fn target(&self) -> OperationTarget {
        OperationTarget::Axis(self.axis)
    }

    /// Runs bounded analysis and records the actual correction or allowed failure.
    /// New unverified-quality failures always remain errors, even under fallback.
    pub fn execute(self) -> Result<crate::Dataset, ProcessingError> {
        self.execute_with_context(&mut ExecutionContext::default())
    }
    /// Executes automatic phasing with shared execution control.
    pub fn execute_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        let target = OperationTarget::Axis(self.axis);
        let result = execute_normalized_acme(
            self.profile,
            self.input.as_processed().expect("checked processed input"),
            self.axis,
            self.polarity,
            self.failure_policy,
            control,
            self.budget,
        )
        .map_err(|e| e.located(Some(0), Some(target), ProcessingPhase::Execution))?;
        Ok(self.input.derived_processed(result))
    }
}

/// Named automatic-phase request; optimization occurs only during execution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoPhaseRequest {
    /// Current Cartesian frequency axis to phase.
    pub axis: crate::AxisIndex,
    /// Caller-established peak polarity.
    pub polarity: PolarityState,
    /// Explicit behavior on a recoverable phase failure.
    pub failure_policy: PhaseFailurePolicy,
}

impl NormalizedAcmeV1 {
    /// Previews bounded automatic phasing without inspecting samples or spending work.
    pub fn preflight<'a>(
        self,
        input: &'a crate::Dataset,
        request: AutoPhaseRequest,
        options: ProcessingOptions,
    ) -> Result<PreparedAutoPhase<'a>, ProcessingError> {
        let AutoPhaseRequest {
            axis,
            polarity,
            failure_policy,
        } = request;
        let axis = axis.index();
        let dataset = input.as_processed().ok_or(ProcessingError::Mapping(
            "automatic phasing requires processed input",
        ))?;
        let options = reserve_context(input.metadata(), options)?;
        Ok(PreparedAutoPhase {
            input,
            profile: self,
            axis,
            polarity,
            failure_policy,
            budget: auto_phase_budget(dataset, axis, options)?,
        })
    }
    /// Applies automatic phasing while preserving complete aggregate context.
    pub fn apply(
        self,
        input: &crate::Dataset,
        request: AutoPhaseRequest,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.apply_with_context(
            input,
            request,
            ProcessingOptions::default(),
            &mut ExecutionContext::default(),
        )
    }
    /// Applies automatic phasing with cancellation, progress and a shared work budget.
    pub fn apply_with_context(
        self,
        input: &crate::Dataset,
        request: AutoPhaseRequest,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        control.begin(ExecutionStage::Preflight, Some(0), None)?;
        self.preflight(input, request, options)
            .map_err(|error| {
                error.located(
                    Some(0),
                    Some(OperationTarget::Axis(request.axis.index())),
                    ProcessingPhase::Preflight,
                )
            })?
            .execute_with_context(control)
    }
    /// Applies automatic phasing to a low-level processed value.
    pub fn apply_processed(
        self,
        input: &ProcessedDataset,
        request: AutoPhaseRequest,
    ) -> Result<ProcessedDataset, ProcessingError> {
        self.apply_processed_with_context(
            input,
            request,
            ProcessingOptions::default(),
            &mut ExecutionContext::default(),
        )
    }
    /// Applies automatic phasing to a low-level value under shared execution control.
    pub fn apply_processed_with_context(
        self,
        input: &ProcessedDataset,
        request: AutoPhaseRequest,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<ProcessedDataset, ProcessingError> {
        apply_normalized_acme(
            self,
            input,
            request.axis.index(),
            request.polarity,
            request.failure_policy,
            control,
            options,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::processing) struct AutoPhaseBudget {
    pub(in crate::processing) options: ProcessingOptions,
    pub(in crate::processing) resources: crate::resource::ResourceEstimate,
    pub(in crate::processing) work: u128,
}

pub(in crate::processing) fn auto_phase_budget(
    dataset: &ProcessedDataset,
    axis: usize,
    options: ProcessingOptions,
) -> Result<AutoPhaseBudget, ProcessingError> {
    // Immutable aggregate constructors already validated samples and history.
    // All checks here read existing state and sizes without cloning either.
    validate_processing_axes(dataset)?;
    validate_auto_phase_axis(dataset.processing_state(), axis)?;
    let descriptor = dataset.descriptor();
    let rank = descriptor.axes().len();
    let points = descriptor.axes()[axis].points();
    let requests = [
        ProcessingOperation::PhaseCorrection {
            axis,
            correction: PhaseCorrection::new(0.0, 0.0, 0.0)?,
        },
        ProcessingOperation::Projection {
            projection: Projection::UnphasedReal,
            polarity: PolarityState::Ambiguous180,
        },
    ];
    let options = reserve_prepared(rank, requests.len(), true, options)?;
    let options = reserve_axes(reserved_processed_axis_bytes(dataset, &requests)?, options)?;
    let options = reserve_states(
        rank,
        dataset
            .processing_state()
            .observation_ordinals
            .as_ref()
            .map_or(0, |map| map.len()),
        options,
    )?;
    let options = reserve_containers(
        checked_sum(&[
            crate::processing::prepare::memory::processed_apply(dataset, requests.len())?,
            std::mem::size_of::<ProcessingDiagnostic>(),
        ])?,
        options,
    )?;
    let output = descriptor_bytes(descriptor)?;
    check_resource(
        crate::resource::ResourceKind::OutputBytes,
        output,
        options.max_output_bytes,
    )?;
    let trace = checked_times(points, std::mem::size_of::<Complex64>())?;
    let other_components = descriptor
        .axes()
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != axis)
        .try_fold(1usize, |count, (_, axis)| {
            count.checked_mul(axis.component_count())
        })
        .ok_or(ProcessingError::SizeOverflow)?;
    // Selection retains its best trace alongside a candidate and all fields.
    let selection_peak = checked_sum(&[
        checked_times(trace, 2)?,
        checked_times(trace, other_components)?,
        checked_times(rank, 10 * std::mem::size_of::<usize>())?,
    ])?;
    let optimizer_peak = checked_sum(&[
        trace,
        crate::processing::kernels::acme::working_bytes(points)?,
    ])?;
    let correction_peak = checked_sum(&[checked_times(output, 2)?, trace])?;
    let numerical = selection_peak.max(optimizer_peak).max(correction_peak);
    check_sample_peak(numerical, 0, options)?;
    let correction_work = dataset.data().samples().len() as u128;
    let selection_work = correction_work
        .checked_mul(2)
        .ok_or(ProcessingError::SizeOverflow)?;
    let work = selection_work
        .checked_add(crate::processing::kernels::acme::estimated_work(points)?)
        .and_then(|work| work.checked_add(correction_work))
        .ok_or(ProcessingError::SizeOverflow)?;
    let metadata = metadata_reservation(options)?;
    Ok(AutoPhaseBudget {
        options,
        resources: crate::resource::ResourceEstimate::new(
            output,
            metadata,
            checked_sum(&[metadata, numerical])?,
        ),
        work,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_normalized_acme(
    profile: NormalizedAcmeV1,
    dataset: &ProcessedDataset,
    axis: usize,
    polarity: PolarityState,
    failure_policy: PhaseFailurePolicy,
    control: &mut ExecutionContext<'_>,
    options: ProcessingOptions,
) -> Result<ProcessedDataset, ProcessingError> {
    let budget = auto_phase_budget(dataset, axis, options)?;
    execute_normalized_acme(
        profile,
        dataset,
        axis,
        polarity,
        failure_policy,
        control,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::processing) fn execute_normalized_acme(
    profile: NormalizedAcmeV1,
    dataset: &ProcessedDataset,
    axis: usize,
    polarity: PolarityState,
    failure_policy: PhaseFailurePolicy,
    control: &mut ExecutionContext<'_>,
    budget: AutoPhaseBudget,
) -> Result<ProcessedDataset, ProcessingError> {
    // An insufficient ledger must fail before copying history, selecting traces,
    // or performing the first optimizer allocation. Reservation is conservative;
    // the original ledger is charged only for work actually entered below.
    control.ensure_work(budget.work)?;
    control.observe_payload(budget.resources.working_bytes());
    control.begin(
        ExecutionStage::AutoPhase,
        Some(0),
        Some(ProgressTotal::UpperBound(budget.work)),
    )?;
    let options = budget.options;
    let HistoryState {
        initial_descriptor: base,
        input,
        mut records,
        mut state,
    } = history_state(dataset)?;
    records
        .try_reserve_exact(2)
        .map_err(|_| ProcessingError::AllocationFailure)?;
    validate_auto_phase_axis(&state, axis)?;
    let trace = representative_trace_with_context(dataset, axis, control)?;
    let request = ProcessingRequest::AutoPhase {
        axis,
        profile,
        polarity,
        failure_policy,
    };
    let solution = profile.optimize_with_context(&trace, polarity, control);
    drop(trace);
    match solution {
        Ok(solution) => {
            let before = state.descriptor()?;
            transition_auto_phase(&mut state, axis)?;
            let after = state.descriptor()?;
            let resolved = ResolvedOperation::PhaseCorrection(solution.correction());
            check_working_limit(&before, &after, Some(axis), &resolved, options)?;
            check_resource(
                crate::resource::ResourceKind::OutputBytes,
                descriptor_bytes(&after)?,
                options.max_output_bytes,
            )?;
            let step = PreparedStep {
                step_index: 0,
                axis: Some(axis),
                before,
                after: after.clone(),
                resolved: resolved.clone(),
            };

            let samples = execute_step(control, dataset.data().samples(), &step)?;
            records.push(ProcessingRecord::applied(
                request,
                resolved,
                step.before.clone(),
                step.after.clone(),
                Vec::new(),
            ));
            derived_dataset(control, dataset, base, input, records, after, samples)
        }
        Err(error) => {
            let Some(failure) = recoverable_phase_failure(error) else {
                return Err(error.into());
            };
            if failure_policy == PhaseFailurePolicy::Fail {
                return Err(error.into());
            }
            state.record_phase_failure(axis)?;
            records.push(ProcessingRecord::attempted(request, failure, Vec::new()));
            let projection_request = ProcessingOperation::Projection {
                projection: Projection::UnphasedReal,
                polarity,
            };
            let before = state.descriptor()?;
            let resolved = transition_projection(&mut state, Projection::UnphasedReal, polarity)?;
            let after = state.descriptor()?;
            check_working_limit(&before, &after, None, &resolved, options)?;
            check_resource(
                crate::resource::ResourceKind::OutputBytes,
                descriptor_bytes(&after)?,
                options.max_output_bytes,
            )?;
            let step = PreparedStep {
                step_index: 0,
                axis: None,
                before,
                after: after.clone(),
                resolved: resolved.clone(),
            };

            let samples = execute_step(control, dataset.data().samples(), &step)?;
            records.push(ProcessingRecord::applied(
                projection_request,
                resolved,
                step.before.clone(),
                step.after.clone(),
                vec![ProcessingDiagnostic::ContinuedAfterPhaseFailure],
            ));
            derived_dataset(control, dataset, base, input, records, after, samples)
        }
    }
}

pub(in crate::processing) fn recoverable_phase_failure(
    error: PhaseOptimizationError,
) -> Option<AttemptFailure> {
    match error {
        PhaseOptimizationError::NoUsableSignal => Some(AttemptFailure::NoUsableSignal),
        PhaseOptimizationError::ObjectiveUndefined => Some(AttemptFailure::ObjectiveUndefined),
        PhaseOptimizationError::OptimizationDidNotConverge => {
            Some(AttemptFailure::OptimizationDidNotConverge)
        }
        PhaseOptimizationError::NumericalInvariantViolation
        | PhaseOptimizationError::QualityUnverified
        | PhaseOptimizationError::WorkLimit
        | PhaseOptimizationError::Execution(_)
        | PhaseOptimizationError::AllocationFailure => None,
    }
}

pub(in crate::processing) fn representative_trace_with_context(
    dataset: &ProcessedDataset,
    axis: usize,
    control: &mut ExecutionContext<'_>,
) -> Result<Vec<Complex64>, ProcessingError> {
    #[cfg(test)]
    REPRESENTATIVE_SELECTIONS.with(|count| count.set(count.get() + 1));
    let descriptor = dataset.descriptor();
    let selected = descriptor
        .axes()
        .get(axis)
        .ok_or(ProcessingError::InvalidState {
            axis,
            operation: "representative selection",
            reason: "axis index is out of bounds",
        })?;
    if !matches!(selected.component_basis(), ComponentBasis::Cartesian) {
        return Err(ProcessingError::InvalidState {
            axis,
            operation: "representative selection",
            reason: "requires one Cartesian component pair",
        });
    }
    let shape = descriptor.logical_shape();
    let component_counts = descriptor.component_counts();
    let line_count = other_line_count(&shape, axis)?;
    let other_component_count = component_counts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != axis)
        .try_fold(1usize, |total, (_, count)| total.checked_mul(*count))
        .ok_or(ProcessingError::SizeOverflow)?;
    let mut best: Option<(f64, f64, Vec<Complex64>)> = None;
    for line in 0..line_count {
        control.check_cancelled()?;
        let other_logical = other_coordinate(&shape, axis, line)?;
        let mut canonical_components = try_zeroed_usize(shape.len())?;
        let mut trace = try_vec_capacity(selected.points())?;
        for point in 0..selected.points() {
            charge_block(control, point, selected.points())?;
            let logical = merge_coordinate(shape.len(), axis, &other_logical, point)?;
            canonical_components[axis] = 0;
            let real = tensor_sample(
                dataset.data().samples(),
                descriptor,
                &logical,
                &canonical_components,
            )?;
            canonical_components[axis] = 1;
            let imaginary = tensor_sample(
                dataset.data().samples(),
                descriptor,
                &logical,
                &canonical_components,
            )?;
            trace.push(Complex64::new(real, imaginary));
        }
        let primary = scaled_l2(trace.iter().flat_map(|value| [value.re, value.im]));
        let mut all_fields = Vec::new();
        all_fields
            .try_reserve_exact(
                selected
                    .points()
                    .checked_mul(2)
                    .and_then(|value| value.checked_mul(other_component_count))
                    .ok_or(ProcessingError::SizeOverflow)?,
            )
            .map_err(|_| ProcessingError::AllocationFailure)?;
        for other_component in 0..other_component_count {
            let compact_counts = component_counts
                .iter()
                .enumerate()
                .filter_map(|(index, count)| (index != axis).then_some(*count))
                .collect::<Vec<_>>();
            let compact = if compact_counts.is_empty() {
                Vec::new()
            } else {
                unflatten(&compact_counts, other_component)?
            };
            for point in 0..selected.points() {
                charge_block(control, point, selected.points())?;
                let logical = merge_coordinate(shape.len(), axis, &other_logical, point)?;
                for selected_component in 0..2 {
                    let components =
                        merge_coordinate(shape.len(), axis, &compact, selected_component)?;
                    all_fields.push(tensor_sample(
                        dataset.data().samples(),
                        descriptor,
                        &logical,
                        &components,
                    )?);
                }
            }
        }
        let total = scaled_l2(all_fields);
        let replace = best.as_ref().is_none_or(|(best_primary, best_total, _)| {
            primary.total_cmp(best_primary) == std::cmp::Ordering::Greater
                || primary.total_cmp(best_primary) == std::cmp::Ordering::Equal
                    && total.total_cmp(best_total) == std::cmp::Ordering::Greater
        });
        if replace {
            best = Some((primary, total, trace));
        }
    }
    best.map(|(_, _, trace)| trace)
        .ok_or(ProcessingError::Mapping("no representative trace"))
}
