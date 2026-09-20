use super::*;

/// Ordered processed axes, slowest to fastest logical axis.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessedDescriptor {
    axes: Vec<ProcessedAxis>,
}

impl ProcessedDescriptor {
    /// Creates a nonempty N-dimensional descriptor. Algorithm rank limits are
    /// checked separately by processing preflight.
    pub fn new(axes: Vec<ProcessedAxis>) -> Result<Self, ProcessedValidationError> {
        let value = Self { axes };
        value.validate()?;
        Ok(value)
    }

    /// Returns axes from slowest to fastest.
    pub fn axes(&self) -> &[ProcessedAxis] {
        &self.axes
    }

    pub(crate) fn same_axes_except_labels(&self, other: &Self) -> bool {
        self.axes.len() == other.axes.len()
            && self.axes.iter().zip(&other.axes).all(|(left, right)| {
                // Exhaustive field binding makes adding an axis field require an
                // explicit decision here; only the display label is excluded.
                let ProcessedAxisInner {
                    label: _,
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
                } = &*left.0;
                (
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
                ) == (
                    &right.0.role,
                    &right.0.domain,
                    &right.0.unit,
                    &right.0.quantity,
                    &right.0.points,
                    &right.0.coordinates,
                    &right.0.component_basis,
                    &right.0.nucleus,
                    &right.0.spectral_width_hz,
                    &right.0.frequency_evidence,
                    &right.0.spectrum_reference,
                )
            })
    }

    /// Returns logical point counts.
    pub fn logical_shape(&self) -> Vec<usize> {
        self.axes.iter().map(ProcessedAxis::points).collect()
    }

    /// Returns per-axis component counts.
    pub fn component_counts(&self) -> Vec<usize> {
        self.axes
            .iter()
            .map(ProcessedAxis::component_count)
            .collect()
    }

    /// Rechecks axis contracts, unique direct role, and expanded shape arithmetic.
    pub fn validate(&self) -> Result<(), ProcessedValidationError> {
        validate_rank(self.axes.len())?;
        for axis in &self.axes {
            axis.validate()?;
        }
        for (index, selected) in self.axes.iter().enumerate() {
            if let ComponentBasis::SharedComplex { axis, .. } = selected.component_basis() {
                if axis.index() == index
                    || !self.axes.get(axis.index()).is_some_and(|owner| {
                        matches!(owner.component_basis(), ComponentBasis::Cartesian)
                    })
                {
                    return Err(ProcessedValidationError::InvalidComponentBasis);
                }
            }
        }
        if self
            .axes
            .iter()
            .filter(|axis| axis.role() == AxisRole::DirectAcquisition)
            .count()
            > 1
        {
            return Err(ProcessedValidationError::MultipleDirectAxes);
        }
        self.axes.iter().try_fold(1usize, |total, axis| {
            total
                .checked_mul(axis.points())
                .and_then(|value| value.checked_mul(axis.component_count()))
                .ok_or(ProcessedValidationError::SizeOverflow)
        })?;
        Ok(())
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedDescriptor {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Vec<ProcessedAxis>,) {
        (&self.axes,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Vec<ProcessedAxis>,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (axes,) = parts;
        let value = Self { axes };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
