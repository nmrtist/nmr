//! Analytic signals and independent thresholds for processing quality.
//! Original tests are synthetic, distributed under the source repository license.
//! Deterministic, black-box quality contracts for automatic spectrum correction.
//!
//! These tests intentionally assert signal-level outcomes rather than recovered
//! parameters. A different optimizer or baseline solver is therefore free to
//! replace the current implementation as long as the user-visible quality is
//! preserved.

use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedDataset, ProcessedDescriptor, ProcessedOrigin,
    ProcessedProvenance,
};
pub(crate) struct Spectrum {
    pub(crate) values: Vec<Complex64>,
}
pub(crate) fn checked(spec: &Spectrum) -> nmr::Dataset {
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Ppm),
        spec.values.len(),
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0,
        },
        ComponentBasis::Cartesian,
    )
    .unwrap();
    ProcessedDataset::from_dense_samples(
        ProcessedDescriptor::new(vec![axis]).unwrap(),
        spec.values.iter().flat_map(|v| [v.re, v.im]).collect(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
    .into()
}
pub(crate) fn replace(spec: &mut Spectrum, output: &nmr::Dataset) {
    spec.values = output
        .as_dense_processed()
        .unwrap()
        .samples()
        .chunks_exact(2)
        .map(|v| Complex64::new(v[0], v[1]))
        .collect();
}
pub(crate) fn spectrum(values: Vec<Complex64>) -> Spectrum {
    Spectrum { values }
}

pub(crate) fn deterministic_complex_noise(i: usize, amplitude: f64) -> Complex64 {
    let x = i as f64;
    Complex64::new(
        amplitude * ((0.731 * x).sin() + 0.37 * (0.193 * x).cos()),
        amplitude * (0.61 * (0.417 * x).cos() - 0.29 * (0.113 * x).sin()),
    )
}
