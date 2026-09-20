//! Dense scientific data products with independently modeled axis domains.
//!
//! Descriptors and component tensors support any nonzero rank with checked
//! shape arithmetic. Processing algorithms currently accept only rank one or
//! two and enforce their storage-order requirements separately. Declared raw
//! lineage may select or permute source axes without claiming library execution.

/// Supported already-processed spectrum formats.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Format {
    /// Bruker TopSpin `pdata` spectra.
    BrukerTopSpin,
    /// JCAMP-DX NMR spectra.
    JcampDx,
    /// JEOL Delta frequency-domain JDF spectra.
    JeolDelta,
}

pub(crate) mod dataset;
pub(crate) mod evidence;
pub(crate) mod model;

pub use crate::processed::dataset::{ProcessedDataset, ProcessedProvenance};
pub use crate::processed::evidence::{
    ProcessedAxisEvidence, ProcessedGroupDelay, SpectrumReference,
};
pub use model::*;
mod source_metadata;
pub use source_metadata::*;
