use super::*;

/// Canonical storage order.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageOrder {
    /// Row-major storage with the last, direct axis varying fastest.
    RowMajorDirectFastest,
}
/// Immutable canonical relationship between logical coordinates and raw lanes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawLayout {
    pub(super) logical_shape: Box<[usize]>,
    pub(super) lane_counts: Box<[usize]>,
    pub(super) absolute_origin: Box<[usize]>,
    pub(super) storage_order: StorageOrder,
}

impl RawLayout {
    pub(super) fn from_axes(axes: &[RawAxis]) -> Result<Self, ValidationError> {
        Self::from_parts(
            axes.iter().map(RawAxis::points).collect(),
            axes.iter().map(RawAxis::component_lanes).collect(),
        )
    }

    pub(crate) fn from_parts(
        logical_shape: Vec<usize>,
        lane_counts: Vec<usize>,
    ) -> Result<Self, ValidationError> {
        let origin = vec![0; logical_shape.len()];
        Self::from_region(logical_shape, lane_counts, origin)
    }

    pub(crate) fn from_region(
        logical_shape: Vec<usize>,
        lane_counts: Vec<usize>,
        absolute_origin: Vec<usize>,
    ) -> Result<Self, ValidationError> {
        Self::validate_parts(&logical_shape, &lane_counts, &absolute_origin)?;
        Ok(Self {
            logical_shape: logical_shape.into(),
            lane_counts: lane_counts.into(),
            absolute_origin: absolute_origin.into(),
            storage_order: StorageOrder::RowMajorDirectFastest,
        })
    }

    pub(super) fn validate_parts(
        logical_shape: &[usize],
        lane_counts: &[usize],
        absolute_origin: &[usize],
    ) -> Result<(), ValidationError> {
        if logical_shape.is_empty() || logical_shape.contains(&0) {
            return Err(ValidationError::ZeroAxis);
        }
        if logical_shape.len() != lane_counts.len()
            || logical_shape.len() != absolute_origin.len()
            || lane_counts.contains(&0)
        {
            return Err(ValidationError::ComponentRankMismatch);
        }
        for ((&points, &lanes), &origin) in
            logical_shape.iter().zip(lane_counts).zip(absolute_origin)
        {
            points
                .checked_mul(lanes)
                .ok_or(ValidationError::SizeOverflow)?;
            origin
                .checked_add(points)
                .ok_or(ValidationError::SizeOverflow)?;
        }
        checked_product(logical_shape).ok_or(ValidationError::SizeOverflow)?;
        checked_product(lane_counts).ok_or(ValidationError::SizeOverflow)?;
        Ok(())
    }

    /// Returns logical point counts from slowest to fastest axis.
    pub fn logical_shape(&self) -> &[usize] {
        &self.logical_shape
    }

    /// Returns per-axis raw lane counts derived from axis semantics.
    pub fn lane_counts(&self) -> &[usize] {
        &self.lane_counts
    }

    /// Returns the absolute full-grid coordinate represented by local zero.
    pub fn absolute_origin(&self) -> &[usize] {
        &self.absolute_origin
    }

    /// Returns the frozen canonical storage order.
    pub fn storage_order(&self) -> StorageOrder {
        self.storage_order
    }

    /// Returns `logical[j] * lane_count[j]` for every expanded axis.
    pub fn storage_shape(&self) -> Result<Vec<usize>, ValidationError> {
        self.logical_shape
            .iter()
            .zip(&self.lane_counts)
            .map(|(&points, &lanes)| {
                points
                    .checked_mul(lanes)
                    .ok_or(ValidationError::SizeOverflow)
            })
            .collect()
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawLayout {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (&Box<[usize]>, &Box<[usize]>, &Box<[usize]>, &StorageOrder) {
        (
            &self.logical_shape,
            &self.lane_counts,
            &self.absolute_origin,
            &self.storage_order,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Box<[usize]>, Box<[usize]>, Box<[usize]>, StorageOrder),
    ) -> Result<Self, crate::internal::ModelError> {
        let (logical_shape, lane_counts, absolute_origin, storage_order) = parts;
        let value = Self {
            logical_shape,
            lane_counts,
            absolute_origin,
            storage_order,
        };

        Self::validate_parts(
            &value.logical_shape,
            &value.lane_counts,
            &value.absolute_origin,
        )
        .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
