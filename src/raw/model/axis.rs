use super::*;

/// One logical acquisition axis, ordered slowest indirect to fastest direct.
#[derive(Clone, Debug, PartialEq)]
pub struct RawAxis {
    pub(super) label: Option<String>,
    pub(super) kind: RawAxisKind,
    pub(super) domain: AxisDomain,
    pub(super) unit: Option<AxisUnit>,
    pub(super) quantity: Option<AxisQuantity>,
    pub(super) points: usize,
    pub(super) coordinates: AxisCoordinates,
    pub(super) nucleus: Option<String>,
    pub(super) spectral_width_hz: Option<f64>,
    pub(super) frequency_evidence: Option<FrequencyEvidence>,
    pub(super) chemical_shift_reference: Option<ChemicalShiftReference>,
    pub(super) group_delay: GroupDelayState,
}
impl RawAxis {
    /// Creates and validates a logical acquisition axis.
    pub fn new(
        kind: RawAxisKind,
        domain: AxisDomain,
        unit: Option<AxisUnit>,
        points: usize,
        coordinates: AxisCoordinates,
    ) -> Result<Self, ValidationError> {
        let axis = Self {
            label: None,
            kind,
            domain,
            unit,
            quantity: None,
            points,
            coordinates,
            nucleus: None,
            spectral_width_hz: None,
            frequency_evidence: None,
            chemical_shift_reference: None,
            group_delay: GroupDelayState::NotApplicable,
        };
        axis.validate()?;
        Ok(axis)
    }
    /// Sets a portable or vendor parameter label for the axis.
    pub fn with_label(mut self, label: Option<String>) -> Self {
        self.label = normalize_text(label);
        self
    }
    /// Sets the physical quantity represented by an array-parameter axis.
    pub fn with_quantity(
        mut self,
        quantity: Option<AxisQuantity>,
    ) -> Result<Self, ValidationError> {
        self.quantity = quantity;
        self.validate()?;
        Ok(self)
    }
    /// Sets an optional portable nucleus label.
    ///
    /// Isotope notation using any periodic-table element symbol or English name
    /// is normalized to mass number followed by element symbol, such as `1H`
    /// and `19F`. Verified vendor aliases are also normalized. Empty values and
    /// `off` become absent; unknown non-empty labels remain opaque.
    pub fn with_nucleus(mut self, nucleus: Option<String>) -> Result<Self, ValidationError> {
        self.nucleus = crate::internal::nucleus::normalize(nucleus);
        self.validate()?;
        Ok(self)
    }
    /// Sets finite positive source-declared spectral-window width in Hz.
    ///
    /// Coordinate endpoint span is a separate quantity exposed by
    /// [`Self::coordinate_span`].
    pub fn with_spectral_width_hz(mut self, value: Option<f64>) -> Result<Self, ValidationError> {
        self.spectral_width_hz = value;
        self.validate()?;
        Ok(self)
    }
    /// Sets the distinct observe, transmitter, and reference frequencies.
    pub fn with_frequency_evidence(
        mut self,
        value: Option<FrequencyEvidence>,
    ) -> Result<Self, ValidationError> {
        self.frequency_evidence = value;
        self.validate()?;
        Ok(self)
    }
    /// Sets an optional canonical chemical-shift reference.
    pub fn with_chemical_shift_reference(
        mut self,
        value: Option<ChemicalShiftReference>,
    ) -> Result<Self, ValidationError> {
        self.chemical_shift_reference = value;
        self.validate()?;
        Ok(self)
    }
    /// Sets the raw direct-axis group-delay state.
    pub fn with_group_delay(mut self, value: GroupDelayState) -> Result<Self, ValidationError> {
        self.group_delay = value;
        self.validate()?;
        Ok(self)
    }
    /// Returns the scientific role, independent of unit and domain.
    pub fn role(&self) -> AxisRole {
        match &self.kind {
            RawAxisKind::Direct(_) => AxisRole::DirectAcquisition,
            RawAxisKind::Indirect(_) => AxisRole::IndirectAcquisition,
            RawAxisKind::Parameter => AxisRole::ArrayParameter,
        }
    }
    /// Returns the axis or array-parameter label when known.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    /// Returns the coordinate quantity.
    pub fn domain(&self) -> AxisDomain {
        self.domain
    }
    /// Returns the physical coordinate unit when known.
    pub fn unit(&self) -> Option<AxisUnit> {
        self.unit
    }
    /// Returns the physical parameter quantity when established.
    pub fn quantity(&self) -> Option<AxisQuantity> {
        self.quantity
    }
    /// Returns the number of logical coordinates.
    pub fn points(&self) -> usize {
        self.points
    }
    /// Returns coordinates for logical points.
    pub fn coordinates(&self) -> &AxisCoordinates {
        &self.coordinates
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

    /// Returns the canonical raw-axis kind.
    pub fn kind(&self) -> &RawAxisKind {
        &self.kind
    }
    /// Returns the number of stored component lanes per logical coordinate.
    pub fn component_lanes(&self) -> usize {
        self.kind.lane_count()
    }
    /// Returns the portable nucleus label when present.
    ///
    /// Known isotopes use mass-number-first ASCII notation. Periodic-table
    /// element symbols and English names are recognized in mass-first or
    /// mass-last form. Unknown non-empty source labels are returned opaquely
    /// and are never inferred from frequency.
    pub fn nucleus(&self) -> Option<&str> {
        self.nucleus.as_deref()
    }
    /// Returns source-declared or lineage-preserved spectral-window width in Hz.
    ///
    /// This is acquisition/processing bandwidth evidence, not the distance
    /// between the first and last sampled coordinates. Use
    /// [`Self::coordinate_span`] for the latter.
    pub fn spectral_width_hz(&self) -> Option<f64> {
        self.spectral_width_hz
    }
    /// Returns the absolute distance between first and last sampled coordinates.
    ///
    /// Unknown coordinates and single-point axes return `None`. The coordinate
    /// unit is [`Self::unit`]; this value is independent of spectral width.
    pub fn coordinate_span(&self) -> Option<f64> {
        coordinate_span(&self.coordinates, self.points)
    }
    /// Returns the frequency-reference quantities when any are known.
    pub fn frequency_evidence(&self) -> Option<FrequencyEvidence> {
        self.frequency_evidence
    }
    /// Returns the direction derived from coordinates.
    pub fn direction(&self) -> AxisDirection {
        direction(&self.coordinates, self.points)
    }
    /// Returns the canonical chemical-shift reference when established.
    pub fn chemical_shift_reference(&self) -> Option<&ChemicalShiftReference> {
        self.chemical_shift_reference.as_ref()
    }
    /// Returns the raw group-delay correction state.
    pub fn group_delay(&self) -> &GroupDelayState {
        &self.group_delay
    }
    /// Checks coordinate, role, quadrature, lane, and numeric invariants.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_axis(self.domain, self.unit, self.points, &self.coordinates)?;
        validate_quantity(self.role(), self.domain, self.unit, self.quantity)?;
        let role_domain_matches = match &self.kind {
            RawAxisKind::Direct(_) | RawAxisKind::Indirect(_) => {
                self.domain != AxisDomain::Parameter
            }
            RawAxisKind::Parameter => self.domain == AxisDomain::Parameter,
        };
        if !role_domain_matches {
            return Err(ValidationError::RawRoleDomainMismatch);
        }
        if self
            .spectral_width_hz
            .is_some_and(|v| !v.is_finite() || v <= 0.0)
        {
            return Err(ValidationError::InvalidNumber("axis metadata"));
        }
        if let Some(evidence) = self.frequency_evidence {
            evidence.validate()?;
        }
        if matches!(&self.kind, RawAxisKind::Parameter)
            && (self.nucleus.is_some()
                || self.spectral_width_hz.is_some()
                || self.frequency_evidence.is_some()
                || self.chemical_shift_reference.is_some()
                || !matches!(self.group_delay, GroupDelayState::NotApplicable))
        {
            return Err(ValidationError::SignalEvidenceOnParameterAxis);
        }
        if !matches!(&self.kind, RawAxisKind::Direct(_))
            && !matches!(self.group_delay, GroupDelayState::NotApplicable)
        {
            return Err(ValidationError::GroupDelayOnNonDirectAxis);
        }
        Ok(())
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawAxis {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Option<String>,
        &RawAxisKind,
        &AxisDomain,
        &Option<AxisUnit>,
        &Option<AxisQuantity>,
        &usize,
        &AxisCoordinates,
        &Option<String>,
        &Option<f64>,
        &Option<FrequencyEvidence>,
        &Option<ChemicalShiftReference>,
        &GroupDelayState,
    ) {
        (
            &self.label,
            &self.kind,
            &self.domain,
            &self.unit,
            &self.quantity,
            &self.points,
            &self.coordinates,
            &self.nucleus,
            &self.spectral_width_hz,
            &self.frequency_evidence,
            &self.chemical_shift_reference,
            &self.group_delay,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Option<String>,
            RawAxisKind,
            AxisDomain,
            Option<AxisUnit>,
            Option<AxisQuantity>,
            usize,
            AxisCoordinates,
            Option<String>,
            Option<f64>,
            Option<FrequencyEvidence>,
            Option<ChemicalShiftReference>,
            GroupDelayState,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            label,
            kind,
            domain,
            unit,
            quantity,
            points,
            coordinates,
            nucleus,
            spectral_width_hz,
            frequency_evidence,
            chemical_shift_reference,
            group_delay,
        ) = parts;
        let value = Self {
            label,
            kind,
            domain,
            unit,
            quantity,
            points,
            coordinates,
            nucleus,
            spectral_width_hz,
            frequency_evidence,
            chemical_shift_reference,
            group_delay,
        };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
