use super::*;

/// Description and metadata for an acquisition.
#[derive(Clone, Debug, PartialEq)]
pub struct RawDescriptor {
    pub(super) axes: Vec<RawAxis>,
    pub(super) acquisition: RawMetadata,
    pub(super) layout: RawLayout,
    pub(super) layout_evidence: crate::acquisition::NormalizationEvidence,
}
impl RawDescriptor {
    /// Creates a checked descriptor whose last axis is the direct axis.
    pub(crate) fn new(
        axes: Vec<RawAxis>,
        acquisition: RawMetadata,
    ) -> Result<Self, ValidationError> {
        let evidence = crate::acquisition::NormalizationEvidence::user_constructed(vec![
            crate::acquisition::NormalizationFact::TraceMappingBijection,
        ])?;
        Self::new_resolved(axes, acquisition, evidence)
    }

    pub(crate) fn new_resolved(
        axes: Vec<RawAxis>,
        acquisition: RawMetadata,
        layout_evidence: crate::acquisition::NormalizationEvidence,
    ) -> Result<Self, ValidationError> {
        let layout = RawLayout::from_axes(&axes)?;
        let value = Self {
            axes,
            acquisition,
            layout,
            layout_evidence,
        };
        value.validate()?;
        Ok(value)
    }
    /// Returns axes from slowest indirect to fastest direct.
    pub fn axes(&self) -> &[RawAxis] {
        &self.axes
    }
    /// Returns portable acquisition metadata.
    pub fn acquisition(&self) -> &RawMetadata {
        &self.acquisition
    }
    /// Returns the canonical storage order.
    pub fn storage_order(&self) -> StorageOrder {
        self.layout.storage_order()
    }
    /// Returns the immutable canonical raw layout.
    pub fn layout(&self) -> &RawLayout {
        &self.layout
    }
    /// Returns evidence for physical trace, coordinate, and lane mapping.
    pub fn layout_evidence(&self) -> &crate::acquisition::NormalizationEvidence {
        &self.layout_evidence
    }
    /// Returns logical point counts without quadrature component lanes.
    pub fn logical_shape(&self) -> Vec<usize> {
        self.layout.logical_shape().to_vec()
    }
    /// Returns the component-lane count for each logical axis.
    pub fn component_lanes(&self) -> Vec<usize> {
        self.layout.lane_counts().to_vec()
    }
    /// Validates all axes, acquisition metadata, and axis ordering.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.axes.is_empty() {
            return Err(ValidationError::EmptyAxes);
        }
        for axis in &self.axes {
            axis.validate()?;
        }
        if self.axes.iter().any(|a| {
            matches!(
                a.kind(),
                crate::acquisition::RawAxisKind::Indirect(
                    crate::acquisition::IndirectComponents::SharedComplex { .. }
                )
            )
        }) && !self.axes.last().is_some_and(|a| {
            matches!(
                a.kind(),
                crate::acquisition::RawAxisKind::Direct(crate::acquisition::DirectSamples::Complex)
            )
        }) {
            return Err(ValidationError::DescriptorDataMismatch);
        }
        // Check the derived extents without allocating another three layout arrays.
        let mut points_product = 1usize;
        let mut lanes_product = 1usize;
        for axis in &self.axes {
            axis.points()
                .checked_mul(axis.component_lanes())
                .ok_or(ValidationError::SizeOverflow)?;
            points_product = points_product
                .checked_mul(axis.points())
                .ok_or(ValidationError::SizeOverflow)?;
            lanes_product = lanes_product
                .checked_mul(axis.component_lanes())
                .ok_or(ValidationError::SizeOverflow)?;
        }
        if !self
            .axes
            .iter()
            .map(RawAxis::points)
            .eq(self.layout.logical_shape.iter().copied())
            || !self.axes.iter().map(RawAxis::component_lanes).eq(self
                .layout
                .lane_counts
                .iter()
                .copied())
            || self.layout.storage_order != StorageOrder::RowMajorDirectFastest
        {
            return Err(ValidationError::DescriptorDataMismatch);
        }
        self.acquisition.validate()?;
        if let Some(diffusion) = self.acquisition.diffusion() {
            let axis = self
                .axes
                .get(diffusion.gradient_axis())
                .ok_or(ValidationError::DiffusionGradientAxisMismatch)?;
            if axis.quantity() != Some(AxisQuantity::MagneticFieldGradientStrength) {
                return Err(ValidationError::DiffusionGradientAxisMismatch);
            }
        }
        if self
            .axes
            .last()
            .is_some_and(|axis| axis.role() != AxisRole::DirectAcquisition)
            || self.axes[..self.axes.len() - 1]
                .iter()
                .any(|axis| axis.role() == AxisRole::DirectAcquisition)
        {
            return Err(ValidationError::DirectAxisNotFastest);
        }
        Ok(())
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawDescriptor {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Vec<RawAxis>,
        &RawMetadata,
        &RawLayout,
        &crate::acquisition::NormalizationEvidence,
    ) {
        (
            &self.axes,
            &self.acquisition,
            &self.layout,
            &self.layout_evidence,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Vec<RawAxis>,
            RawMetadata,
            RawLayout,
            crate::acquisition::NormalizationEvidence,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (axes, acquisition, layout, layout_evidence) = parts;
        let value = Self {
            axes,
            acquisition,
            layout,
            layout_evidence,
        };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
