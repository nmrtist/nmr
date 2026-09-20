//! Bounded normalized ACME-inspired phase optimization.

use crate::Complex64;
use crate::execution::{ExecutionContext, ExecutionError};
use crate::internal::numeric::ScaledNeumaier;
use crate::processing::contracts::operation::PhaseCorrection;
use crate::processing::contracts::polarity::PolarityState;
use crate::processing::contracts::profile::NormalizedAcmeV1;
use crate::resource::{ResourceError, WorkLedger};
use std::cmp::Ordering;
use std::f64::consts::PI;
use thiserror::Error;

const COARSE_STEP_DEGREES: f64 = 15.0;
const SIMPLEX_STEP_DEGREES: f64 = 2.0;
const SIMPLEX_EVALUATIONS: usize = 512;
const PARAMETER_TOLERANCE_DEGREES: f64 = 1e-6;
const OBJECTIVE_RELATIVE_TOLERANCE: f64 = 1e-10;
const LINE_SHAPE_TOLERANCE: f64 = 1e-6;

impl NormalizedAcmeV1 {
    /// Total objective-evaluation ceiling: 600 coarse plus four times 512.
    pub const MAX_EVALUATIONS: usize = 2648;

    /// Worst-case trace passes charged for objectives and the quality guard.
    pub const MAX_WORK_PASSES: usize = Self::MAX_EVALUATIONS + 5;

    /// Creates the immutable V1 profile.
    pub fn new() -> Self {
        Self
    }

    /// Optimizes one finite complex trace under an existing polarity state.
    ///
    /// Only a resolved single positive Lorentzian with an established polarity
    /// and a verified complex absorption/dispersion line shape is accepted.
    /// Half-width must span at least two samples, at most 5% of the trace, and
    /// both edges must be at least four half-widths from the peak center.
    /// An already phased line returns the identity correction. An entropy
    /// minimum without this quality evidence returns `QualityUnverified`.
    pub fn optimize(
        self,
        trace: &[Complex64],
        polarity: PolarityState,
        work: &mut WorkLedger,
    ) -> Result<PhaseSolution, PhaseOptimizationError> {
        self.optimize_with_context(trace, polarity, &mut ExecutionContext::new(work))
    }
    /// Optimizes under cooperative cancellation and a shared numerical budget.
    pub fn optimize_with_context(
        self,
        trace: &[Complex64],
        polarity: PolarityState,
        work: &mut ExecutionContext<'_>,
    ) -> Result<PhaseSolution, PhaseOptimizationError> {
        work.complete_work()?;
        work.begin(crate::execution::ExecutionStage::AutoPhase, Some(0), None)?;
        let result = self.optimize_controlled(trace, polarity, work);
        work.check_cancelled()?;
        if result.is_ok() {
            work.complete_work()?;
        }
        result
    }
    fn optimize_controlled(
        self,
        trace: &[Complex64],
        polarity: PolarityState,
        work: &mut ExecutionContext<'_>,
    ) -> Result<PhaseSolution, PhaseOptimizationError> {
        work.ensure_work((trace.len() as u128) * Self::MAX_WORK_PASSES as u128)?;

        let mut evaluator = ObjectiveEvaluator::new(trace, polarity.is_established(), work)?;
        evaluator.charge_passes(4)?;
        let model =
            LorentzianGuard::from_magnitudes(&evaluator.normalized, evaluator.work.cancellation());
        if polarity.is_established()
            && model.is_some_and(|model| {
                model.accepts(
                    &evaluator.normalized,
                    0.0,
                    0.0,
                    evaluator.work.cancellation(),
                )
            })
        {
            evaluator.work.complete_work()?;
            return Ok(PhaseSolution {
                correction: PhaseCorrection::new(0.0, 0.0, 1.0)
                    .map_err(|_| PhaseOptimizationError::NumericalInvariantViolation)?,
                objective: 0.0,
                evaluations: 0,
            });
        }
        let mut coarse = Vec::new();
        coarse
            .try_reserve_exact(600)
            .map_err(|_| PhaseOptimizationError::AllocationFailure)?;
        for p0_index in 0..24 {
            let p0 = -180.0 + COARSE_STEP_DEGREES * p0_index as f64;
            for p1_index in 0..=24 {
                let p1 = -180.0 + COARSE_STEP_DEGREES * p1_index as f64;
                if let Some(candidate) = evaluator.candidate(p0, p1)? {
                    coarse.push(candidate);
                }
            }
        }
        if coarse.len() < 4 {
            return Err(PhaseOptimizationError::ObjectiveUndefined);
        }
        coarse.sort_unstable_by(compare_candidates);
        let starts = [coarse[0], coarse[1], coarse[2], coarse[3]];
        let mut converged = Vec::new();
        converged
            .try_reserve_exact(4)
            .map_err(|_| PhaseOptimizationError::AllocationFailure)?;
        let mut finite_simplex = false;
        for start in starts {
            match nelder_mead(start, &mut evaluator)? {
                SimplexResult::Converged(candidate) => {
                    finite_simplex = true;
                    converged.push(candidate);
                }
                SimplexResult::DidNotConverge => finite_simplex = true,
                SimplexResult::Undefined => {}
            }
        }
        if converged.is_empty() {
            return if finite_simplex {
                Err(PhaseOptimizationError::OptimizationDidNotConverge)
            } else {
                Err(PhaseOptimizationError::ObjectiveUndefined)
            };
        }
        converged.sort_unstable_by(compare_candidates);
        let best = converged[0];
        evaluator.charge_passes(1)?;
        if !polarity.is_established()
            || !model.is_some_and(|model| {
                model.accepts(
                    &evaluator.normalized,
                    best.p0,
                    best.p1,
                    evaluator.work.cancellation(),
                )
            })
        {
            return Err(PhaseOptimizationError::QualityUnverified);
        }
        let correction = PhaseCorrection::new(best.p0, best.p1, 1.0)
            .map_err(|_| PhaseOptimizationError::NumericalInvariantViolation)?;
        evaluator.work.complete_work()?;
        Ok(PhaseSolution {
            correction,
            objective: best.objective,
            evaluations: evaluator.evaluations,
        })
    }
}

/// Unique bounded phase-optimization result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseSolution {
    correction: PhaseCorrection,
    objective: f64,
    evaluations: usize,
}

impl PhaseSolution {
    /// Returns the resolved zero- and first-order phase correction.
    pub fn correction(self) -> PhaseCorrection {
        self.correction
    }

    /// Returns the finite normalized entropy objective, or zero for verified identity.
    pub fn objective(self) -> f64 {
        self.objective
    }

    /// Returns objective evaluations; verified identity requires no entropy search.
    pub fn evaluations(self) -> usize {
        self.evaluations
    }
}

/// Failure from the bounded normalized phase profile.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PhaseOptimizationError {
    /// Cooperative execution control failed.
    #[error(transparent)]
    Execution(ExecutionError),
    /// The trace has no finite nonzero scale.
    #[error("phase optimizer found no usable signal")]
    NoUsableSignal,
    /// The normalized objective is not defined on enough candidates.
    #[error("phase objective is undefined")]
    ObjectiveUndefined,
    /// No simplex met both convergence conditions within 512 evaluations.
    #[error("phase optimization did not converge")]
    OptimizationDidNotConverge,
    /// The candidate lacks verified single-positive-Lorentzian line-shape quality.
    #[error("phase candidate is outside the verified single-Lorentzian quality domain")]
    QualityUnverified,
    /// Finite input produced a non-finite intermediate or result.
    #[error("phase optimization numerical invariant was violated")]
    NumericalInvariantViolation,
    /// Work charging exceeded the caller's ledger.
    #[error("phase optimization exceeded its work limit")]
    WorkLimit,
    /// A bounded scratch allocation failed.
    #[error("phase optimization allocation failed")]
    AllocationFailure,
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    p0: f64,
    p1: f64,
    objective: f64,
}

fn compare_candidates(left: &Candidate, right: &Candidate) -> Ordering {
    left.objective
        .total_cmp(&right.objective)
        .then_with(|| left.p0.total_cmp(&right.p0))
        .then_with(|| left.p1.total_cmp(&right.p1))
}

struct ObjectiveEvaluator<'a, 'ctx> {
    normalized: Vec<Complex64>,
    positive: bool,
    work: &'a mut ExecutionContext<'ctx>,
    evaluations: usize,
}

impl<'a, 'ctx> ObjectiveEvaluator<'a, 'ctx> {
    fn charge_passes(&mut self, passes: usize) -> Result<(), PhaseOptimizationError> {
        self.work
            .charge(self.normalized.len() as u128 * passes as u128)
            .map_err(PhaseOptimizationError::from)
    }

    fn new(
        trace: &[Complex64],
        positive: bool,
        work: &'a mut ExecutionContext<'ctx>,
    ) -> Result<Self, PhaseOptimizationError> {
        if trace
            .iter()
            .any(|value| !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(PhaseOptimizationError::NumericalInvariantViolation);
        }
        let mut scale = 0.0_f64;
        for (index, value) in trace.iter().enumerate() {
            if index % 4096 == 0 {
                work.check_cancelled()?;
            }
            let magnitude = value.re.hypot(value.im);
            if !magnitude.is_finite() {
                return Err(PhaseOptimizationError::NumericalInvariantViolation);
            }
            scale = scale.max(magnitude);
        }
        if scale == 0.0 {
            return Err(PhaseOptimizationError::NoUsableSignal);
        }
        let mut normalized = Vec::new();
        normalized
            .try_reserve_exact(trace.len())
            .map_err(|_| PhaseOptimizationError::AllocationFailure)?;
        normalized.extend(trace.iter().map(|value| *value / scale));
        if normalized.len() < 2
            || normalized
                .windows(2)
                .all(|pair| pair[0].re == pair[1].re && pair[0].im == pair[1].im)
        {
            return Err(PhaseOptimizationError::ObjectiveUndefined);
        }
        Ok(Self {
            normalized,
            positive,
            work,
            evaluations: 0,
        })
    }

    fn candidate(&mut self, p0: f64, p1: f64) -> Result<Option<Candidate>, PhaseOptimizationError> {
        self.work
            .charge(self.normalized.len() as u128)
            .map_err(PhaseOptimizationError::from)?;
        self.evaluations = self
            .evaluations
            .checked_add(1)
            .ok_or(PhaseOptimizationError::NumericalInvariantViolation)?;
        let p0 = wrap_p0(p0);
        let p1 = p1.clamp(-180.0, 180.0);
        let objective = self.objective(p0, p1)?;
        Ok(objective.map(|objective| Candidate { p0, p1, objective }))
    }

    fn objective(&self, p0: f64, p1: f64) -> Result<Option<f64>, PhaseOptimizationError> {
        let length = self.normalized.len();
        let mut real = Vec::new();
        real.try_reserve_exact(length)
            .map_err(|_| PhaseOptimizationError::AllocationFailure)?;
        let mut penalty = ScaledNeumaier::default();
        for (index, value) in self.normalized.iter().enumerate() {
            if index % 4096 == 0 {
                self.work.check_cancelled()?;
            }
            let degrees = p1.mul_add(index as f64 / length as f64 - 1.0, p0);
            let angle = degrees * PI / 180.0;
            let rotated = value.re.mul_add(angle.cos(), -value.im * angle.sin());
            if !rotated.is_finite() {
                return Err(PhaseOptimizationError::NumericalInvariantViolation);
            }
            if rotated < 0.0 {
                penalty.add(rotated * rotated);
            }
            real.push(rotated);
        }
        let mut differences = Vec::new();
        differences
            .try_reserve_exact(length - 1)
            .map_err(|_| PhaseOptimizationError::AllocationFailure)?;
        let mut total = ScaledNeumaier::default();
        for (index, pair) in real.windows(2).enumerate() {
            if index % 4096 == 0 {
                self.work.check_cancelled()?;
            }
            let difference = (pair[1] - pair[0]).abs() / 2.0;
            if !difference.is_finite() {
                return Err(PhaseOptimizationError::NumericalInvariantViolation);
            }
            differences.push(difference);
            total.add(difference);
        }
        let total = total.total();
        if total == 0.0 {
            return Ok(None);
        }
        if !total.is_finite() || total < 0.0 {
            return Err(PhaseOptimizationError::NumericalInvariantViolation);
        }
        let mut entropy = ScaledNeumaier::default();
        for (index, difference) in differences.into_iter().enumerate() {
            if index % 4096 == 0 {
                self.work.check_cancelled()?;
            }
            let probability = difference / total;
            if probability > 0.0 {
                entropy.add(-probability * probability.ln());
            }
        }
        let entropy = entropy.total();
        let penalty = penalty.total() / length as f64;
        let objective = if self.positive {
            (1000.0_f64).mul_add(penalty, entropy)
        } else {
            entropy
        };
        if objective.is_finite() {
            Ok(Some(objective))
        } else {
            Err(PhaseOptimizationError::NumericalInvariantViolation)
        }
    }
}

/// Allocation bound excludes the caller's trace and allocator bookkeeping.
pub(crate) fn working_bytes(points: usize) -> Result<usize, ResourceError> {
    points
        .checked_mul(4 * std::mem::size_of::<f64>())
        .and_then(|bytes| bytes.checked_add(604 * std::mem::size_of::<Candidate>()))
        .ok_or(ResourceError::SizeOverflow)
}

pub(crate) fn estimated_work(points: usize) -> Result<u128, ResourceError> {
    (points as u128)
        .checked_mul(NormalizedAcmeV1::MAX_WORK_PASSES as u128)
        .ok_or(ResourceError::SizeOverflow)
}

#[derive(Clone, Copy)]
struct LorentzianGuard {
    center: f64,
    width: f64,
    amplitude: f64,
}

impl LorentzianGuard {
    // Reciprocal squared magnitude of a single Lorentzian is quadratic. This
    // phase-invariant check supplies a quality oracle, never a phase correction.
    fn from_magnitudes(trace: &[Complex64], token: &crate::CancellationToken) -> Option<Self> {
        let peak = trace
            .iter()
            .enumerate()
            .take_while(|_| !token.is_cancelled())
            .max_by(|left, right| left.1.norm_sqr().total_cmp(&right.1.norm_sqr()))?
            .0;
        let half = trace[peak].norm_sqr() / 2.0;
        let left = (0..peak)
            .rev()
            .take_while(|_| !token.is_cancelled())
            .find(|&index| trace[index].norm_sqr() <= half)?;
        let right = (peak + 1..trace.len())
            .take_while(|_| !token.is_cancelled())
            .find(|&index| trace[index].norm_sqr() <= half)?;
        let x_left = left as f64 - peak as f64;
        let x_right = right as f64 - peak as f64;
        let y_center = trace[peak].norm_sqr().recip();
        let slope_left = (trace[left].norm_sqr().recip() - y_center) / x_left;
        let slope_right = (trace[right].norm_sqr().recip() - y_center) / x_right;
        let a = (slope_right - slope_left) / (x_right - x_left);
        let b = slope_right - a * x_right;
        let offset = -b / (2.0 * a);
        let width_squared = y_center / a - offset * offset;
        let model = Self {
            center: peak as f64 + offset,
            width: width_squared.sqrt(),
            amplitude: (a * width_squared).sqrt().recip(),
        };
        (model.center.is_finite()
            && model.width.is_finite()
            && model.amplitude.is_finite()
            && model.width >= 2.0
            && model.width <= 0.05 * trace.len() as f64
            && model.center >= 4.0 * model.width
            && (trace.len() - 1) as f64 - model.center >= 4.0 * model.width)
            .then_some(model)
    }

    fn accepts(
        self,
        trace: &[Complex64],
        p0: f64,
        p1: f64,
        token: &crate::CancellationToken,
    ) -> bool {
        let mut error = ScaledNeumaier::default();
        let mut reference = ScaledNeumaier::default();
        for (index, &value) in trace.iter().enumerate() {
            if index % 4096 == 0 && token.is_cancelled() {
                return false;
            }
            let u = (index as f64 - self.center) / self.width;
            let expected = Complex64::new(1.0, -u) * (self.amplitude / (1.0 + u * u));
            let angle = (p0 + p1 * (index as f64 / trace.len() as f64 - 1.0)).to_radians();
            let corrected = value * Complex64::from_polar(1.0, angle);
            error.add((corrected - expected).norm_sqr());
            reference.add(expected.norm_sqr());
        }
        let error = error.total();
        let reference = reference.total();
        error.is_finite()
            && reference.is_finite()
            && reference > 0.0
            && error <= LINE_SHAPE_TOLERANCE.powi(2) * reference
    }
}

enum SimplexResult {
    Converged(Candidate),
    DidNotConverge,
    Undefined,
}

fn nelder_mead(
    start: Candidate,
    evaluator: &mut ObjectiveEvaluator<'_, '_>,
) -> Result<SimplexResult, PhaseOptimizationError> {
    let p1_step = if start.p1 == 180.0 {
        -SIMPLEX_STEP_DEGREES
    } else {
        SIMPLEX_STEP_DEGREES
    };
    let mut evaluations = 0usize;
    let Some(a) = evaluate_simplex(evaluator, start.p0, start.p1, &mut evaluations)? else {
        return Ok(SimplexResult::Undefined);
    };
    let Some(b) = evaluate_simplex(
        evaluator,
        start.p0 + SIMPLEX_STEP_DEGREES,
        start.p1,
        &mut evaluations,
    )?
    else {
        return Ok(SimplexResult::Undefined);
    };
    let Some(c) = evaluate_simplex(evaluator, start.p0, start.p1 + p1_step, &mut evaluations)?
    else {
        return Ok(SimplexResult::Undefined);
    };
    let mut simplex = [a, b, c];

    loop {
        simplex.sort_unstable_by(compare_candidates);
        if simplex_converged(&simplex) {
            return Ok(SimplexResult::Converged(simplex[0]));
        }
        if evaluations >= SIMPLEX_EVALUATIONS {
            return Ok(SimplexResult::DidNotConverge);
        }
        let best = simplex[0];
        let second = simplex[1];
        let worst = simplex[2];
        let centroid_p0 = wrap_p0(best.p0 + wrapped_delta(second.p0, best.p0) / 2.0);
        let centroid_p1 = (best.p1 + second.p1) / 2.0;
        let reflected_p0 = wrap_p0(centroid_p0 + wrapped_delta(centroid_p0, worst.p0));
        let reflected_p1 = (2.0 * centroid_p1 - worst.p1).clamp(-180.0, 180.0);
        let Some(reflected) =
            evaluate_simplex(evaluator, reflected_p0, reflected_p1, &mut evaluations)?
        else {
            return Ok(SimplexResult::DidNotConverge);
        };
        if compare_candidates(&reflected, &best) == Ordering::Less {
            let expanded_p0 = wrap_p0(centroid_p0 + 2.0 * wrapped_delta(reflected.p0, centroid_p0));
            let expanded_p1 =
                (centroid_p1 + 2.0 * (reflected.p1 - centroid_p1)).clamp(-180.0, 180.0);
            let Some(expanded) =
                evaluate_simplex(evaluator, expanded_p0, expanded_p1, &mut evaluations)?
            else {
                return Ok(SimplexResult::DidNotConverge);
            };
            simplex[2] = if compare_candidates(&expanded, &reflected) == Ordering::Less {
                expanded
            } else {
                reflected
            };
        } else if compare_candidates(&reflected, &second) == Ordering::Less {
            simplex[2] = reflected;
        } else {
            let outside = compare_candidates(&reflected, &worst) == Ordering::Less;
            let source = if outside { reflected } else { worst };
            let direction = 0.5;
            let contracted_p0 =
                wrap_p0(centroid_p0 + direction * wrapped_delta(source.p0, centroid_p0));
            let contracted_p1 =
                (centroid_p1 + direction * (source.p1 - centroid_p1)).clamp(-180.0, 180.0);
            let Some(contracted) =
                evaluate_simplex(evaluator, contracted_p0, contracted_p1, &mut evaluations)?
            else {
                return Ok(SimplexResult::DidNotConverge);
            };
            let threshold = if outside { reflected } else { worst };
            if compare_candidates(&contracted, &threshold) == Ordering::Less {
                simplex[2] = contracted;
            } else {
                for vertex in simplex.iter_mut().skip(1) {
                    let p0 = wrap_p0(best.p0 + 0.5 * wrapped_delta(vertex.p0, best.p0));
                    let p1 = best.p1 + 0.5 * (vertex.p1 - best.p1);
                    let Some(shrunk) = evaluate_simplex(evaluator, p0, p1, &mut evaluations)?
                    else {
                        return Ok(SimplexResult::DidNotConverge);
                    };
                    *vertex = shrunk;
                }
            }
        }
    }
}

fn evaluate_simplex(
    evaluator: &mut ObjectiveEvaluator<'_, '_>,
    p0: f64,
    p1: f64,
    evaluations: &mut usize,
) -> Result<Option<Candidate>, PhaseOptimizationError> {
    if *evaluations >= SIMPLEX_EVALUATIONS {
        return Ok(None);
    }
    *evaluations += 1;
    evaluator.candidate(p0, p1)
}

fn simplex_converged(simplex: &[Candidate; 3]) -> bool {
    let mut p0_span = 0.0_f64;
    for left in 0..3 {
        for right in left + 1..3 {
            p0_span = p0_span.max(wrapped_delta(simplex[left].p0, simplex[right].p0).abs());
        }
    }
    let p1_min = simplex
        .iter()
        .map(|candidate| candidate.p1)
        .fold(f64::INFINITY, f64::min);
    let p1_max = simplex
        .iter()
        .map(|candidate| candidate.p1)
        .fold(f64::NEG_INFINITY, f64::max);
    let objective_span = simplex[2].objective - simplex[0].objective;
    p0_span.max(p1_max - p1_min) <= PARAMETER_TOLERANCE_DEGREES
        && objective_span <= OBJECTIVE_RELATIVE_TOLERANCE * simplex[0].objective.abs().max(1.0)
}

fn wrap_p0(value: f64) -> f64 {
    (value + 180.0).rem_euclid(360.0) - 180.0
}

fn wrapped_delta(left: f64, right: f64) -> f64 {
    wrap_p0(left - right)
}

impl From<ExecutionError> for PhaseOptimizationError {
    fn from(error: ExecutionError) -> Self {
        match error {
            ExecutionError::WorkLimit => Self::WorkLimit,
            _ => Self::Execution(error),
        }
    }
}
