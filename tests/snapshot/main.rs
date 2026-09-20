//! Snapshot validation and offline restoration.

#[allow(dead_code)] // This shared builder module serves several independent test targets.
#[path = "../support/datasets.rs"]
mod datasets;

mod validation;

#[path = "../support/fixture_paths.rs"]
mod fixture_paths;
