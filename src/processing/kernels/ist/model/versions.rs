use crate::execution::ExecutionContext;
use crate::resource::WorkLedger;

use super::reconstruction::reconstruct;
use super::resources::estimate_work;
use super::resources::memory_preflight;
use super::{
    INDIRECT_ZERO_FILL, IstError, IstInput, IstOptions, IstOutput, M_MEASURED, N_LOGICAL,
    validate_options,
};

/// Fixed-grid phase-covariant group IST profile (512 points, 128 observations).
///
/// Experimental: this fixed profile does not establish reconstruction quality
/// for general NUS data. An independent noise standard deviation or explicit
/// noiseless assertion is required; signal magnitudes never estimate noise.
/// It remains an ordinary public API under the crate's
/// [compatibility policy](crate#experimental-algorithms).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PhaseCovariantGroupRetainedIstV1;

impl PhaseCovariantGroupRetainedIstV1 {
    /// Required full indirect logical length.
    pub const N_LOGICAL: usize = N_LOGICAL;
    /// Required measured indirect coordinate count.
    pub const M_MEASURED: usize = M_MEASURED;
    /// Fixed subsequent indirect zero-fill length.
    pub const INDIRECT_ZERO_FILL: usize = INDIRECT_ZERO_FILL;

    /// Creates the immutable V1 profile.
    pub fn new() -> Self {
        Self
    }

    /// Returns the preflight work bound for an input and options.
    pub fn estimated_work(self, input: &IstInput, options: IstOptions) -> Result<u128, IstError> {
        validate_options(options)?;
        estimate_work(input, options.max_iterations, false)
    }

    /// Reconstructs the fixed 512-point indirect grid.
    pub fn reconstruct(
        self,
        input: &IstInput,
        work: &mut WorkLedger,
        options: IstOptions,
    ) -> Result<IstOutput, IstError> {
        self.reconstruct_with_context(input, options, &mut ExecutionContext::new(work))
    }
    /// Reconstructs with shared work, progress and cooperative cancellation.
    pub fn reconstruct_with_context(
        self,
        input: &IstInput,
        options: IstOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<IstOutput, IstError> {
        if input.nlogical != N_LOGICAL || input.measured_indices.len() != M_MEASURED {
            return Err(IstError::InvalidInput);
        }
        reconstruct(input, options, control, false)
    }
}

/// General-grid phase-covariant group IST, with explicit noise and iteration bounds.
/// Threshold continuation reaches each column's floor without slowing when
/// the iteration ceiling increases. At that fixed floor, FISTA extrapolation
/// accelerates the same retained-data map; convergence uses its fixed-point residual.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneralGridPhaseCovariantGroupRetainedIstV1;
impl GeneralGridPhaseCovariantGroupRetainedIstV1 {
    /// Conservative work before sample analysis.
    pub fn estimated_work(self, input: &IstInput, options: IstOptions) -> Result<u128, IstError> {
        validate_options(options)?;
        estimate_work(input, options.max_iterations, true)
    }
    /// Numerical output and peak-working capacity, excluding borrowed input.
    pub fn resources(
        self,
        input: &IstInput,
    ) -> Result<crate::resource::ResourceEstimate, IstError> {
        let memory = memory_preflight(input, true)?;
        Ok(crate::resource::ResourceEstimate::new(
            memory.output_bytes,
            0,
            memory.peak_working_bytes,
        ))
    }
    /// Reconstruct arbitrary positive grid and observation counts, rejecting duplicates.
    pub fn reconstruct_with_context(
        self,
        input: &IstInput,
        options: IstOptions,
        control: &mut ExecutionContext<'_>,
    ) -> Result<IstOutput, IstError> {
        reconstruct(input, options, control, true)
    }
}
