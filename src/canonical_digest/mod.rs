//! Frozen versioned byte protocol for canonical acquisition identities.

mod model;
pub use model::{CanonicalDatasetDigests, CanonicalDigest};

mod encoding;
mod processed;
mod raw;

pub(crate) use processed::{
    check_processed_evidence, processed_digests, processed_digests_controlled,
};
pub(crate) use raw::{
    dataset_digests, dataset_digests_controlled, descriptor_digest_controlled, raw_binding,
};

#[cfg(test)]
mod tests;
