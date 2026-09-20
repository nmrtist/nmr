//! Read-only effective calibration and filter evidence for a processed axis.
use crate::acquisition::{ChemicalShiftReference, NormalizationEvidence, PendingGroupDelay};
use crate::processing::contracts::state::{AxisState, ProcessingDelayState};

/// A spectrum's ppm-to-Hz interval scale, independent of its acquisition carrier.
#[derive(Clone, Debug, PartialEq)]
pub struct SpectrumReference {
    pub(crate) frequency_mhz: f64,
    pub(crate) evidence: NormalizationEvidence,
}
impl SpectrumReference {
    /// Reference frequency in MHz; multiply a ppm interval by this value for Hz.
    pub fn reference_frequency_mhz(&self) -> f64 {
        self.frequency_mhz
    }
    /// Authority and versioned rule establishing this scale.
    pub fn evidence(&self) -> &NormalizationEvidence {
        &self.evidence
    }
}

/// Current digital-filter state; imported spectra without evidence remain unknown.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProcessedGroupDelay<'a> {
    /// This axis has no applicable filter.
    NotApplicable,
    /// No evidence establishes whether correction is needed or already applied.
    Unknown,
    /// A known delay still requires correction.
    Pending(&'a PendingGroupDelay),
    /// The library applied a correction with the recorded delay and evidence.
    Corrected {
        /// Applied delay in original sample points.
        delay_points: f64,
        /// Supporting acquisition evidence, when established.
        evidence: Option<&'a NormalizationEvidence>,
    },
}

/// Borrowed effective evidence at the current descriptor axis position.
#[derive(Clone, Copy, Debug)]
pub struct ProcessedAxisEvidence<'a>(pub(crate) &'a AxisState);
impl<'a> ProcessedAxisEvidence<'a> {
    /// Acquisition observe frequency; never inferred from a processing reference.
    pub fn observe_frequency_mhz(self) -> Option<f64> {
        self.0
            .axis
            .frequency_evidence()
            .and_then(|f| f.observe_frequency_mhz())
    }
    /// Effective ppm-to-Hz interval scale, retained through binning and derivation.
    pub fn reference_frequency_mhz(self) -> Option<f64> {
        self.chemical_shift_reference()
            .map(ChemicalShiftReference::reference_frequency_mhz)
            .or_else(|| {
                self.0
                    .axis
                    .spectrum_reference()
                    .map(SpectrumReference::reference_frequency_mhz)
            })
    }
    /// Authority establishing the effective reference scale.
    pub fn reference_evidence(self) -> Option<&'a NormalizationEvidence> {
        self.chemical_shift_reference()
            .map(ChemicalShiftReference::evidence)
            .or_else(|| {
                self.0
                    .axis
                    .spectrum_reference()
                    .map(SpectrumReference::evidence)
            })
    }
    /// Full carrier-relative calibration, only when the carrier is established.
    /// An imported SF alone supplies a scale, not a carrier chemical shift.
    pub fn chemical_shift_reference(self) -> Option<&'a ChemicalShiftReference> {
        self.0.chemical_shift_reference.as_ref()
    }
    /// Effective filter state after all completed operations.
    pub fn group_delay(self) -> ProcessedGroupDelay<'a> {
        match &self.0.group_delay {
            ProcessingDelayState::NotApplicable => ProcessedGroupDelay::NotApplicable,
            ProcessingDelayState::Unknown => ProcessedGroupDelay::Unknown,
            ProcessingDelayState::Pending(value) => ProcessedGroupDelay::Pending(value),
            ProcessingDelayState::Corrected { delay, evidence } => ProcessedGroupDelay::Corrected {
                delay_points: *delay,
                evidence: evidence.as_ref(),
            },
        }
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SpectrumReference {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&f64, &NormalizationEvidence) {
        (&self.frequency_mhz, &self.evidence)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (f64, NormalizationEvidence),
    ) -> Result<Self, crate::internal::ModelError> {
        let (frequency_mhz, evidence) = parts;
        let value = Self {
            frequency_mhz,
            evidence,
        };

        if !value.frequency_mhz.is_finite() || value.frequency_mhz <= 0.0 {
            return Err(crate::internal::ModelError::Structure);
        }

        Ok(value)
    }
}
