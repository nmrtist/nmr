//! Explicit direct processing of acquired rows followed by general-grid IST.
use crate::acquisition::{DirectSamples, RawAxisKind};
use crate::axis::AxisCoordinates;
use crate::execution::{ExecutionContext, ExecutionStage};
use crate::processed::{ComponentBasis, ProcessedData, ProcessedDataset, ProcessedDescriptor};
use crate::processing::contracts::{
    error::*, operation::*, options::*, prepared_step::*, state::*,
};
use crate::processing::engine::execution::*;
use crate::processing::kernels::buffer::*;
use crate::processing::kernels::ist::*;
use crate::processing::prepare::plan::*;
use crate::processing::prepare::resources::*;
use crate::raw::RawDataset;

use crate::axis::AxisDomain;

mod noise;
pub use noise::{AutoNusSettings, NusNoiseError, NusNoiseReport, NusNoiseSource};

/// General NUS reconstruction settings; noise evidence must be supplied explicitly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NusSettings {
    /// Maximum iterations, 1..=2048. Exhaustion is an error, never success.
    pub max_iterations: usize,
    /// Independent noise sigma in the processed direct-spectrum amplitude units.
    /// Some(0) is an explicit noiseless assertion; None is not interpreted as zero.
    pub noise_standard_deviation: Option<f64>,
}
impl Default for NusSettings {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            noise_standard_deviation: None,
        }
    }
}
impl NusSettings {
    pub(crate) fn validate(self) -> Result<(), ProcessingError> {
        if !(1..=2048).contains(&self.max_iterations)
            || !self
                .noise_standard_deviation
                .is_some_and(|v| v.is_finite() && v >= 0.0)
        {
            return Err(ProcessingError::InvalidParameter(
                "NUS requires valid iterations and independent noise sigma or explicit zero",
            ));
        }
        Ok(())
    }
    /// Preflight direct-axis operations and reconstruction together, before sample
    /// allocation. Only acquired rows enter F2 kernels. Indirect FFT remains a
    /// separate explicit plan on the returned dense Dataset.
    pub fn prepare(
        self,
        input: &crate::Dataset,
        direct_plan: ProcessingPlan,
        options: ProcessingOptions,
    ) -> Result<PreparedNus<'_>, ProcessingError> {
        self.prepare_with_context(
            input,
            direct_plan,
            options,
            &mut ExecutionContext::default(),
        )
    }

    /// Prepares NUS with host cancellation and preflight progress, without charging
    /// numerical work. The returned preparation borrows only the input. Index
    /// traversal and internal preflight share this context; individual allocations
    /// and the index sort are checked before and after, not interrupted internally.
    pub fn prepare_with_context<'a>(
        self,
        input: &'a crate::Dataset,
        direct_plan: ProcessingPlan,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<PreparedNus<'a>, ProcessingError> {
        self.prepare_internal(input, direct_plan, options, control, false)
    }

    fn prepare_internal<'a>(
        self,
        input: &'a crate::Dataset,
        direct_plan: ProcessingPlan,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
        automatic: bool,
    ) -> Result<PreparedNus<'a>, ProcessingError> {
        let result = (|| {
            control.begin(ExecutionStage::Preflight, None, None)?;
            if automatic {
                if !(1..=2048).contains(&self.max_iterations) {
                    return Err(ProcessingError::InvalidParameter("NUS iterations"));
                }
                noise::validate_plan(&direct_plan)?;
            } else {
                self.validate()?;
            }
            let raw = input.as_raw().ok_or(ProcessingError::InvalidParameter(
                "NUS reconstruction requires raw acquisition",
            ))?;
            let axes = raw.descriptor().axes();
            if axes.len() != 2
                || axes[0].domain() != AxisDomain::Time
                || !matches!(axes[0].coordinates(),AxisCoordinates::Uniform {step,..} if *step>0.0)
            {
                return Err(ProcessingError::InvalidParameter(
                    "NUS requires a calibrated rank-two indirect time grid",
                ));
            }
            for operation in &direct_plan.operations {
                control.check_cancelled()?;
                if !(operation.axis() == Some(1)
                    || matches!(
                        operation,
                        ProcessingOperation::ComponentTransform { axis: 0 }
                    ))
                    || matches!(operation,ProcessingOperation::Spectrum {operation,..} if operation.removes_axis())
                {
                    return Err(ProcessingError::InvalidParameter(
                        "before reconstruction only direct-axis operations and explicit component decoding are supported",
                    ));
                }
            }
            let n = axes[0].points();
            let m = raw.data().sparse_traces().map_or(n, |t| t.len());
            let metadata = checked_sum(&[
                crate::processing::prepare::memory::raw_apply_with_cancellation(
                    raw,
                    direct_plan.operations.len(),
                    Some(control.cancellation()),
                )?,
                crate::processing::prepare::memory::aggregate_with_cancellation(
                    input.metadata(),
                    Some(control.cancellation()),
                )?,
                checked_times(n, 64)?,
                checked_times(m, 32)?,
                8192,
            ])?;
            check_resource(
                crate::resource::ResourceKind::MetadataBytes,
                metadata,
                options.metadata_bytes(),
            )?;
            check_resource(
                crate::resource::ResourceKind::WorkingBytes,
                metadata,
                options.working_bytes(),
            )?;
            control.check_cancelled()?;
            let mut indices = try_vec_capacity(m)?;
            if let Some(traces) = raw.data().sparse_traces() {
                for chunk in traces.chunks(4096) {
                    control.check_cancelled()?;
                    indices.extend(chunk.iter().map(|trace| trace.coordinate().as_slice()[0]));
                    control.advance(chunk.len() as u128)?;
                }
            } else {
                for start in (0..n).step_by(4096) {
                    control.check_cancelled()?;
                    let end = start.saturating_add(4096).min(n);
                    indices.extend(start..end);
                    control.advance((end - start) as u128)?;
                }
            }
            control.check_cancelled()?;
            let mut sorted = indices.clone();
            control.check_cancelled()?;
            sorted.sort_unstable();
            control.check_cancelled()?;
            let mut duplicates = false;
            for (index, pair) in sorted.windows(2).enumerate() {
                if index % 4096 == 0 {
                    control.check_cancelled()?;
                }
                if pair[0] == pair[1] {
                    duplicates = true;
                    break;
                }
            }
            if m == 0 || sorted.last().is_some_and(|v| *v >= n) || duplicates {
                return Err(ProcessingError::InvalidParameter(
                    "reconstruction rejects duplicate or out-of-grid observations",
                ));
            }
            control.check_cancelled()?;
            let full = expand_raw_descriptor(axes)?;
            let mut state = PlanState::from_raw_with_cancellation(
                &full,
                axes,
                raw.sampling_schedule(),
                raw.data().layout().absolute_origin(),
                Some(control.cancellation()),
            )?;
            let direct_resolved = direct_plan
                .operations
                .iter()
                .enumerate()
                .map(|(step, operation)| {
                    control.check_cancelled()?;
                    transition(&mut state, operation)
                        .map_err(ProcessingError::from)
                        .map_err(|e| {
                            e.located(
                                Some(step),
                                Some(operation.target()),
                                ProcessingPhase::Preflight,
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let descriptor = state.descriptor()?;
            if !matches!(
                descriptor.axes()[0].component_basis(),
                ComponentBasis::Cartesian | ComponentBasis::SharedComplex { .. }
            ) || descriptor.axes()[1].domain() != AxisDomain::Frequency
            {
                return Err(ProcessingError::InvalidParameter(
                    "explicit F2 FFT and Cartesian or SharedComplex indirect components are required before IST",
                ));
            }
            let columns = descriptor.axes()[1].points();
            let fields = ist_fields(&descriptor);
            if automatic && !noise::sufficient_samples(m, columns) {
                return Err(ProcessingError::NoiseEstimation(
                    NusNoiseError::InsufficientSamples,
                ));
            }
            let metadata = checked_sum(&[
                metadata,
                checked_times(columns, 16)?,
                checked_times(
                    direct_resolved.len(),
                    std::mem::size_of::<ResolvedOperation>(),
                )?,
            ])?;
            check_resource(
                crate::resource::ResourceKind::MetadataBytes,
                metadata,
                options.metadata_bytes(),
            )?;
            let (ist_resources, ist_work) = crate::processing::kernels::ist::shape_resources(
                n,
                m,
                columns,
                fields,
                self.max_iterations,
            )
            .map_err(ProcessingError::Reconstruction)?;
            let row = row_state(raw, indices[0], control.cancellation())?;
            let preview = preflight(
                control,
                row,
                &direct_plan.operations,
                options,
                input.canonical_digests().dataset(),
            )?;
            let compact = checked_times(checked_times(m, columns)?, fields * 16)?;
            let working = checked_sum(&[
                metadata,
                checked_times(compact, 2)?,
                ist_resources.working_bytes(),
                preview.resources().working_bytes(),
                if automatic { 8192 } else { 0 },
                ist_resources.output_bytes(),
            ])?;
            let output = descriptor_bytes(&descriptor)?;
            check_resource(
                crate::resource::ResourceKind::OutputBytes,
                output,
                options.output_bytes(),
            )?;
            check_resource(
                crate::resource::ResourceKind::WorkingBytes,
                working,
                options.working_bytes(),
            )?;
            let work = ist_work
                .checked_add(
                    preview
                        .estimated_work()?
                        .checked_mul(m as u128)
                        .ok_or(ProcessingError::SizeOverflow)?,
                )
                .and_then(|v| v.checked_add((compact / 8 + output / 8) as u128))
                .ok_or(ProcessingError::SizeOverflow)?;
            let work = work
                .checked_add(if automatic {
                    noise::work(compact / 8)?
                } else {
                    0
                })
                .ok_or(ProcessingError::SizeOverflow)?;
            control.check_cancelled()?;
            Ok(PreparedNus {
                automatic,
                interior_noise: automatic && jeol_filtered(&axes[1]),
                input,
                settings: self,
                direct_plan,
                direct_resolved,
                options,
                indices,
                descriptor,
                state,
                preview,
                resources: crate::resource::ResourceEstimate::new(output, metadata, working),
                work,
            })
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                None,
                Some(OperationTarget::Dataset),
                ProcessingPhase::Preflight,
            )
        })
    }
}

/// Immutable complete NUS capacity preview and full-grid output description.
#[derive(Debug)]
pub struct PreparedNus<'a> {
    automatic: bool,
    interior_noise: bool,
    input: &'a crate::Dataset,
    settings: NusSettings,
    direct_plan: ProcessingPlan,
    direct_resolved: Vec<ResolvedOperation>,
    options: ProcessingOptions,
    indices: Vec<usize>,
    descriptor: ProcessedDescriptor,
    state: PlanState,
    preview: PreparedRawPlan,
    resources: crate::resource::ResourceEstimate,
    work: u128,
}
impl<'a> PreparedNus<'a> {
    // Archived derivations retain their selection policy even if the current
    // automatic default chooses a newer receiver-specific policy.
    pub(crate) fn recorded_noise_source(
        mut self,
        source: NusNoiseSource,
    ) -> Result<Self, ProcessingError> {
        if !self.automatic
            || source == NusNoiseSource::Explicit
            || source.holdout() != (self.indices.len() < 48)
            || (source.interior() && !self.interior_noise)
        {
            return Err(ProcessingError::InvalidParameter(
                "invalid recorded NUS noise policy",
            ));
        }
        self.interior_noise = source.interior();
        Ok(self)
    }
    /// Full-grid mixed-domain output descriptor; F1 remains time domain.
    pub fn output_descriptor(&self) -> &ProcessedDescriptor {
        &self.descriptor
    }
    /// Direct step preview on one acquired observation; local indices match the supplied plan.
    pub fn direct_steps(&self) -> impl ExactSizeIterator<Item = PreparedStepView<'_>> {
        self.preview.steps()
    }
    /// Original observation order, without sorting, deduplication or missing-value filling.
    pub fn measured_indices(&self) -> &[usize] {
        &self.indices
    }
    /// Full output, metadata and peak working payload.
    pub fn resources(&self) -> crate::resource::ResourceEstimate {
        self.resources
    }
    /// Complete direct-stage and reconstruction work reservation.
    pub fn estimated_work(&self) -> u128 {
        self.work
    }
    /// Execute direct processing and reconstruction; returns no partial dataset on failure.
    pub fn execute(self) -> Result<crate::Dataset, ProcessingError> {
        self.execute_with_context(&mut ExecutionContext::default())
    }
    /// Execute all stages using the caller's shared context, then permit a separate F1 plan.
    pub fn execute_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.analyze_with_context(control)?
            .execute_with_context(control)
    }
    /// Process acquired F2 rows once and resolve noise evidence. The returned
    /// object owns the cached observations and can reconstruct without another F2 pass.
    pub fn analyze(self) -> Result<AnalyzedNus<'a>, ProcessingError> {
        self.analyze_with_context(&mut ExecutionContext::default())
    }
    /// Analyze with shared cancellation, progress and complete work reservation.
    pub fn analyze_with_context(
        mut self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<AnalyzedNus<'a>, ProcessingError> {
        control.ensure_work(self.work)?;
        control.observe_payload(self.resources.working_bytes());
        let raw = self.input.as_raw().unwrap();
        let direct = raw.descriptor().axes().last().unwrap();
        let lanes = raw.descriptor().axes()[0].component_lanes();
        let dc = match direct.kind() {
            RawAxisKind::Direct(DirectSamples::Complex) => 2,
            _ => 1,
        };
        let columns = self.descriptor.axes()[1].points();
        let fields = ist_fields(&self.descriptor);
        let mut compact = try_vec_capacity(self.indices.len() * columns * fields * 2)?;
        for (ordinal, &coordinate) in self.indices.iter().enumerate() {
            control.check_cancelled()?;
            let row = row_state(raw, coordinate, control.cancellation())?;
            let baseline = row.descriptor()?;
            let prepared = preflight(
                control,
                row,
                &self.direct_plan.operations,
                self.options,
                self.input.canonical_digests().dataset(),
            )?;
            let samples = if let Some(traces) = raw.data().sparse_traces() {
                traces[ordinal].samples()
            } else {
                let count = lanes * direct.points();
                &raw.data().dense_samples().unwrap()[ordinal * count..(ordinal + 1) * count]
            };
            let mut expanded = try_vec_capacity(samples.len() * dc)?;
            for sample in samples {
                expanded.push(sample.re);
                if dc == 2 {
                    expanded.push(sample.im);
                }
            }
            let result = execute(control, expanded, baseline, &prepared, self.options)?;
            // Transpose [F1 component,F2,F2 component] into IST [F2,field,F1 complex].
            // These must be Cartesian C/S lanes, not echo/antiecho channels.
            // JEOL pn_type=y is normalized by the reader; encoded acquisitions
            // require the explicit ComponentTransform checked in preflight.
            if is_shared(&self.descriptor) {
                // One complex channel belongs to F2 and is also the F1 channel.
                // No fabricated quadrature lane or component conversion. The
                // conjugated F1 convention reflects the shrinkage spectrum and
                // therefore commutes with this symmetric complex IST operator.
                compact.extend_from_slice(result.samples());
            } else {
                for p in 0..columns {
                    for field in 0..fields {
                        compact.push(result.samples()[p * fields + field]);
                        compact.push(result.samples()[(columns + p) * fields + field]);
                    }
                }
            }
        }
        control.charge(compact.len() as u128)?;
        let input = IstInput::with_grid(
            self.descriptor.axes()[0].points(),
            columns,
            fields,
            std::mem::take(&mut self.indices),
            compact,
        )
        .map_err(ProcessingError::Reconstruction)?;
        let noise_report = if self.automatic {
            noise::estimate(&input, control, self.interior_noise)?
        } else {
            NusNoiseReport::explicit(self.settings.noise_standard_deviation.unwrap())
        };
        Ok(AnalyzedNus {
            prepared: self,
            input,
            noise_report,
        })
    }

    fn finish(
        self,
        input: IstInput,
        noise_report: NusNoiseReport,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        let columns = self.descriptor.axes()[1].points();
        let fields = ist_fields(&self.descriptor);
        let options = IstOptions::new()
            .max_iterations(self.settings.max_iterations)
            .map_err(ProcessingError::Reconstruction)?
            .noise_standard_deviation(noise_report.sigma)
            .map_err(ProcessingError::Reconstruction)?
            .max_working_bytes(self.options.working_bytes())
            .max_output_bytes(self.options.output_bytes())
            .max_reconstruction_work(self.work);
        let reconstructed = GeneralGridPhaseCovariantGroupRetainedIstV1
            .reconstruct_with_context(&input, options, control)
            .map_err(|e| {
                ProcessingError::Reconstruction(e).located(
                    Some(self.direct_plan.operations.len()),
                    Some(OperationTarget::Axis(0)),
                    ProcessingPhase::Execution,
                )
            })?;
        let n = self.descriptor.axes()[0].points();
        let mut samples = try_zeroed(n * columns * fields * 2)?;
        control.charge(samples.len() as u128)?;
        for row in 0..n {
            control.check_cancelled()?;
            for p in 0..columns {
                for field in 0..fields {
                    for lane in 0..2 {
                        let target = if is_shared(&self.descriptor) {
                            (row * columns + p) * 2 + lane
                        } else {
                            ((row * 2 + lane) * columns + p) * fields + field
                        };
                        samples[target] = reconstructed.components()
                            [((row * columns + p) * fields + field) * 2 + lane];
                    }
                }
            }
        }
        let data = ProcessedData::new(
            self.descriptor.logical_shape(),
            self.descriptor.component_counts(),
            samples,
        )?;
        let digests =
            crate::canonical_digest::processed_digests(&self.descriptor, &data, &self.state);
        let operation = crate::derivation::DerivationOperation::NusReconstruction {
            direct_operations: self.direct_plan.operations,
            direct_resolved: self.direct_resolved,
            settings: self.settings,
            noise_report: Box::new(noise_report),
            iterations: reconstructed.iterations(),
            column_thresholds: (0..columns)
                .map(|i| reconstructed.column_final_threshold(i).unwrap())
                .collect(),
            column_relative_changes: (0..columns)
                .map(|i| reconstructed.column_relative_change(i).unwrap())
                .collect(),
        };
        let provenance = crate::derivation::provenance(
            vec![crate::derivation::DerivationInput::capture(self.input)],
            operation,
            self.descriptor.clone(),
            self.state,
            digests,
            self.input.sources().to_vec(),
        );
        Ok(self.input.derived_processed(ProcessedDataset::new_library(
            control,
            self.descriptor,
            data,
            provenance,
        )?))
    }
}

/// Acquired direct-frequency observations and resolved noise evidence, cached
/// under the prepared resource limits. Dropping this value returns no dataset.
#[derive(Debug)]
pub struct AnalyzedNus<'a> {
    prepared: PreparedNus<'a>,
    input: IstInput,
    noise_report: NusNoiseReport,
}
impl AnalyzedNus<'_> {
    /// Actual sigma, source and quality diagnostics, before reconstruction.
    pub fn noise_report(&self) -> &NusNoiseReport {
        &self.noise_report
    }
    /// Reconstruct using the cached direct observations.
    pub fn execute(self) -> Result<crate::Dataset, ProcessingError> {
        self.execute_with_context(&mut ExecutionContext::default())
    }
    /// Reconstruct with the shared work ledger and cancellation token.
    pub fn execute_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.prepared.finish(self.input, self.noise_report, control)
    }
}

fn row_state(
    raw: &RawDataset,
    coordinate: usize,
    token: &crate::CancellationToken,
) -> Result<PlanState, ProcessingError> {
    let axes = raw.descriptor().axes();
    let full = expand_raw_descriptor(axes)?;
    let mut row_axes = full.axes().to_vec();
    let a = &row_axes[0];
    let at = axes[0]
        .coordinate(coordinate)
        .map_err(|_| ProcessingError::MissingTimeCalibration)?;
    row_axes[0] = a.rebuilt(
        a.domain(),
        a.unit(),
        1,
        AxisCoordinates::Explicit(vec![at]),
        a.component_basis().clone(),
        a.spectral_width_hz(),
    )?;
    let row = ProcessedDescriptor::new(row_axes)?;
    let mut origin = raw.data().layout().absolute_origin().to_vec();
    origin[0] = coordinate;
    PlanState::from_raw_with_cancellation(&row, axes, raw.sampling_schedule(), &origin, Some(token))
        .map_err(Into::into)
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl NusSettings {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&usize, &Option<f64>) {
        (&self.max_iterations, &self.noise_standard_deviation)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (usize, Option<f64>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (max_iterations, noise_standard_deviation) = parts;
        let value = Self {
            max_iterations,
            noise_standard_deviation,
        };

        Ok(value)
    }
}

pub(crate) fn validate_automatic_plan(
    operations: &[ProcessingOperation],
) -> Result<(), ProcessingError> {
    noise::validate_operations(operations)
}

fn is_shared(descriptor: &ProcessedDescriptor) -> bool {
    matches!(
        descriptor.axes()[0].component_basis(),
        ComponentBasis::SharedComplex { .. }
    )
}

fn ist_fields(descriptor: &ProcessedDescriptor) -> usize {
    if is_shared(descriptor) {
        1
    } else {
        descriptor.axes()[1].component_count()
    }
}

pub(crate) fn jeol_filtered(axis: &crate::raw::RawAxis) -> bool {
    matches!(axis.group_delay(), crate::acquisition::GroupDelayState::Pending(delay)
        if matches!(delay.evidence().authority(), crate::acquisition::ResolutionAuthority::FormatRule(rule)
            if *rule == crate::acquisition::registry::JEOL_DIRECT_V1))
}
