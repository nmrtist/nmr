use super::*;

/// Zero-based coordinate in acquisition order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplingCoordinate(pub(super) Vec<usize>);
impl SamplingCoordinate {
    /// Creates a zero-based logical indirect coordinate.
    pub fn new(values: Vec<usize>) -> Self {
        Self(values)
    }
    /// Returns coordinate components from slowest to fastest indirect axis.
    pub fn as_slice(&self) -> &[usize] {
        &self.0
    }
}
/// NUS sampling schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplingSchedule {
    pub(super) grid: Vec<usize>,
    pub(super) coordinates: Vec<SamplingCoordinate>,
    pub(super) repeated_coordinates: bool,
    pub(super) declaration: Option<std::sync::Arc<crate::SamplingDeclaration>>,
}
impl SamplingSchedule {
    /// Creates a non-empty, in-bounds logical sampling schedule.
    ///
    /// Coordinates retain acquisition order and may repeat when the source
    /// acquired more than one observation at the same logical grid point.
    pub fn new(
        grid: Vec<usize>,
        coordinates: Vec<SamplingCoordinate>,
    ) -> Result<Self, ValidationError> {
        if grid.contains(&0) {
            return Err(ValidationError::ZeroAxis);
        }
        if coordinates.is_empty() {
            return Err(ValidationError::EmptySchedule);
        }
        for coordinate in &coordinates {
            if coordinate.0.len() != grid.len() {
                return Err(ValidationError::SamplingRankMismatch);
            }
            if coordinate
                .0
                .iter()
                .enumerate()
                .any(|(axis, &value)| value >= grid[axis])
            {
                return Err(ValidationError::SamplingOutOfBounds);
            }
        }
        // Sort only borrowed references: acquisition order remains unchanged.
        // A single exact-sized index gives the constructor a predictable peak;
        // all later uniqueness queries use the immutable cached result.
        let requested_bytes = coordinates
            .len()
            .checked_mul(std::mem::size_of::<&SamplingCoordinate>())
            .filter(|&bytes| bytes <= isize::MAX as usize)
            .ok_or(ValidationError::SizeOverflow)?;
        let mut sorted = Vec::new();
        sorted
            .try_reserve_exact(coordinates.len())
            .map_err(|_| ValidationError::Allocation { requested_bytes })?;
        sorted.extend(coordinates.iter());
        sorted.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        let repeated_coordinates = sorted.windows(2).any(|pair| pair[0] == pair[1]);
        drop(sorted);
        Ok(Self {
            grid,
            coordinates,
            repeated_coordinates,
            declaration: None,
        })
    }
    /// Returns the full indirect logical grid shape.
    pub fn grid(&self) -> &[usize] {
        &self.grid
    }
    /// Returns sampled coordinates in acquisition order.
    pub fn coordinates(&self) -> &[SamplingCoordinate] {
        &self.coordinates
    }

    /// Returns whether any logical grid point was acquired more than once.
    pub fn has_repeated_coordinates(&self) -> bool {
        self.repeated_coordinates
    }

    /// Explicit caller declaration checked by the reader against vendor evidence.
    pub fn declaration(&self) -> Option<&crate::SamplingDeclaration> {
        self.declaration.as_deref()
    }
    pub(crate) fn set_declaration(&mut self, value: std::sync::Arc<crate::SamplingDeclaration>) {
        self.declaration = Some(value);
    }

    pub(crate) fn is_complete_unique(&self) -> Result<bool, ValidationError> {
        if self.has_repeated_coordinates() {
            return Ok(false);
        }
        Ok(self.coordinates.len()
            == checked_product(&self.grid).ok_or(ValidationError::SizeOverflow)?)
    }
}
/// Samples acquired at one logical indirect coordinate.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservationOrdinal(usize);

impl ObservationOrdinal {
    /// Creates an acquisition-order observation ordinal.
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the zero-based acquisition-order value.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Samples acquired at one logical indirect coordinate.
#[derive(Clone, Debug, PartialEq)]
pub struct SparseTrace {
    pub(super) ordinal: ObservationOrdinal,
    pub(super) coordinate: SamplingCoordinate,
    pub(super) samples: Vec<Complex64>,
}
impl SparseTrace {
    /// Creates a trace that will be checked when inserted into [`RawData`].
    pub fn new(
        ordinal: ObservationOrdinal,
        coordinate: SamplingCoordinate,
        samples: Vec<Complex64>,
    ) -> Self {
        Self {
            ordinal,
            coordinate,
            samples,
        }
    }
    /// Returns the unique acquisition-order observation ordinal.
    pub fn ordinal(&self) -> ObservationOrdinal {
        self.ordinal
    }
    /// Returns the logical indirect coordinate.
    pub fn coordinate(&self) -> &SamplingCoordinate {
        &self.coordinate
    }
    /// Returns component-major samples with direct points fastest per component.
    pub fn samples(&self) -> &[Complex64] {
        &self.samples
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ObservationOrdinal {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&usize,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(parts: (usize,)) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SamplingCoordinate {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Vec<usize>,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Vec<usize>,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

impl SamplingSchedule {
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Vec<usize>,
        &Vec<SamplingCoordinate>,
        &bool,
        &Option<std::sync::Arc<crate::SamplingDeclaration>>,
    ) {
        (
            &self.grid,
            &self.coordinates,
            &self.repeated_coordinates,
            &self.declaration,
        )
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SparseTrace {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (&ObservationOrdinal, &SamplingCoordinate, &Vec<Complex64>) {
        (&self.ordinal, &self.coordinate, &self.samples)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (ObservationOrdinal, SamplingCoordinate, Vec<Complex64>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (ordinal, coordinate, samples) = parts;
        let value = Self {
            ordinal,
            coordinate,
            samples,
        };

        Ok(value)
    }
}
