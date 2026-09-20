//! Complete public workflows across reading, processing and persistence.

#[allow(dead_code)] // This shared builder module serves several independent test targets.
#[path = "../support/datasets.rs"]
mod datasets;

mod external_derivation;
mod prepare_execute;
mod processing;

#[path = "../support/fixture_paths.rs"]
mod fixture_paths;
