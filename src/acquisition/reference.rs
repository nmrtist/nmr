use super::{EvidenceValidationError, NormalizationEvidence, NormalizationFact};

/// A caller-independent chemical-shift reference.
#[derive(Clone, Debug, PartialEq)]
pub struct ChemicalShiftReference {
    carrier_ppm: f64,
    reference_frequency_mhz: f64,
    evidence: NormalizationEvidence,
}

impl ChemicalShiftReference {
    /// Creates an explicitly user-authored reference.
    pub fn user_constructed(
        carrier_ppm: f64,
        reference_frequency_mhz: f64,
    ) -> Result<Self, EvidenceValidationError> {
        Self::try_new(
            carrier_ppm,
            reference_frequency_mhz,
            NormalizationEvidence::user_constructed(vec![
                NormalizationFact::ChemicalShiftReference,
            ])?,
        )
    }

    pub(crate) fn resolved(
        carrier_ppm: f64,
        reference_frequency_mhz: f64,
        evidence: NormalizationEvidence,
    ) -> Result<Self, EvidenceValidationError> {
        Self::try_new(carrier_ppm, reference_frequency_mhz, evidence)
    }

    fn try_new(
        carrier_ppm: f64,
        reference_frequency_mhz: f64,
        evidence: NormalizationEvidence,
    ) -> Result<Self, EvidenceValidationError> {
        if !carrier_ppm.is_finite()
            || !reference_frequency_mhz.is_finite()
            || reference_frequency_mhz <= 0.0
        {
            return Err(EvidenceValidationError::InvalidChemicalShiftReference);
        }
        Ok(Self {
            carrier_ppm,
            reference_frequency_mhz,
            evidence,
        })
    }

    /// Returns the carrier chemical shift in ppm.
    pub fn carrier_ppm(&self) -> f64 {
        self.carrier_ppm
    }

    /// Returns the positive reference frequency in MHz.
    pub fn reference_frequency_mhz(&self) -> f64 {
        self.reference_frequency_mhz
    }

    /// Returns the supporting evidence.
    pub fn evidence(&self) -> &NormalizationEvidence {
        &self.evidence
    }

    /// Converts a signed offset in Hz; positive means higher absolute frequency.
    /// Rejects non-finite inputs and non-finite conversion results.
    pub fn ppm(&self, frequency_hz: f64) -> Result<f64, EvidenceValidationError> {
        if !frequency_hz.is_finite() {
            return Err(EvidenceValidationError::InvalidFrequencyOffset);
        }
        let ppm = self.carrier_ppm + frequency_hz / self.reference_frequency_mhz;
        if !ppm.is_finite() {
            return Err(EvidenceValidationError::InvalidChemicalShiftResult);
        }
        Ok(ppm)
    }
}

/// Evidence for a pending direct-axis digital-filter correction.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingGroupDelay {
    delay_points: f64,
    evidence: NormalizationEvidence,
}

impl PendingGroupDelay {
    /// Creates a user-authored finite non-negative delay.
    pub fn user_constructed(delay_points: f64) -> Result<Self, EvidenceValidationError> {
        Self::try_new(
            delay_points,
            NormalizationEvidence::user_constructed(vec![NormalizationFact::DigitalFilterDelay])?,
        )
    }

    pub(crate) fn resolved(
        delay_points: f64,
        evidence: NormalizationEvidence,
    ) -> Result<Self, EvidenceValidationError> {
        Self::try_new(delay_points, evidence)
    }

    fn try_new(
        delay_points: f64,
        evidence: NormalizationEvidence,
    ) -> Result<Self, EvidenceValidationError> {
        if !delay_points.is_finite() || delay_points < 0.0 {
            return Err(EvidenceValidationError::InvalidGroupDelay);
        }
        Ok(Self {
            delay_points,
            evidence,
        })
    }

    /// Returns the delay in logical complex points.
    pub fn delay_points(&self) -> f64 {
        self.delay_points
    }

    /// Returns the applicability and derivation evidence.
    pub fn evidence(&self) -> &NormalizationEvidence {
        &self.evidence
    }
}

/// Raw direct-axis digital-filter correction state.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum GroupDelayState {
    /// The input encoding has no applicable digital-filter delay.
    NotApplicable,
    /// Applicability or the delay value could not be established.
    Unknown,
    /// A correction is required and has not yet been applied.
    Pending(PendingGroupDelay),
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ChemicalShiftReference {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&f64, &f64, &NormalizationEvidence) {
        (
            &self.carrier_ppm,
            &self.reference_frequency_mhz,
            &self.evidence,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (f64, f64, NormalizationEvidence),
    ) -> Result<Self, crate::internal::ModelError> {
        let (carrier_ppm, reference_frequency_mhz, evidence) = parts;
        let value = Self {
            carrier_ppm,
            reference_frequency_mhz,
            evidence,
        };

        let value = Self::try_new(
            value.carrier_ppm,
            value.reference_frequency_mhz,
            value.evidence,
        )
        .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl PendingGroupDelay {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&f64, &NormalizationEvidence) {
        (&self.delay_points, &self.evidence)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (f64, NormalizationEvidence),
    ) -> Result<Self, crate::internal::ModelError> {
        let (delay_points, evidence) = parts;
        let value = Self {
            delay_points,
            evidence,
        };

        let value = Self::try_new(value.delay_points, value.evidence)
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
