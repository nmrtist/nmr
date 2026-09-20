use crate::acquisition::AssertionId;
use crate::acquisition::DirectSamples;
use crate::acquisition::LinearComponentTransform;
use crate::acquisition::ResolvedComponentTransform;
use crate::raw::ReadLimits;

#[derive(Clone, Debug)]
pub(crate) struct VarianOptions {
    pub(super) limits: ReadLimits,
    pub(super) max_input_bytes: usize,
    pub(super) max_decode_bytes: usize,
    pub(super) max_region_bytes: usize,
    pub(super) max_materialized_bytes: usize,
    pub(super) max_metadata_bytes: usize,
    pub(super) assertions: Option<ReadAssertions>,
}

impl VarianOptions {
    pub(super) fn new(limits: ReadLimits, assertions: Option<&ReadAssertions>) -> Self {
        Self {
            limits,
            max_input_bytes: limits.source_bytes(),
            max_decode_bytes: limits.working_bytes(),
            max_region_bytes: limits.region_bytes(),
            max_materialized_bytes: limits.materialized_bytes(),
            max_metadata_bytes: limits.metadata_bytes(),
            assertions: assertions.cloned(),
        }
    }
}

impl Default for VarianOptions {
    fn default() -> Self {
        Self::new(ReadLimits::default(), None)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TraceOrder {
    Flat,
    Regular,
    Opposite,
}

/// Complete caller facts that may fill absent, but never override explicit, Varian evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadAssertions {
    pub(super) id: AssertionId,
    pub(super) direct_samples: DirectSamples,
    pub(super) trace_permutation: Vec<usize>,
    pub(super) transform: ResolvedComponentTransform,
}

impl ReadAssertions {
    /// Creates complete direct, trace-permutation, and optional transform assertions.
    pub fn try_new(
        id: AssertionId,
        direct_samples: DirectSamples,
        trace_permutation: Vec<usize>,
        transform: LinearComponentTransform,
    ) -> Result<Self, crate::acquisition::EvidenceValidationError> {
        if trace_permutation.is_empty()
            || trace_permutation
                .iter()
                .any(|&value| value >= trace_permutation.len())
            || {
                let mut seen = vec![false; trace_permutation.len()];
                trace_permutation.iter().any(|&value| {
                    let duplicate = seen[value];
                    seen[value] = true;
                    duplicate
                })
            }
        {
            return Err(crate::acquisition::EvidenceValidationError::InvalidTracePermutation);
        }
        let transform = ResolvedComponentTransform::from_assertion(transform, id.clone())?;
        Ok(Self {
            id,
            direct_samples,
            trace_permutation,
            transform,
        })
    }
    /// Returns the stable assertion ID.
    pub fn id(&self) -> &AssertionId {
        &self.id
    }
    /// Returns the asserted direct sample encoding.
    pub fn direct_samples(&self) -> DirectSamples {
        self.direct_samples
    }
    /// Returns the complete physical-to-canonical trace permutation.
    pub fn trace_permutation(&self) -> &[usize] {
        &self.trace_permutation
    }
    /// Returns the complete asserted component transform.
    pub fn transform(&self) -> &ResolvedComponentTransform {
        &self.transform
    }
}
