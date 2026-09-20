//! Vendor readers, format selection, provenance and malformed inputs.

#[allow(dead_code)] // Reader, processing and snapshot tests use different helpers.
#[path = "../support/bruker_group_delay.rs"]
mod group_delay_inputs;

#[allow(dead_code)] // Shared synthetic inputs also used by processing tests.
#[path = "../support/reading_inputs.rs"]
mod inputs;

mod bruker;
mod context;
mod fixture_evidence;
mod fixture_support;
mod jcamp_dx;
mod jeol;
mod raw_support;
mod robustness;
mod selection;
mod support;
mod varian;
