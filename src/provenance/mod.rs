//! Source provenance and canonical identity shared by raw and processed datasets.

pub use crate::canonical_digest::{CanonicalDatasetDigests, CanonicalDigest};

mod lineage;
mod normalization;
mod read_record;
mod source;

pub use lineage::{InputAxisRef, InputSlot};
pub use normalization::SampleNormalization;
pub use read_record::{ProcessedReadRecord, ProcessedReadTransform};
pub use source::{ProvenanceError, SourceDigest, SourceFile, SourceId, SourceIdentity, SourceKind};
