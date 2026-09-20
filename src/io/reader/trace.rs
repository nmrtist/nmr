use crate::ExecutionContext;
use crate::ReadError;
use crate::execution::ExecutionStage;
use crate::raw::Trace;

use super::Reader;
use super::storage::unflatten;

impl Reader {
    /// Uses default synchronous execution controls.
    pub fn read_observation(
        &self,
        ordinal: crate::raw::ObservationOrdinal,
    ) -> Result<Trace, ReadError> {
        self.read_observation_with_context(&mut ExecutionContext::default(), ordinal)
    }

    /// Uses default synchronous execution controls.
    pub fn read_trace(&self, coordinate: &[usize]) -> Result<Trace, ReadError> {
        self.read_trace_with_context(&mut ExecutionContext::default(), coordinate)
    }

    /// Reads one logical trace while retaining every component lane.
    ///
    /// A coordinate acquired more than once is ambiguous; materialize the
    /// dataset to retain each acquisition-ordered sparse observation.
    pub fn read_trace_with_context(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Trace, ReadError> {
        control.begin(ExecutionStage::Reading, None, None)?;
        self.read_trace_inner(control, coordinate)
            .map_err(|error| self.with_format(error))
    }

    pub(super) fn read_trace_inner(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Trace, ReadError> {
        crate::raw::model::validate_trace_coordinate(
            self.descriptor.layout().logical_shape(),
            coordinate,
        )?;
        if self.sampling.as_ref().is_some_and(|schedule| {
            schedule
                .coordinates()
                .iter()
                .filter(|candidate| candidate.as_slice() == coordinate)
                .take(2)
                .count()
                > 1
        }) {
            return Err(crate::AccessError::AmbiguousObservation {
                coordinate: coordinate.to_vec(),
            }
            .into());
        }
        self.check_numeric_working(0, 0, self.trace_metadata_bytes(false)?)?;
        let samples = self.source.read_trace_controlled(control, coordinate)?;
        Trace::new(
            coordinate,
            None,
            self.direct_points(),
            self.descriptor.layout().lane_counts(),
            samples,
        )
    }

    /// Reads one acquisition-ordered observation exactly, preserving duplicates.
    pub fn read_observation_with_context(
        &self,
        control: &mut ExecutionContext<'_>,
        ordinal: crate::raw::ObservationOrdinal,
    ) -> Result<Trace, ReadError> {
        control.begin(ExecutionStage::Reading, None, None)?;
        self.check_numeric_working(0, 0, self.trace_metadata_bytes(true)?)
            .map_err(|error| self.with_format(error))?;
        let coordinate = if let Some(schedule) = &self.sampling {
            schedule
                .coordinates()
                .get(ordinal.get())
                .ok_or(crate::AccessError::ObservationUnavailable { ordinal })?
                .as_slice()
                .to_vec()
        } else {
            let indirect =
                &self.descriptor.layout().logical_shape()[..self.descriptor.axes().len() - 1];
            unflatten(indirect, ordinal.get())?
        };
        let samples = self
            .source
            .read_scheduled_trace_controlled(control, ordinal.get(), &coordinate)
            .map_err(|error| self.with_format(error))?;
        Trace::new(
            &coordinate,
            Some(ordinal),
            self.direct_points(),
            self.descriptor.layout().lane_counts(),
            samples,
        )
        .map_err(|error| self.with_format(error))
    }
}
