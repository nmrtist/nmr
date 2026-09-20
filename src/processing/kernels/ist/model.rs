use crate::execution::ExecutionError;
use thiserror::Error;

const N_LOGICAL: usize = 512;
const M_MEASURED: usize = 128;
const INDIRECT_ZERO_FILL: usize = 1024;
const DEFAULT_ITERATIONS: usize = 768;
const ABSOLUTE_ITERATIONS: usize = 2048;
const DEFAULT_BYTES: usize = 512 * 1024 * 1024;
const DEFAULT_RECONSTRUCTION_WORK: u128 = 35_000_000_000;

/// Compact public component input in `[M,F2,C,2]` row-major order.
#[derive(Clone, Debug, PartialEq)]
pub struct IstInput {
    nlogical: usize,
    f2_points: usize,
    cartesian_fields: usize,
    measured_indices: Vec<usize>,
    components: Vec<f64>,
}

impl IstInput {
    /// Creates checked compact input with exactly 128 unique coordinates.
    pub fn new(
        f2_points: usize,
        cartesian_fields: usize,
        measured_indices: Vec<usize>,
        components: Vec<f64>,
    ) -> Result<Self, IstError> {
        if measured_indices.len() != M_MEASURED {
            return Err(IstError::InvalidInput);
        }
        Self::with_grid(
            N_LOGICAL,
            f2_points,
            cartesian_fields,
            measured_indices,
            components,
        )
    }
    /// Construct a general-grid compact input. Observation order is retained;
    /// duplicate and out-of-grid coordinates are errors, never overwritten.
    pub fn with_grid(
        nlogical: usize,
        f2_points: usize,
        cartesian_fields: usize,
        measured_indices: Vec<usize>,
        components: Vec<f64>,
    ) -> Result<Self, IstError> {
        if nlogical == 0
            || f2_points == 0
            || !matches!(cartesian_fields, 1 | 2)
            || measured_indices.is_empty()
            || measured_indices.len() > nlogical
        {
            return Err(IstError::InvalidInput);
        }
        // Check without allocating a grid-sized mask in a constructor.
        let mut sorted = measured_indices.clone();
        sorted.sort_unstable();
        if sorted.last().is_some_and(|v| *v >= nlogical) || sorted.windows(2).any(|v| v[0] == v[1])
        {
            return Err(IstError::InvalidInput);
        }
        let expected = measured_indices
            .len()
            .checked_mul(f2_points)
            .and_then(|value| value.checked_mul(cartesian_fields))
            .and_then(|value| value.checked_mul(2))
            .ok_or(IstError::SizeOverflow)?;
        if components.len() != expected || components.iter().any(|value| !value.is_finite()) {
            return Err(IstError::InvalidInput);
        }
        Ok(Self {
            nlogical,
            f2_points,
            cartesian_fields,
            measured_indices,
            components,
        })
    }

    /// Returns the full indirect logical length.
    pub fn nlogical(&self) -> usize {
        self.nlogical
    }

    /// Returns direct frequency point count.
    pub fn f2_points(&self) -> usize {
        self.f2_points
    }

    /// Returns the number of Cartesian fields grouped at each grid point.
    pub fn cartesian_fields(&self) -> usize {
        self.cartesian_fields
    }

    /// Returns measured logical coordinates in compact acquisition order.
    pub fn measured_indices(&self) -> &[usize] {
        &self.measured_indices
    }

    /// Returns public component values in `[M,F2,C,2]` row-major order.
    pub fn components(&self) -> &[f64] {
        &self.components
    }

    fn complex_len(&self) -> usize {
        self.components.len() / 2
    }
}

/// Dense reconstructed public components in `[N,F2,C,2]` row-major order.
#[derive(Clone, Debug, PartialEq)]
pub struct IstOutput {
    nlogical: usize,
    f2_points: usize,
    cartesian_fields: usize,
    components: Vec<f64>,
    iterations: usize,
    estimated_work: u128,
    noise_standard_deviation: f64,
    columns: Vec<ColumnState>,
}

impl IstOutput {
    /// Returns the full indirect logical length.
    pub fn nlogical(&self) -> usize {
        self.nlogical
    }

    /// Returns direct frequency point count.
    pub fn f2_points(&self) -> usize {
        self.f2_points
    }

    /// Returns grouped Cartesian field count.
    pub fn cartesian_fields(&self) -> usize {
        self.cartesian_fields
    }

    /// Returns public component values in `[N,F2,C,2]` row-major order.
    pub fn components(&self) -> &[f64] {
        &self.components
    }

    /// Returns completed reconstruction iterations.
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// Returns work units charged during preflight.
    pub fn estimated_work(&self) -> u128 {
        self.estimated_work
    }

    /// Returns the caller-supplied noise standard deviation per real component.
    pub fn noise_standard_deviation(&self) -> f64 {
        self.noise_standard_deviation
    }

    /// Returns the largest terminal threshold across direct columns.
    /// Use [`Self::column_final_threshold`] for a particular column's value.
    pub fn final_threshold(&self) -> f64 {
        self.columns
            .iter()
            .map(|column| column.floor)
            .fold(0.0, f64::max)
    }

    /// Returns the largest final relative fixed-point residual across columns.
    /// For the fixed-grid profile this is the ordinary iterate change; general-grid acceleration
    /// checks the unaccelerated map at its extrapolated input. This is not a
    /// reconstruction error bound. Each column converges independently.
    pub fn relative_change(&self) -> f64 {
        self.columns
            .iter()
            .map(|column| column.relative_change)
            .fold(0.0, f64::max)
    }

    /// Returns one column's final threshold in unnormalized Fourier amplitude units.
    /// All-zero columns have zero threshold. Out-of-range columns return `None`.
    pub fn column_final_threshold(&self, column: usize) -> Option<f64> {
        self.columns.get(column).map(|state| state.floor)
    }

    /// Returns one column's final relative L2 fixed-point residual.
    pub fn column_relative_change(&self, column: usize) -> Option<f64> {
        self.columns.get(column).map(|state| state.relative_change)
    }

    /// Returns one column's iteration count; all-zero columns require no iterations.
    pub fn column_iterations(&self, column: usize) -> Option<usize> {
        self.columns.get(column).map(|state| state.iterations)
    }

    /// Returns the measured-data residual; retained samples are checked bit for bit.
    /// This is always zero on success and does not validate unmeasured samples.
    pub fn measured_relative_residual(&self) -> f64 {
        0.0
    }
}

/// Independent output, working-memory, iteration, and reconstruction-work limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IstOptions {
    max_output_bytes: usize,
    max_working_bytes: usize,
    max_iterations: usize,
    max_reconstruction_work: u128,
    noise_standard_deviation: Option<f64>,
}

impl IstOptions {
    /// Creates default limits with no noise assumption. Reconstruction requires
    /// [`Self::noise_standard_deviation`] or [`Self::noiseless`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Supplies an independent noise standard deviation per real component.
    ///
    /// Estimate this from separately identified noise-only observations using
    /// the same amplitude units. The value must be finite and nonnegative.
    pub fn noise_standard_deviation(mut self, sigma: f64) -> Result<Self, IstError> {
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(IstError::InvalidOptions);
        }
        self.noise_standard_deviation = Some(sigma);
        Ok(self)
    }

    /// Explicitly asserts zero noise for the supplied data.
    pub fn noiseless(mut self) -> Self {
        self.noise_standard_deviation = Some(0.0);
        self
    }

    /// Sets the independent dense public output limit.
    pub fn max_output_bytes(mut self, bytes: usize) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Sets peak newly allocated bytes, including output, FFT plans and scratch.
    /// Caller-owned borrowed input is excluded; allocator overhead is not measured.
    pub fn max_working_bytes(mut self, bytes: usize) -> Self {
        self.max_working_bytes = bytes;
        self
    }

    /// Sets an iteration ceiling no greater than 2048.
    pub fn max_iterations(mut self, iterations: usize) -> Result<Self, IstError> {
        if iterations == 0 || iterations > ABSOLUTE_ITERATIONS {
            return Err(IstError::InvalidOptions);
        }
        self.max_iterations = iterations;
        Ok(self)
    }

    /// Sets the independent reconstruction-work limit.
    pub fn max_reconstruction_work(mut self, work: u128) -> Self {
        self.max_reconstruction_work = work;
        self
    }

    /// Returns the output byte limit.
    pub fn output_bytes(self) -> usize {
        self.max_output_bytes
    }

    /// Returns the working byte limit.
    pub fn working_bytes(self) -> usize {
        self.max_working_bytes
    }

    /// Returns the iteration ceiling.
    pub fn iterations(self) -> usize {
        self.max_iterations
    }

    /// Returns the reconstruction-work limit.
    pub fn reconstruction_work(self) -> u128 {
        self.max_reconstruction_work
    }
}

impl Default for IstOptions {
    fn default() -> Self {
        Self {
            max_output_bytes: DEFAULT_BYTES,
            max_working_bytes: DEFAULT_BYTES,
            max_iterations: DEFAULT_ITERATIONS,
            max_reconstruction_work: DEFAULT_RECONSTRUCTION_WORK,
            noise_standard_deviation: None,
        }
    }
}

/// Failure from fixed-profile NUS reconstruction.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum IstError {
    /// Caller requested cooperative cancellation.
    #[error("IST cancelled")]
    Cancelled,
    /// Rank, shape, schedule, component count, or sample values are invalid.
    #[error("IST input does not satisfy the selected grid/component contract")]
    InvalidInput,
    /// An option is zero or exceeds its absolute bound.
    #[error("IST options are invalid")]
    InvalidOptions,
    /// The caller has not supplied independent noise evidence or asserted no noise.
    #[error("IST requires an independent noise standard deviation or a noiseless assertion")]
    NoiseEstimateRequired,
    /// Threshold evidence does not support recoverable signal.
    #[error("IST input has no recoverable signal")]
    NoRecoverableSignal,
    /// Three consecutive floor-threshold convergence rounds were not reached.
    #[error("IST reconstruction did not converge")]
    DidNotConverge,
    /// Dense public output exceeds its independent limit.
    #[error("IST output exceeds max_output_bytes")]
    OutputLimit,
    /// Peak live buffers exceed the independent working limit.
    #[error("IST working set exceeds max_working_bytes")]
    WorkingLimit,
    /// Audited reconstruction or total processing work is exceeded.
    #[error("IST reconstruction exceeds its work limit")]
    WorkLimit,
    /// A checked size or work computation overflowed.
    #[error("IST size computation overflow")]
    SizeOverflow,
    /// A bounded allocation failed.
    #[error("IST allocation failed")]
    AllocationFailure,
    /// Finite input produced a non-finite numerical result.
    #[error("IST numerical invariant was violated")]
    NumericalInvariantViolation,
    /// A measured value changed during replacement or final scatter.
    #[error("IST retained measured value changed bits")]
    RetainedBitsMismatch,
}

struct MemoryEstimate {
    output_bytes: usize,
    peak_working_bytes: usize,
}

fn validate_options(options: IstOptions) -> Result<(), IstError> {
    if options.max_iterations == 0 || options.max_iterations > ABSOLUTE_ITERATIONS {
        return Err(IstError::InvalidOptions);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct ColumnState {
    lambda: f64,
    floor: f64,
    relative_change: f64,
    converged_at_floor: usize,
    iterations: usize,
    converged: bool,
    momentum_t: f64,
}

impl From<ExecutionError> for IstError {
    fn from(error: ExecutionError) -> Self {
        match error {
            ExecutionError::Cancelled => Self::Cancelled,
            ExecutionError::WorkLimit => Self::WorkLimit,
            ExecutionError::SizeOverflow => Self::SizeOverflow,
        }
    }
}

mod iteration;
mod reconstruction;
pub(super) mod resources;
mod versions;

pub use versions::{GeneralGridPhaseCovariantGroupRetainedIstV1, PhaseCovariantGroupRetainedIstV1};
