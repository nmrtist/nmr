//! Named automatic methods with bounded full-resolution objectives.
use crate::Complex64;
use crate::execution::{ExecutionContext, ExecutionStage, ProgressTotal};
use crate::processing::contracts::{
    error::*, history::*, operation::*, options::*, prepared_step::*, state::*,
};
use crate::processing::engine::execution::*;
use crate::processing::kernels::acme::PhaseOptimizationError;
use crate::processing::kernels::buffer::*;
use crate::processing::methods::auto_phase::*;
use crate::processing::prepare::resources::*;
use crate::provenance::CanonicalDigest;
use std::f64::consts::PI;

const ZERO_ORDER_GRID_POINTS: usize = 48;
const FIRST_ORDER_GRID_INTERVALS: usize = 48;
const MAX_FIRST_ORDER: f64 = 4.0 * PI;
const REFINEMENT_ITERATIONS: usize = 100;
// Two full grids and coordinate refinements, followed by at most four
// consensus seeds and a two-direction refinement. The same bound drives
// preflight and execution; keep the full-resolution spacing at 7.5 / 15 degrees.
const MAX_EVALUATIONS: usize = 2
    * (ZERO_ORDER_GRID_POINTS * (FIRST_ORDER_GRID_INTERVALS + 1) + 4 * REFINEMENT_ITERATIONS)
    + 4
    + 2 * REFINEMENT_ITERATIONS;

/// Immutable analysis preparation. Select a representative 1D slice explicitly.
#[derive(Debug)]
pub struct PreparedPhaseEstimate<'a> {
    input: &'a crate::Dataset,
    method: PhaseMethod,
    axis: usize,
    budget: AutoPhaseBudget,
}

/// Actual automatic phase parameters and diagnostics, bound to the analyzed input.
#[derive(Clone, Debug)]
pub struct PhaseEstimate {
    method: PhaseMethod,
    axis: usize,
    correction: PhaseCorrection,
    objective: f64,
    evaluations: usize,
    input_digest: CanonicalDigest,
}

impl PhaseMethod {
    /// Validate and reserve analysis/correction on a checked Cartesian 1D spectrum.
    /// Hz and ppm have identical index-phase semantics. Select a series trace with
    /// Slice first, then apply the returned correction to the series explicitly.
    /// A shared-complex 2D `Slice { component: 0, .. }` retains the full pair in
    /// the surviving axis's orientation. Submit `estimate.correction()` on that
    /// original 2D axis without an additional sign change.
    pub fn prepare(
        self,
        input: &crate::Dataset,
        axis: usize,
        options: ProcessingOptions,
    ) -> Result<PreparedPhaseEstimate<'_>, ProcessingError> {
        let result = (|| {
            let dataset = input
                .as_processed()
                .ok_or(ProcessingError::MissingCapability {
                    capability: "processed phase input",
                    axis: Some(axis),
                })?;
            if dataset.descriptor().axes().len() != 1 {
                return Err(ProcessingError::InvalidParameter(
                    "select a representative 1D trace before phase estimation",
                ));
            }
            let options = reserve_context(input.metadata(), options)?;
            let mut budget = auto_phase_budget(dataset, axis, options)?;
            budget.work = budget
                .work
                .checked_add(
                    (dataset.descriptor().axes()[axis].points() as u128)
                        * (MAX_EVALUATIONS as u128 * 4 + 16),
                )
                .ok_or(ProcessingError::SizeOverflow)?;
            Ok(PreparedPhaseEstimate {
                input,
                method: self,
                axis,
                budget,
            })
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Axis(axis)),
                ProcessingPhase::Preflight,
            )
        })
    }
}

impl PreparedPhaseEstimate<'_> {
    /// Conservative complete analysis and application payload capacity.
    pub fn resources(&self) -> crate::resource::ResourceEstimate {
        self.budget.resources
    }
    /// Maximum work, including full-resolution search without stride decimation.
    pub fn estimated_work(&self) -> u128 {
        self.budget.work
    }
    /// Estimate parameters without changing the dataset.
    pub fn estimate(self) -> Result<PhaseEstimate, ProcessingError> {
        self.estimate_with_context(&mut ExecutionContext::default())
    }
    /// Analyze using shared cancellation and work. The local operation position is zero.
    pub fn estimate_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<PhaseEstimate, ProcessingError> {
        let result = (|| {
            control.ensure_work(self.budget.work)?;
            control.observe_payload(self.budget.resources.working_bytes());
            control.begin(
                ExecutionStage::AutoPhase,
                Some(0),
                Some(ProgressTotal::UpperBound(self.budget.work)),
            )?;
            let dataset = self
                .input
                .as_processed()
                .expect("preflight checked processed");
            let samples = dataset.data().samples();
            control.charge(samples.len() as u128 * 4)?;
            let mut normalized = try_vec_capacity(samples.len() / 2)?;
            let scale = samples
                .chunks_exact(2)
                .map(|v| v[0].hypot(v[1]))
                .fold(0.0_f64, f64::max);
            if scale == 0.0 {
                return Err(ProcessingError::PhaseOptimization(
                    PhaseOptimizationError::NoUsableSignal,
                ));
            }
            for (i, v) in samples.chunks_exact(2).enumerate() {
                if i % 4096 == 0 {
                    control.check_cancelled()?;
                }
                normalized.push(Complex64::new(v[0] / scale, v[1] / scale));
            }
            let (p0, p1, objective, evaluations) = estimate(&normalized, self.method, control)?;
            let n = normalized.len();
            // Estimator uses exp(-i*(p0+p1*i/(N-1))); convert once to the public convention.
            let correction = PhaseCorrection::new(
                -p0.to_degrees(),
                if n == 1 {
                    0.0
                } else {
                    -p1.to_degrees() * n as f64 / (n - 1) as f64
                },
                0.0,
            )?;
            control.complete_work()?;
            Ok(PhaseEstimate {
                method: self.method,
                axis: self.axis,
                correction,
                objective,
                evaluations,
                input_digest: self.input.canonical_digests().dataset(),
            })
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Axis(self.axis)),
                ProcessingPhase::Execution,
            )
        })
    }
}

impl PhaseEstimate {
    /// Actual correction; may also be submitted explicitly to another compatible spectrum.
    pub fn correction(&self) -> PhaseCorrection {
        self.correction
    }
    /// Method that produced the estimate.
    pub fn method(&self) -> PhaseMethod {
        self.method
    }
    /// Axis analyzed in the bound input.
    pub fn axis(&self) -> usize {
        self.axis
    }
    /// Actual dimensionless final objective (zero for direct peak methods).
    pub fn objective(&self) -> f64 {
        self.objective
    }
    /// Actual number of evaluated candidates.
    pub fn evaluations(&self) -> usize {
        self.evaluations
    }
    /// Apply to the same input and record the estimator, correction and diagnostics.
    pub fn apply(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.apply_with_context(input, options, &mut ExecutionContext::default())
    }
    /// Apply using the same shared execution context as prior and subsequent segments.
    pub fn apply_with_context(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        let result = (|| {
            if input.canonical_digests().dataset() != self.input_digest {
                return Err(ProcessingError::InputIdentityMismatch);
            }
            let prepared = self.method.prepare(input, self.axis, options)?;
            let options = prepared.budget.options;
            let dataset = input.as_processed().expect("prepare validated input");
            control.ensure_work(dataset.data().samples().len() as u128)?;
            control.observe_payload(prepared.resources().working_bytes());
            control.begin(ExecutionStage::Processing, Some(0), None)?;
            let HistoryState {
                initial_descriptor: base,
                input: binding,
                mut records,
                mut state,
            } = history_state(dataset)?;
            let before = state.descriptor()?;
            transition_auto_phase(&mut state, self.axis)?;
            let after = state.descriptor()?;
            let resolved = ResolvedOperation::AutomaticPhase {
                correction: self.correction,
                objective: self.objective,
                evaluations: self.evaluations,
            };
            check_working_limit(&before, &after, Some(self.axis), &resolved, options)?;
            let step = PreparedStep {
                step_index: 0,
                axis: Some(self.axis),
                before,
                after: after.clone(),
                resolved: resolved.clone(),
            };
            let samples = execute_step(control, dataset.data().samples(), &step)?;
            records.push(ProcessingRecord::applied(
                ProcessingRequest::PhaseMethod {
                    axis: self.axis,
                    method: self.method,
                },
                resolved,
                step.before,
                step.after,
                Vec::new(),
            ));
            Ok(input.derived_processed(derived_dataset(
                control, dataset, base, binding, records, after, samples,
            )?))
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Axis(self.axis)),
                ProcessingPhase::Execution,
            )
        })
    }
}

struct Objective<'a, 'c, 'w> {
    values: &'a [Complex64],
    control: &'c mut ExecutionContext<'w>,
    evaluations: usize,
}
impl Objective<'_, '_, '_> {
    fn cost(&mut self, p0: f64, p1: f64, kind: usize) -> Result<f64, ProcessingError> {
        if self.evaluations >= MAX_EVALUATIONS {
            return Err(ProcessingError::PhaseOptimization(
                PhaseOptimizationError::OptimizationDidNotConverge,
            ));
        }
        self.evaluations += 1;
        self.control.charge(self.values.len() as u128 * 4)?;
        let n = self.values.len();
        let mut previous = 0.0;
        let mut derivative_sum = 0.0;
        let mut derivative_log = 0.0;
        let mut negative = 0.0;
        let mut total = 0.0;
        for (i, z) in self.values.iter().enumerate() {
            if i % 4096 == 0 {
                self.control.check_cancelled()?;
            }
            let (sin, cos) = (p0 + p1 * i as f64 / (n - 1).max(1) as f64).sin_cos();
            let real = z.re * cos + z.im * sin;
            total += real * real;
            if real < 0.0 {
                negative += real * real;
            }
            if i > 0 {
                let d = (real - previous).abs();
                derivative_sum += d;
                if d > 0.0 {
                    derivative_log += d * d.ln();
                }
            }
            previous = real;
        }
        if total == 0.0 {
            return Ok(f64::INFINITY);
        }
        if kind == 1 {
            return Ok(negative / total);
        }
        if derivative_sum == 0.0 {
            return Ok(f64::INFINITY);
        }
        let entropy = derivative_sum.ln() - derivative_log / derivative_sum;
        Ok(if kind == 0 {
            entropy + 1000.0 * negative
        } else {
            entropy / ((n - 1).max(2) as f64).ln() + 4.0 * negative / total
        })
    }
    fn optimized(&mut self, kind: usize) -> Result<(f64, f64, f64), ProcessingError> {
        let mut best = (0.0, 0.0, f64::INFINITY);
        for i in 0..ZERO_ORDER_GRID_POINTS {
            for j in 0..=FIRST_ORDER_GRID_INTERVALS {
                let p0 = -PI + 2.0 * PI * i as f64 / ZERO_ORDER_GRID_POINTS as f64;
                // Endpoint first-order phase is not periodic modulo 2*pi:
                // intermediate samples distinguish ramps separated by a turn.
                let p1 = -MAX_FIRST_ORDER
                    + 2.0 * MAX_FIRST_ORDER * j as f64 / FIRST_ORDER_GRID_INTERVALS as f64;
                let cost = self.cost(p0, p1, kind)?;
                if cost < best.2 {
                    best = (p0, p1, cost);
                }
            }
        }
        let mut step = PI / 18.0;
        for _ in 0..REFINEMENT_ITERATIONS {
            let mut improved = false;
            for (d0, d1) in [(step, 0.0), (-step, 0.0), (0.0, step), (0.0, -step)] {
                let p1 = (best.1 + d1).clamp(-MAX_FIRST_ORDER, MAX_FIRST_ORDER);
                let cost = self.cost(best.0 + d0, p1, kind)?;
                if cost < best.2 {
                    best = (best.0 + d0, p1, cost);
                    improved = true;
                }
            }
            if !improved {
                step *= 0.5;
                if step < 1e-5 {
                    break;
                }
            }
        }
        if !best.2.is_finite() {
            return Err(ProcessingError::PhaseOptimization(
                PhaseOptimizationError::ObjectiveUndefined,
            ));
        }
        Ok(best)
    }
}

fn peak_regression(values: &[Complex64]) -> Result<(f64, f64), ProcessingError> {
    let n = values.len();
    // Local maxima with a scale-relative height and prominence; no public peak analysis API.
    let mut peaks = Vec::new();
    for i in 1..n - 1 {
        let height = values[i].norm();
        if height > 0.05 && height > values[i - 1].norm() && height >= values[i + 1].norm() {
            peaks.push(i);
        }
    }
    if peaks.len() < 2 {
        return Err(ProcessingError::InvalidParameter(
            "phase regression requires at least two resolved peaks",
        ));
    }
    let mut previous = values[peaks[0]].arg();
    let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in peaks {
        let raw = values[i].arg();
        let phase = previous + (raw - previous + PI).rem_euclid(2.0 * PI) - PI;
        previous = phase;
        let x = i as f64 / (n - 1) as f64;
        let w = values[i].norm();
        sw += w;
        sx += w * x;
        sy += w * phase;
        sxx += w * x * x;
        sxy += w * x * phase;
    }
    let denominator = sw * sxx - sx * sx;
    if denominator <= 1e-15 {
        return Err(ProcessingError::InvalidParameter(
            "unidentifiable first-order phase",
        ));
    }
    let slope = (sw * sxy - sx * sy) / denominator;
    Ok(((sy - slope * sx) / sw, slope))
}

fn estimate(
    values: &[Complex64],
    method: PhaseMethod,
    control: &mut ExecutionContext<'_>,
) -> Result<(f64, f64, f64, usize), ProcessingError> {
    let peak = values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.norm().total_cmp(&b.1.norm()))
        .expect("nonempty checked dataset")
        .0;
    let angle = values[peak].arg();
    if method == PhaseMethod::AbsorptivePeak {
        return Ok((angle, 0.0, 0.0, 1));
    }
    if values.len() < 4 || values.windows(2).all(|p| p[0] == p[1]) {
        return Err(ProcessingError::InvalidParameter(
            "phase ramp is unidentifiable on a short or constant trace",
        ));
    }
    if method == PhaseMethod::PeakRegression {
        let (p0, p1) = peak_regression(values)?;
        return Ok((p0, p1, 0.0, 1));
    }
    let mut objective = Objective {
        values,
        control,
        evaluations: 0,
    };
    let result = match method {
        PhaseMethod::Entropy => objective.optimized(0)?,
        PhaseMethod::NegativeMinimization => objective.optimized(1)?,
        PhaseMethod::RobustConsensus => {
            let a = objective.optimized(0)?;
            let b = objective.optimized(1)?;
            let mut candidates = vec![0.0, a.1, b.1];
            if let Ok((_, slope)) = peak_regression(values) {
                candidates.push(slope);
            }
            let fraction = peak as f64 / (values.len() - 1) as f64;
            let mut best = (0.0, f64::INFINITY);
            for slope in candidates {
                let slope = slope.clamp(-MAX_FIRST_ORDER, MAX_FIRST_ORDER);
                let cost = objective.cost(angle - slope * fraction, slope, 2)?;
                if cost < best.1 {
                    best = (slope, cost);
                }
            }
            let mut step = PI / 18.0;
            for _ in 0..REFINEMENT_ITERATIONS {
                let mut improved = false;
                for delta in [step, -step] {
                    let slope = (best.0 + delta).clamp(-MAX_FIRST_ORDER, MAX_FIRST_ORDER);
                    let cost = objective.cost(angle - slope * fraction, slope, 2)?;
                    if cost < best.1 {
                        best = (slope, cost);
                        improved = true;
                    }
                }
                if !improved {
                    step *= 0.5;
                    if step < 1e-5 {
                        break;
                    }
                }
            }
            (angle - best.0 * fraction, best.0, best.1)
        }
        _ => unreachable!(),
    };
    Ok((result.0, result.1, result.2, objective.evaluations))
}
