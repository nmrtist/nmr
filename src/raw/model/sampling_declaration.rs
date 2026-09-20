//! Explicit sampling declarations and shared acquisition/restore validation.
use super::{SamplingCoordinate, SamplingSchedule, ValidationError};
use crate::execution::ExecutionError;
use std::sync::Arc;

/// Index convention of a caller-supplied sampling table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamplingIndexBase {
    /// The first grid coordinate is zero.
    Zero,
    /// The first grid coordinate is one.
    One,
}

/// A complete sampling assertion, retaining original indices and their source.
/// This cannot override vendor evidence or change physical trace/lane order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SamplingDeclaration {
    assertion: crate::acquisition::AssertionId,
    source: String,
    grid: Vec<usize>,
    indices: Vec<Vec<usize>>,
    index_base: SamplingIndexBase,
    indirect_lanes: Vec<usize>,
}
impl SamplingDeclaration {
    /// Creates a declaration for validation during reading. All axes use current
    /// slowest-to-fastest indirect order; each row represents one observation,
    /// including all declared component lanes. Duplicate rows remain distinct.
    pub fn new(
        assertion: crate::acquisition::AssertionId,
        source: impl Into<String>,
        grid: Vec<usize>,
        indices: Vec<Vec<usize>>,
        index_base: SamplingIndexBase,
        indirect_lanes: Vec<usize>,
    ) -> Self {
        Self {
            assertion,
            source: source.into(),
            grid,
            indices,
            index_base,
            indirect_lanes,
        }
    }
    /// Caller identity, retained independently of the vendor source.
    pub fn assertion(&self) -> &crate::acquisition::AssertionId {
        &self.assertion
    }
    /// Caller-supplied origin description (for example, a schedule export name).
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Asserted full indirect grid.
    pub fn grid(&self) -> &[usize] {
        &self.grid
    }
    /// Original indices in observation order and in the declared index base.
    pub fn indices(&self) -> &[Vec<usize>] {
        &self.indices
    }
    /// Index base of the original indices.
    pub fn index_base(&self) -> SamplingIndexBase {
        self.index_base
    }
    /// Asserted component lane counts for the indirect axes.
    pub fn indirect_lanes(&self) -> &[usize] {
        &self.indirect_lanes
    }

    pub(crate) fn resolve(
        self: &Arc<Self>,
        grid: &[usize],
        lanes: &[usize],
        observations: usize,
        vendor: Option<&SamplingSchedule>,
        cancellation: Option<&crate::CancellationToken>,
    ) -> Result<SamplingSchedule, SamplingDeclarationError> {
        if self.source.trim().is_empty()
            || grid.is_empty()
            || grid.contains(&0)
            || self.grid != grid
            || self.indirect_lanes != lanes
            || lanes.contains(&0)
            || lanes.len() != grid.len()
            || self.indices.len() != observations
            || observations == 0
        {
            return Err(SamplingDeclarationError::Mismatch);
        }
        let base = usize::from(self.index_base == SamplingIndexBase::One);
        let mut coordinates = Vec::with_capacity(observations);
        for row in &self.indices {
            if let Some(token) = cancellation {
                token.check().map_err(SamplingDeclarationError::Execution)?;
            }
            if row.len() != grid.len() {
                return Err(SamplingDeclarationError::Mismatch);
            }
            let values = row
                .iter()
                .zip(grid)
                .map(|(&index, &size)| {
                    index
                        .checked_sub(base)
                        .filter(|&index| index < size)
                        .ok_or(SamplingDeclarationError::IndexOutOfBounds)
                })
                .collect::<Result<Vec<_>, _>>()?;
            coordinates.push(SamplingCoordinate::new(values));
        }
        let mut schedule = SamplingSchedule::new(grid.to_vec(), coordinates)
            .map_err(SamplingDeclarationError::Model)?;
        if vendor.is_some_and(|s| {
            s.grid() != schedule.grid() || s.coordinates() != schedule.coordinates()
        }) {
            return Err(SamplingDeclarationError::ScheduleConflict);
        }
        schedule.set_declaration(Arc::clone(self));
        Ok(schedule)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SamplingDeclaration {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &crate::acquisition::AssertionId,
        &String,
        &Vec<usize>,
        &Vec<Vec<usize>>,
        &SamplingIndexBase,
        &Vec<usize>,
    ) {
        (
            &self.assertion,
            &self.source,
            &self.grid,
            &self.indices,
            &self.index_base,
            &self.indirect_lanes,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            crate::acquisition::AssertionId,
            String,
            Vec<usize>,
            Vec<Vec<usize>>,
            SamplingIndexBase,
            Vec<usize>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (assertion, source, grid, indices, index_base, indirect_lanes) = parts;
        let value = Self {
            assertion,
            source,
            grid,
            indices,
            index_base,
            indirect_lanes,
        };

        Ok(value)
    }
}

/// Shared validation failures; callers own their boundary-specific presentation.
#[derive(Debug)]
pub(crate) enum SamplingDeclarationError {
    Mismatch,
    IndexOutOfBounds,
    ScheduleConflict,
    Model(ValidationError),
    Execution(ExecutionError),
}
