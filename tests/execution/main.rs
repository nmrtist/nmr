//! Execution cancellation, resource accounting and reports.

#[allow(dead_code)] // This shared builder module serves several independent test targets.
#[path = "../support/datasets.rs"]
mod datasets;

mod cancellation;
mod report;
mod resources;

#[path = "../support/fixture_paths.rs"]
mod fixture_paths;
