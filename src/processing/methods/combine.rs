//! Binary arithmetic retains both checked inputs as library derivation evidence.
use crate::execution::{ExecutionContext, ExecutionStage, ProgressTotal};
use crate::processed::{ProcessedData, ProcessedDataset};
use crate::processing::contracts::{
    combination::{combined_state, compatible},
    spectrum,
};
use crate::processing::contracts::{error::*, operation::*, options::*};
use crate::processing::kernels::buffer::*;
use crate::processing::prepare::resources::*;

/// A + scale*B on A's grid; B uses linear interpolation and zero outside its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearCombination {
    scale: f64,
}
/// Immutable preflight for two rank-one spectra with matching units and components.
#[derive(Debug)]
pub struct PreparedCombination<'a> {
    a: &'a crate::Dataset,
    b: &'a crate::Dataset,
    scale: f64,
    resources: crate::resource::ResourceEstimate,
    work: u128,
}
impl LinearCombination {
    /// Finite B multiplier; negative values implement subtraction.
    pub fn new(scale: f64) -> Result<Self, ProcessingError> {
        if !scale.is_finite() {
            return Err(ProcessingError::InvalidParameter(
                "linear-combination scale",
            ));
        }
        Ok(Self { scale })
    }
    /// Check axes, both input identities and complete new output/evidence capacity.
    pub fn prepare<'a>(
        self,
        a: &'a crate::Dataset,
        b: &'a crate::Dataset,
        options: ProcessingOptions,
    ) -> Result<PreparedCombination<'a>, ProcessingError> {
        let result = (|| {
            let pa = a.as_processed().ok_or(ProcessingError::InvalidParameter(
                "arithmetic requires processed input",
            ))?;
            let pb = b.as_processed().ok_or(ProcessingError::InvalidParameter(
                "arithmetic requires processed input",
            ))?;
            compatible(pa.descriptor(), pb.descriptor())?;
            let ax = &pa.descriptor().axes()[0];
            let output = descriptor_bytes(pa.descriptor())?;
            let metadata = checked_sum(&[
                crate::processing::prepare::memory::processed_apply(pa, 1)?,
                crate::processing::prepare::memory::processed_apply(pb, 1)?,
                crate::processing::prepare::memory::aggregate(a.metadata())?,
                crate::processing::prepare::memory::aggregate(b.metadata())?,
                4096,
            ])?;
            let working = checked_sum(&[output, metadata])?;
            check_resource(
                crate::resource::ResourceKind::OutputBytes,
                output,
                options.output_bytes(),
            )?;
            check_resource(
                crate::resource::ResourceKind::MetadataBytes,
                metadata,
                options.metadata_bytes(),
            )?;
            check_resource(
                crate::resource::ResourceKind::WorkingBytes,
                working,
                options.working_bytes(),
            )?;
            let work =
                (ax.points() as u128) * (usize::BITS as u128 + 16) * (ax.component_count() as u128);
            Ok(PreparedCombination {
                a,
                b,
                scale: self.scale,
                resources: crate::resource::ResourceEstimate::new(output, metadata, working),
                work,
            })
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Dataset),
                ProcessingPhase::Preflight,
            )
        })
    }
}
impl PreparedCombination<'_> {
    /// Conservative newly allocated output, provenance and working bytes.
    pub fn resources(&self) -> crate::resource::ResourceEstimate {
        self.resources
    }
    /// Work bound including interpolation searches and all component fields.
    pub fn estimated_work(&self) -> u128 {
        self.work
    }
    /// Execute against the immutably borrowed input pair.
    pub fn execute(self) -> Result<crate::Dataset, ProcessingError> {
        self.execute_with_context(&mut ExecutionContext::default())
    }
    /// Execute with shared work/cancellation; records local operation zero.
    pub fn execute_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        let result = (|| {
            control.ensure_work(self.work)?;
            control.observe_payload(self.resources.working_bytes());
            control.begin(
                ExecutionStage::Processing,
                Some(0),
                Some(ProgressTotal::UpperBound(self.work)),
            )?;
            control.charge(self.work)?;
            let a = self.a.as_processed().unwrap();
            let b = self.b.as_processed().unwrap();
            let ax = &a.descriptor().axes()[0];
            let bx = &b.descriptor().axes()[0];
            let c = ax.component_count();
            let bn = bx.points();
            let ascending =
                bn == 1 || spectrum::coordinate(bx, 0)? < spectrum::coordinate(bx, bn - 1)?;
            let index = |i: usize| if ascending { i } else { bn - 1 - i };
            let x_at = |i| spectrum::coordinate(bx, index(i));
            let mut output = try_zeroed(a.data().samples().len())?;
            for p in 0..ax.points() {
                if p % 128 == 0 {
                    control.check_cancelled()?;
                }
                let x = spectrum::coordinate(ax, p)?;
                let (mut lo, mut hi) = (0, bn);
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    if x_at(mid)? < x {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                for lane in 0..c {
                    let at = |i| b.data().samples()[index(i) * c + lane];
                    let interpolated = if lo < bn && x_at(lo)? == x {
                        at(lo)
                    } else if lo == 0 || lo == bn {
                        0.0
                    } else {
                        let left = x_at(lo - 1)?;
                        let right = x_at(lo)?;
                        let fraction = (x - left) / (right - left);
                        at(lo - 1) * (1.0 - fraction) + at(lo) * fraction
                    };
                    output[p * c + lane] =
                        a.data().samples()[p * c + lane] + self.scale * interpolated;
                }
            }
            let descriptor = a.descriptor().clone();
            let state = combined_state(&descriptor, a.processing_state(), b.processing_state());
            let data = ProcessedData::new(
                descriptor.logical_shape(),
                descriptor.component_counts(),
                output,
            )?;
            let digests = crate::canonical_digest::processed_digests(&descriptor, &data, &state);
            let mut sources = self.a.sources().to_vec();
            sources.extend_from_slice(self.b.sources());
            for (i, source) in sources.iter_mut().enumerate() {
                source.assign_id(i);
            }
            let provenance = crate::derivation::provenance(
                vec![
                    crate::derivation::DerivationInput::capture(self.a),
                    crate::derivation::DerivationInput::capture(self.b),
                ],
                crate::derivation::DerivationOperation::LinearCombination { scale: self.scale },
                descriptor.clone(),
                state,
                digests,
                sources,
            );
            let result = ProcessedDataset::new_library(control, descriptor, data, provenance)?;
            Ok(self.a.derived_processed_with_secondary(result, self.b))
        })();
        result.map_err(|e: ProcessingError| {
            e.located(
                Some(0),
                Some(OperationTarget::Dataset),
                ProcessingPhase::Execution,
            )
        })
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl LinearCombination {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&f64,) {
        (&self.scale,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(parts: (f64,)) -> Result<Self, crate::internal::ModelError> {
        let (scale,) = parts;
        let value = Self { scale };

        Ok(value)
    }
}
