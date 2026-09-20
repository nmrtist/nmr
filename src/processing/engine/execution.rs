use crate::acquisition::{DirectSamples, RawAxisKind};
use crate::execution::{ExecutionContext, ExecutionStage, ProgressTotal};
use crate::processed::{ProcessedData, ProcessedDataset, ProcessedDescriptor, ProcessedProvenance};
use crate::processing::contracts::{
    error::*, history::*, operation::*, options::*, prepared_step::*, state::*,
};
use crate::processing::kernels::buffer::*;
use crate::processing::kernels::operations::*;
use crate::processing::kernels::spectrum;
use crate::processing::kernels::tensor::*;
use crate::processing::prepare::plan::*;
use crate::processing::prepare::resources::*;
use crate::raw::RawDataset;
use std::f64::consts::PI;

pub(in crate::processing) fn apply_processed_prepared(
    control: &mut ExecutionContext<'_>,
    dataset: &ProcessedDataset,
    prepared: PreparedRawPlan,
    history: HistoryState,
) -> Result<ProcessedDataset, ProcessingError> {
    let HistoryState {
        initial_descriptor: base,
        input,
        records: mut previous_records,
        ..
    } = history;
    control.ensure_work(prepared.estimated_work()?)?;
    let samples = clone_samples(dataset.data().samples(), control)?;
    let data = execute(
        control,
        samples,
        dataset.descriptor().clone(),
        &prepared,
        prepared.options,
    )?;
    previous_records
        .try_reserve_exact(prepared.operations.len())
        .map_err(|_| ProcessingError::AllocationFailure)?;
    previous_records.extend(prepared.operations.iter().zip(&prepared.steps).map(
        |(requested, step)| {
            ProcessingRecord::applied(
                requested.clone(),
                step.resolved.clone(),
                step.before.clone(),
                step.after.clone(),
                Vec::new(),
            )
        },
    ));
    let history = ProcessingHistory::new(base, input, previous_records);
    let provenance = ProcessedProvenance::derived_from(dataset.provenance(), history);
    ProcessedDataset::new_derived_with_context(control, prepared.descriptor, data, provenance)
}

#[cfg(test)]
thread_local! {
    pub(in crate::processing) static RAW_EXPANSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(in crate::processing) static PREPARED_RESERVES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(in crate::processing) static HISTORY_SEEDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(in crate::processing) static REPRESENTATIVE_SELECTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(in crate::processing) fn expand_raw(
    control: &mut ExecutionContext<'_>,
    raw: &RawDataset,
) -> Result<(ProcessedDescriptor, Vec<f64>), ProcessingError> {
    #[cfg(test)]
    RAW_EXPANSIONS.with(|count| count.set(count.get() + 1));
    let descriptor = expand_raw_descriptor(raw.descriptor().axes())?;
    let input = raw
        .data()
        .dense_samples()
        .ok_or(ProcessingError::Mapping("raw data is sparse"))?;
    let mut raw_shape = try_vec_capacity(raw.data().shape().len())?;
    for (&points, &lanes) in raw.data().shape().iter().zip(raw.data().component_lanes()) {
        raw_shape.push(
            points
                .checked_mul(lanes)
                .ok_or(ProcessingError::SizeOverflow)?,
        );
    }
    let output_shape = storage_shape(&descriptor)?;
    let output_len = checked_product(&output_shape)?;
    let mut output = try_vec_capacity(output_len)?;
    for output_index in 0..output_len {
        charge_block(control, output_index, output_len)?;
        let output_coordinate = unflatten(&output_shape, output_index)?;
        let mut raw_coordinate = Vec::new();
        raw_coordinate
            .try_reserve_exact(output_coordinate.len())
            .map_err(|_| ProcessingError::AllocationFailure)?;
        let mut imaginary = false;
        for (axis_index, &expanded) in output_coordinate.iter().enumerate() {
            let processed_axis = &descriptor.axes()[axis_index];
            let logical = expanded / processed_axis.component_count();
            let component = expanded % processed_axis.component_count();
            let raw_axis = &raw.descriptor().axes()[axis_index];
            if axis_index + 1 == output_coordinate.len() {
                match raw_axis.kind() {
                    RawAxisKind::Direct(DirectSamples::Real) => raw_coordinate.push(logical),
                    RawAxisKind::Direct(DirectSamples::Complex) => {
                        raw_coordinate.push(logical);
                        imaginary = component == 1;
                    }
                    _ => return Err(ProcessingError::Mapping("invalid direct-axis kind")),
                }
            } else {
                raw_coordinate.push(
                    logical
                        .checked_mul(raw_axis.component_lanes())
                        .and_then(|value| value.checked_add(component))
                        .ok_or(ProcessingError::SizeOverflow)?,
                );
            }
        }
        let sample = input[flatten(&raw_shape, &raw_coordinate)?];
        output.push(if imaginary { sample.im } else { sample.re });
    }
    control.complete_work()?;
    Ok((descriptor, output))
}

pub(in crate::processing) fn execute(
    control: &mut ExecutionContext<'_>,
    mut current: Vec<f64>,
    mut descriptor: ProcessedDescriptor,
    prepared: &PreparedRawPlan,
    _options: ProcessingOptions,
) -> Result<ProcessedData, ProcessingError> {
    for step in &prepared.steps {
        let step_index = step.step_index;
        debug_assert_eq!(descriptor, step.before);
        let target = step
            .axis
            .map_or(OperationTarget::Dataset, OperationTarget::Axis);
        let result = (|| {
            control.begin(
                if prepared.phase == ProcessingPhase::Replay {
                    ExecutionStage::Replay
                } else {
                    ExecutionStage::Processing
                },
                Some(step_index),
                Some(ProgressTotal::UpperBound(step_work(step)?)),
            )?;
            execute_step(control, &current, step)
        })();
        current = result.map_err(|error: ProcessingError| {
            error.located(Some(step_index), Some(target), prepared.phase)
        })?;
        control.complete_work().map_err(|error| {
            ProcessingError::from(error).located(
                Some(step.step_index),
                step.axis.map(OperationTarget::Axis),
                prepared.phase,
            )
        })?;
        descriptor = step.after.clone();
    }
    control.check_cancelled()?;
    ProcessedData::new(
        descriptor.logical_shape(),
        descriptor.component_counts(),
        current,
    )
    .map_err(Into::into)
}

pub(in crate::processing) fn execute_step(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
) -> Result<Vec<f64>, ProcessingError> {
    match &step.resolved {
        ResolvedOperation::Spectrum(operation) => {
            spectrum::execute(control, current, step, operation)
        }
        ResolvedOperation::EstimatedBaseline { values, .. } => {
            let mut output = clone_samples(current, control)?;
            let c = step.before.axes()[0].component_count();
            for (i, b) in values.iter().enumerate() {
                if i % 4096 == 0 {
                    control.check_cancelled()?;
                }
                output[i * c] -= b;
            }
            if output.iter().any(|v| !v.is_finite()) {
                return Err(ProcessingError::NumericalInvariantViolation);
            }
            Ok(output)
        }
        ResolvedOperation::Window(window) => {
            let axis = &step.before.axes()[step.axis()];
            map_retained(control, current, step, |point, value| {
                Ok(value * window_weight(window, axis, step.axis(), point)?)
            })
        }
        ResolvedOperation::ZeroFill { .. } => zero_fill(control, current, step),
        ResolvedOperation::FourierTransform(sign) => fft_axis(control, current, step, *sign),
        ResolvedOperation::FrequencyDomainPhaseRampV1 { delay, sign } => {
            complex_pointwise(control, current, step, |point, points, value| {
                let q = point as isize - (points / 2) as isize;
                value * unit_complex(-sign.sigma() * 2.0 * PI * q as f64 * delay / points as f64)
            })
        }
        ResolvedOperation::TimeDomainShiftFoldV1 {
            applied_delay,
            skip,
            fold,
            ..
        } => time_domain_shift_fold_v1(control, current, step, *applied_delay, *skip, *fold),
        ResolvedOperation::PhaseCorrection(correction)
        | ResolvedOperation::AutomaticPhase { correction, .. } => {
            complex_pointwise(control, current, step, |point, points, value| {
                let degrees = correction.p0_degrees
                    + correction.p1_degrees
                        * (point as f64 / points as f64 - correction.pivot_fraction);
                value * unit_complex(degrees.to_radians())
            })
        }
        ResolvedOperation::ComponentTransform {
            transform,
            observation_ordinals,
            grid_origin,
        } => apply_component_transform(
            control,
            current,
            step,
            transform,
            observation_ordinals.as_deref(),
            *grid_origin,
        ),
        ResolvedOperation::Projection { projection, .. } => {
            project_scalar(control, current, step, *projection)
        }
        ResolvedOperation::BaselineCorrection(BaselineProfile::PositivePeaksV1(profile)) => {
            baseline_axis(control, current, step, *profile)
        }
        ResolvedOperation::ResolveFrequencyFrame { .. }
        | ResolvedOperation::AcknowledgeZeroDelayV1 { .. } => clone_samples(current, control),
        ResolvedOperation::ReverseAxis => reverse_axis(control, current, step),
    }
}

pub(in crate::processing) fn derived_dataset(
    control: &mut ExecutionContext<'_>,
    dataset: &ProcessedDataset,
    initial_descriptor: ProcessedDescriptor,
    input: HistoryInput,
    records: Vec<ProcessingRecord>,
    descriptor: ProcessedDescriptor,
    samples: Vec<f64>,
) -> Result<ProcessedDataset, ProcessingError> {
    let data = ProcessedData::new(
        descriptor.logical_shape(),
        descriptor.component_counts(),
        samples,
    )?;
    let history = ProcessingHistory::new(initial_descriptor, input, records);
    let provenance = ProcessedProvenance::derived_from(dataset.provenance(), history);
    ProcessedDataset::new_derived_with_context(control, descriptor, data, provenance)
}

#[derive(Debug)]
pub(in crate::processing) struct HistoryState {
    pub(in crate::processing) initial_descriptor: ProcessedDescriptor,
    pub(in crate::processing) input: HistoryInput,
    pub(in crate::processing) records: Vec<ProcessingRecord>,
    pub(in crate::processing) state: PlanState,
}

pub(in crate::processing) fn history_state(
    dataset: &ProcessedDataset,
) -> Result<HistoryState, ProcessingError> {
    validate_processing_axes(dataset)?;
    #[cfg(test)]
    HISTORY_SEEDS.with(|count| count.set(count.get() + 1));
    if let Some(history) = dataset.provenance().history() {
        Ok(HistoryState {
            initial_descriptor: history.initial_descriptor().clone(),
            input: history.input().clone(),
            records: history.records().to_vec(),
            state: dataset.processing_state().clone(),
        })
    } else {
        Ok(HistoryState {
            initial_descriptor: dataset.descriptor().clone(),
            input: HistoryInput::Processed {
                digests: dataset.canonical_digests(),
                sources: dataset.provenance().sources().to_vec(),
                read_record: dataset.provenance().read_record().cloned(),
            },
            records: Vec::new(),
            state: dataset.processing_state().clone(),
        })
    }
}
