//! Processing methods, scientific accuracy, contracts and replay.

#[allow(dead_code)] // This shared builder module serves several independent test targets.
#[path = "../support/datasets.rs"]
mod datasets;

#[allow(dead_code)] // Reader, processing and snapshot tests use different helpers.
#[path = "../support/bruker_group_delay.rs"]
mod group_delay_inputs;

#[allow(dead_code)] // Input builders are shared with reader tests.
#[path = "../support/reading_inputs.rs"]
mod inputs;

mod baseline;
mod bruker_delay;
mod components;
mod contracts;
mod errors;
mod fft_window;
mod fourier_accuracy;
mod group_delay;
mod group_delay_evidence;
mod history;
mod jeol;
mod nus;
mod phase;
mod pipeline;
mod preflight;
mod prepare_api;
mod quality_support;
mod reference_propagation;
mod replay;
mod spectrum;
mod spectrum_support;
mod support;
mod window_accuracy;
