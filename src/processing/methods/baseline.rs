//! Separate, checked baseline analysis and recorded subtraction.
use crate::execution::{ExecutionContext, ExecutionStage, ProgressTotal};
use crate::processing::contracts::{
    error::*, history::*, operation::*, options::*, prepared_step::*, state::*,
};
use crate::processing::engine::execution::*;
use crate::processing::kernels::spectrum;
use crate::processing::prepare::plan::*;
use crate::processing::prepare::resources::*;
use crate::provenance::CanonicalDigest;

/// Baseline preflight for a checked one-dimensional spectrum.
#[derive(Debug)]
pub struct PreparedBaselineEstimate<'a> {
    input: &'a crate::Dataset,
    method: RealBaseline,
    resources: crate::resource::ResourceEstimate,
    work: u128,
}
/// Actual fitted values and parameters, bound to the analyzed scientific input.
#[derive(Clone, Debug)]
pub struct BaselineEstimate {
    method: RealBaseline,
    values: Vec<f64>,
    coefficients: Vec<f64>,
    input: CanonicalDigest,
}
impl RealBaseline {
    /// Reserve analysis and application on a scalar/Cartesian frequency spectrum.
    pub fn prepare(
        self,
        input: &crate::Dataset,
        options: ProcessingOptions,
    ) -> Result<PreparedBaselineEstimate<'_>, ProcessingError> {
        let result = (|| {
            let p = input
                .as_processed()
                .ok_or(ProcessingError::InvalidParameter(
                    "baseline requires processed spectrum",
                ))?;
            if p.descriptor().axes().len() != 1 {
                return Err(ProcessingError::InvalidParameter(
                    "select a one-dimensional baseline input",
                ));
            }
            let extra =
                checked_sum(&[checked_times(p.descriptor().axes()[0].points(), 64)?, 4096])?;
            let prepared = ProcessingPlan::new(vec![ProcessingOperation::Spectrum {
                axis: 0,
                operation: SpectrumOperation::Baseline(self),
            }])?
            .preflight(input, options)?;
            let r = prepared.resources();
            check_resource(
                crate::resource::ResourceKind::MetadataBytes,
                checked_sum(&[r.metadata_bytes(), extra])?,
                options.metadata_bytes(),
            )?;
            check_resource(
                crate::resource::ResourceKind::WorkingBytes,
                checked_sum(&[r.working_bytes(), extra])?,
                options.working_bytes(),
            )?;
            Ok(PreparedBaselineEstimate {
                input,
                method: self,
                resources: crate::resource::ResourceEstimate::new(
                    r.output_bytes(),
                    checked_sum(&[r.metadata_bytes(), extra])?,
                    checked_sum(&[r.working_bytes(), extra])?,
                ),
                work: prepared.resolved.estimated_work()?,
            })
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Axis(0)),
                ProcessingPhase::Preflight,
            )
        })
    }
}
impl PreparedBaselineEstimate<'_> {
    /// Conservative output, history and working payload including fitted values.
    pub fn resources(&self) -> crate::resource::ResourceEstimate {
        self.resources
    }
    /// Analysis work upper bound.
    pub fn estimated_work(&self) -> u128 {
        self.work
    }
    /// Analyze without changing the input.
    pub fn estimate(self) -> Result<BaselineEstimate, ProcessingError> {
        self.estimate_with_context(&mut ExecutionContext::default())
    }
    /// Analyze using the same caller context as surrounding plan segments.
    pub fn estimate_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<BaselineEstimate, ProcessingError> {
        let result = (|| {
            control.ensure_work(self.work)?;
            control.observe_payload(self.resources.working_bytes());
            control.begin(
                ExecutionStage::Processing,
                Some(0),
                Some(ProgressTotal::UpperBound(self.work)),
            )?;
            control.charge(self.work)?;
            let p = self.input.as_processed().unwrap();
            let c = p.descriptor().axes()[0].component_count();
            let y = p
                .data()
                .samples()
                .chunks_exact(c)
                .map(|v| v[0])
                .collect::<Vec<_>>();
            let (values, coefficients) = spectrum::fit_baseline(&y, self.method, control)?;
            if values.iter().chain(&coefficients).any(|v| !v.is_finite()) {
                return Err(ProcessingError::NumericalInvariantViolation);
            }
            Ok(BaselineEstimate {
                method: self.method,
                values,
                coefficients,
                input: self.input.canonical_digests().dataset(),
            })
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Axis(0)),
                ProcessingPhase::Execution,
            )
        })
    }
}
impl BaselineEstimate {
    /// Original method, including all explicit AsLS settings.
    pub fn method(&self) -> RealBaseline {
        self.method
    }
    /// Fitted baseline at each input coordinate in input intensity units.
    pub fn values(&self) -> &[f64] {
        &self.values
    }
    /// Ascending monomial coefficients in t=2*i/(N-1)-1; empty for AsLS.
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }
    /// Subtract the fit from the same input's real component and record actual values.
    pub fn apply(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.apply_with_context(input, options, &mut ExecutionContext::default())
    }
    /// Apply with shared work and cancellation; imaginary data is retained exactly.
    pub fn apply_with_context(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        let result = (|| {
            if input.canonical_digests().dataset() != self.input {
                return Err(ProcessingError::InputIdentityMismatch);
            }
            let prepared = self.method.prepare(input, options)?;
            control.observe_payload(prepared.resources.working_bytes());
            let p = input.as_processed().unwrap();
            control.ensure_work(p.data().samples().len() as u128)?;
            control.begin(ExecutionStage::Processing, Some(0), None)?;
            let HistoryState {
                initial_descriptor: base,
                input: binding,
                mut records,
                mut state,
            } = history_state(p)?;
            let before = state.descriptor()?;
            transition(
                &mut state,
                &ProcessingOperation::Spectrum {
                    axis: 0,
                    operation: SpectrumOperation::Baseline(self.method),
                },
            )?;
            let after = state.descriptor()?;
            let resolved = ResolvedOperation::EstimatedBaseline {
                values: self.values.clone(),
                coefficients: self.coefficients.clone(),
            };
            let step = PreparedStep {
                step_index: 0,
                axis: Some(0),
                before,
                after: after.clone(),
                resolved: resolved.clone(),
            };
            let samples = execute_step(control, p.data().samples(), &step)?;
            records.push(ProcessingRecord::applied(
                ProcessingRequest::BaselineEstimate {
                    axis: 0,
                    method: self.method,
                },
                resolved,
                step.before,
                step.after,
                Vec::new(),
            ));
            Ok(input.derived_processed(derived_dataset(
                control, p, base, binding, records, after, samples,
            )?))
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Axis(0)),
                ProcessingPhase::Execution,
            )
        })
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl BaselineEstimate {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&RealBaseline, &Vec<f64>, &Vec<f64>, &CanonicalDigest) {
        (&self.method, &self.values, &self.coefficients, &self.input)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (RealBaseline, Vec<f64>, Vec<f64>, CanonicalDigest),
    ) -> Result<Self, crate::internal::ModelError> {
        let (method, values, coefficients, input) = parts;
        let value = Self {
            method,
            values,
            coefficients,
            input,
        };

        Ok(value)
    }
}
