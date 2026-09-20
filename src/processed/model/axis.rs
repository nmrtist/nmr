use super::*;

/// One mathematical axis of a processed data product.
/// Clones share immutable storage; consuming builders detach when necessary.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessedAxis(pub(super) std::sync::Arc<ProcessedAxisInner>);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProcessedAxisInner {
    pub(super) label: Option<String>,
    pub(super) role: AxisRole,
    pub(super) domain: AxisDomain,
    pub(super) unit: Option<AxisUnit>,
    pub(super) quantity: Option<AxisQuantity>,
    pub(super) points: usize,
    pub(super) coordinates: AxisCoordinates,
    pub(super) component_basis: ComponentBasis,
    pub(super) nucleus: Option<String>,
    pub(super) spectral_width_hz: Option<f64>,
    pub(super) frequency_evidence: Option<FrequencyEvidence>,
    pub(super) spectrum_reference: Option<crate::processed::SpectrumReference>,
}

impl ProcessedAxis {
    fn from_inner(inner: ProcessedAxisInner) -> Result<Self, ProcessedValidationError> {
        #[cfg(test)]
        AXIS_ALLOCATIONS.with(|count| count.set(count.get() + 1));
        let value = Self(std::sync::Arc::new(inner));
        value.validate()?;
        Ok(value)
    }

    /// Creates a checked processed axis.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role: AxisRole,
        domain: AxisDomain,
        unit: Option<AxisUnit>,
        points: usize,
        coordinates: AxisCoordinates,
        component_basis: ComponentBasis,
    ) -> Result<Self, ProcessedValidationError> {
        Self::from_inner(ProcessedAxisInner {
            label: None,
            role,
            domain,
            unit,
            quantity: None,
            points,
            coordinates,
            component_basis,
            nucleus: None,
            spectral_width_hz: None,
            frequency_evidence: None,
            spectrum_reference: None,
        })
    }

    // Preserve already validated portable metadata without parsing or trimming it again.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rebuilt(
        &self,
        domain: AxisDomain,
        unit: Option<AxisUnit>,
        points: usize,
        coordinates: AxisCoordinates,
        component_basis: ComponentBasis,
        spectral_width_hz: Option<f64>,
    ) -> Result<Self, ProcessedValidationError> {
        Self::from_inner(ProcessedAxisInner {
            label: self.0.label.clone(),
            role: self.0.role,
            domain,
            unit,
            quantity: self.0.quantity,
            points,
            coordinates,
            component_basis,
            nucleus: self.0.nucleus.clone(),
            spectral_width_hz,
            frequency_evidence: self.0.frequency_evidence,
            spectrum_reference: self.0.spectrum_reference.clone(),
        })
    }

    pub(crate) fn from_raw_axis(
        raw: &crate::raw::RawAxis,
        component_basis: ComponentBasis,
    ) -> Result<Self, ProcessedValidationError> {
        Self::from_inner(ProcessedAxisInner {
            label: raw.label().map(str::to_owned),
            role: raw.role(),
            domain: raw.domain(),
            unit: raw.unit(),
            quantity: raw.quantity(),
            points: raw.points(),
            coordinates: raw.coordinates().clone(),
            component_basis,
            nucleus: raw.nucleus().map(str::to_owned),
            spectral_width_hz: raw.spectral_width_hz(),
            frequency_evidence: raw.frequency_evidence(),
            spectrum_reference: None,
        })
    }

    pub(crate) fn backing_bytes(
        label: Option<&str>,
        nucleus: Option<&str>,
        coordinates: &AxisCoordinates,
    ) -> Option<usize> {
        let coordinates = match coordinates {
            AxisCoordinates::Explicit(values) => {
                values.len().checked_mul(std::mem::size_of::<f64>())?
            }
            _ => 0,
        };
        [
            std::mem::size_of::<ProcessedAxisInner>(),
            2 * std::mem::size_of::<usize>(),
            std::mem::align_of::<ProcessedAxisInner>().max(std::mem::align_of::<usize>()),
            label.map_or(0, str::len),
            nucleus.map_or(0, str::len),
            coordinates,
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .filter(|&bytes| bytes <= isize::MAX as usize)
    }

    /// Sets a non-empty axis label.
    pub fn with_label(mut self, label: Option<String>) -> Self {
        std::sync::Arc::make_mut(&mut self.0).label = label
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        self
    }
    /// Sets the physical quantity represented by an array-parameter axis.
    pub fn with_quantity(
        mut self,
        quantity: Option<AxisQuantity>,
    ) -> Result<Self, ProcessedValidationError> {
        std::sync::Arc::make_mut(&mut self.0).quantity = quantity;
        self.validate()?;
        Ok(self)
    }

    /// Sets an optional portable nucleus label.
    ///
    /// Isotope notation using any periodic-table element symbol or English name
    /// is normalized to mass number followed by element symbol, such as `1H`
    /// and `19F`. Verified vendor aliases are also normalized. Empty values and
    /// `off` become absent; unknown non-empty labels remain opaque.
    pub fn with_nucleus(
        mut self,
        nucleus: Option<String>,
    ) -> Result<Self, ProcessedValidationError> {
        std::sync::Arc::make_mut(&mut self.0).nucleus =
            crate::internal::nucleus::normalize(nucleus);
        self.validate()?;
        Ok(self)
    }

    /// Sets checked frequency evidence without converting coordinates.
    pub fn with_frequency_evidence(
        mut self,
        evidence: Option<FrequencyEvidence>,
    ) -> Result<Self, ProcessedValidationError> {
        if let Some(value) = evidence {
            value.validate()?;
        }
        std::sync::Arc::make_mut(&mut self.0).frequency_evidence = evidence;
        self.validate()?;
        Ok(self)
    }

    /// Sets finite positive source-declared or trusted-lineage bandwidth in Hz.
    ///
    /// Coordinate endpoint span is a separate quantity exposed by
    /// [`Self::coordinate_span`].
    pub fn with_spectral_width_hz(
        mut self,
        spectral_width_hz: Option<f64>,
    ) -> Result<Self, ProcessedValidationError> {
        std::sync::Arc::make_mut(&mut self.0).spectral_width_hz = spectral_width_hz;
        self.validate()?;
        Ok(self)
    }

    /// Returns the scientific role.
    pub fn role(&self) -> AxisRole {
        self.0.role
    }

    /// Returns the mathematical domain.
    pub fn domain(&self) -> AxisDomain {
        self.0.domain
    }

    /// Returns the physical coordinate unit, when known.
    pub fn unit(&self) -> Option<AxisUnit> {
        self.0.unit
    }
    /// Returns the physical parameter quantity when established.
    pub fn quantity(&self) -> Option<AxisQuantity> {
        self.0.quantity
    }

    /// Returns the logical point count.
    pub fn points(&self) -> usize {
        self.0.points
    }

    /// Returns the logical coordinates.
    pub fn coordinates(&self) -> &AxisCoordinates {
        &self.0.coordinates
    }

    /// Returns one coordinate; unknown calibration and out-of-range indices differ.
    pub fn coordinate(&self, index: usize) -> Result<f64, crate::axis::CoordinateError> {
        crate::axis::coordinate(self.coordinates(), self.points(), index)
    }

    /// Iterates coordinates using the same fused evaluation as indexed access.
    pub fn coordinate_iter(
        &self,
    ) -> Result<crate::axis::CoordinateIter<'_>, crate::axis::CoordinateError> {
        crate::axis::CoordinateIter::new(self.coordinates(), self.points())
    }

    /// Returns the direction derived from the coordinates.
    pub fn direction(&self) -> AxisDirection {
        direction(&self.0.coordinates, self.0.points)
    }

    /// Returns the component count at every logical coordinate.
    pub fn component_count(&self) -> usize {
        self.0.component_basis.component_count()
    }

    /// Returns the scientific component basis.
    pub fn component_basis(&self) -> &ComponentBasis {
        &self.0.component_basis
    }

    /// Returns the label, when known.
    pub fn label(&self) -> Option<&str> {
        self.0.label.as_deref()
    }

    /// Returns the portable nucleus label when present.
    ///
    /// Known isotopes use mass-number-first ASCII notation. Periodic-table
    /// element symbols and English names are recognized in mass-first or
    /// mass-last form. Unknown non-empty source labels are returned opaquely
    /// and are never inferred from frequency.
    pub fn nucleus(&self) -> Option<&str> {
        self.0.nucleus.as_deref()
    }

    /// Returns frequency evidence, when present.
    pub fn frequency_evidence(&self) -> Option<FrequencyEvidence> {
        self.0.frequency_evidence
    }

    /// Reader-established spectrum reference, independent of acquisition observe.
    /// For effective raw-lineage calibration use ProcessedDataset::axis_evidence.
    pub fn spectrum_reference(&self) -> Option<&crate::processed::SpectrumReference> {
        self.0.spectrum_reference.as_ref()
    }

    pub(crate) fn with_spectrum_reference(
        mut self,
        reference: crate::processed::SpectrumReference,
    ) -> Self {
        std::sync::Arc::make_mut(&mut self.0).spectrum_reference = Some(reference);
        self
    }

    /// Returns source-declared or lineage-preserved spectral-window width in Hz.
    ///
    /// This is acquisition/processing bandwidth evidence, not the distance
    /// between the first and last sampled coordinates. Use
    /// [`Self::coordinate_span`] for the latter.
    pub fn spectral_width_hz(&self) -> Option<f64> {
        self.0.spectral_width_hz
    }

    /// Returns the absolute distance between first and last sampled coordinates.
    ///
    /// Unknown coordinates and single-point axes return `None`. The coordinate
    /// unit is [`Self::unit`]; this value is independent of spectral width.
    pub fn coordinate_span(&self) -> Option<f64> {
        coordinate_span(&self.0.coordinates, self.0.points)
    }

    /// Rechecks the axis contract and component truth table.
    pub fn validate(&self) -> Result<(), ProcessedValidationError> {
        validate_axis(
            self.0.domain,
            self.0.unit,
            self.0.points,
            &self.0.coordinates,
        )?;
        validate_quantity(self.0.role, self.0.domain, self.0.unit, self.0.quantity)?;
        if self
            .0
            .spectral_width_hz
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(ProcessedValidationError::InvalidAxisEvidence);
        }
        if let Some(evidence) = self.0.frequency_evidence {
            evidence.validate()?;
        }
        if self.0.spectrum_reference.as_ref().is_some_and(|r| {
            !r.reference_frequency_mhz().is_finite() || r.reference_frequency_mhz() <= 0.0
        }) {
            return Err(ProcessedValidationError::InvalidAxisEvidence);
        }
        if self.0.domain == AxisDomain::Parameter
            && (self.0.nucleus.is_some()
                || self.0.spectral_width_hz.is_some()
                || self.0.frequency_evidence.is_some()
                || self.0.spectrum_reference.is_some())
        {
            return Err(ProcessedValidationError::SignalEvidenceOnParameterAxis);
        }
        if (self.0.spectral_width_hz.is_some()
            || self.0.frequency_evidence.is_some()
            || self.0.spectrum_reference.is_some())
            && (!self.0.role.is_signal()
                || !matches!(
                    self.0.domain,
                    AxisDomain::Time | AxisDomain::Frequency | AxisDomain::Unknown
                ))
        {
            return Err(ProcessedValidationError::InvalidAxisEvidence);
        }
        let valid = match &self.0.component_basis {
            ComponentBasis::Scalar => true,
            ComponentBasis::Cartesian | ComponentBasis::SharedComplex { .. } => {
                matches!(self.0.domain, AxisDomain::Time | AxisDomain::Frequency)
                    && self.0.role.is_signal()
            }
            ComponentBasis::Encoded(_) => {
                matches!(self.0.domain, AxisDomain::Time | AxisDomain::Unknown)
                    && self.0.role == AxisRole::IndirectAcquisition
            }
        };
        if !valid {
            return Err(ProcessedValidationError::InvalidComponentBasis);
        }
        Ok(())
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static AXIS_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedAxis {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&std::sync::Arc<ProcessedAxisInner>,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (std::sync::Arc<ProcessedAxisInner>,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedAxisInner {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Option<String>,
        &AxisRole,
        &AxisDomain,
        &Option<AxisUnit>,
        &Option<AxisQuantity>,
        &usize,
        &AxisCoordinates,
        &ComponentBasis,
        &Option<String>,
        &Option<f64>,
        &Option<FrequencyEvidence>,
        &Option<crate::processed::SpectrumReference>,
    ) {
        (
            &self.label,
            &self.role,
            &self.domain,
            &self.unit,
            &self.quantity,
            &self.points,
            &self.coordinates,
            &self.component_basis,
            &self.nucleus,
            &self.spectral_width_hz,
            &self.frequency_evidence,
            &self.spectrum_reference,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Option<String>,
            AxisRole,
            AxisDomain,
            Option<AxisUnit>,
            Option<AxisQuantity>,
            usize,
            AxisCoordinates,
            ComponentBasis,
            Option<String>,
            Option<f64>,
            Option<FrequencyEvidence>,
            Option<crate::processed::SpectrumReference>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            label,
            role,
            domain,
            unit,
            quantity,
            points,
            coordinates,
            component_basis,
            nucleus,
            spectral_width_hz,
            frequency_evidence,
            spectrum_reference,
        ) = parts;
        let value = Self {
            label,
            role,
            domain,
            unit,
            quantity,
            points,
            coordinates,
            component_basis,
            nucleus,
            spectral_width_hz,
            frequency_evidence,
            spectrum_reference,
        };

        Ok(value)
    }
}
