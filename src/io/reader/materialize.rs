use crate::Complex64;
use crate::ExecutionContext;
use crate::ReadError;
use crate::execution::ExecutionStage;
use crate::raw::RawData;
use crate::raw::RawDataset;
use crate::raw::SparseTrace;

use super::Reader;
use super::storage::{
    enforce_sample_limit, product, reserve_complex_samples, storage_shape, unflatten,
};

impl Reader {
    /// Uses default synchronous execution controls.
    pub fn into_dataset(self) -> Result<RawDataset, ReadError> {
        self.into_dataset_with_context(&mut ExecutionContext::default())
    }

    /// Consumes the reader and materializes the complete acquisition.
    ///
    /// File-backed readers first copy the acquisition payload to a private
    /// temporary file while hashing it, then decode that same copy. This needs
    /// temporary disk space equal to the source length and may fail with an I/O
    /// error. The copy buffer respects the configured working-byte limit.
    pub fn into_dataset_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<RawDataset, ReadError> {
        control.begin(ExecutionStage::Reading, None, None)?;
        let format = self.provenance.format();
        (|| {
            self.check_materialized_limit()?;
            self.provenance.verify_sources()?;
            let digest = self.source.snapshot_controlled(
                control,
                self.max_working_bytes
                    .checked_sub(self.retained_bytes)
                    .ok_or(ReadError::SizeOverflow)?,
                self.provenance.source_metadata(),
            )?;
            let data = self.materialize_data(control)?;
            let mut provenance = self.provenance;
            provenance.finalize_sources(digest)?;
            Ok(RawDataset::from_reader(
                self.descriptor,
                data,
                provenance,
                self.sampling,
            )?)
        })()
        .map_err(|error: ReadError| match format {
            Some(format) => error.with_format(format),
            None => error,
        })
    }

    pub(super) fn materialize_data(
        &self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<RawData, ReadError> {
        let shape = self.descriptor.layout().logical_shape();
        let component_lanes = self.descriptor.layout().lane_counts();
        if self.is_sparse()? {
            let schedule = self
                .sampling
                .as_ref()
                .expect("sparse acquisition has a sampling schedule");
            let trace_len = self
                .direct_points()
                .checked_mul(product(component_lanes)?)
                .ok_or(ReadError::SizeOverflow)?;
            let sample_count = trace_len
                .checked_mul(schedule.coordinates().len())
                .ok_or(ReadError::SizeOverflow)?;
            enforce_sample_limit(
                sample_count,
                self.max_materialized_bytes,
                "materialized acquisition",
            )?;
            let trace_count = schedule.coordinates().len();
            let trace_bytes = trace_count
                .checked_mul(std::mem::size_of::<SparseTrace>())
                .ok_or(ReadError::SizeOverflow)?;
            let mut traces = Vec::new();
            traces
                .try_reserve_exact(trace_count)
                .map_err(|_| ReadError::allocation(trace_bytes))?;
            for (acquisition, coordinate) in schedule.coordinates().iter().enumerate() {
                let samples = self.source.read_scheduled_trace_controlled(
                    control,
                    acquisition,
                    coordinate.as_slice(),
                )?;
                traces.push(SparseTrace::new(
                    crate::raw::ObservationOrdinal::new(acquisition),
                    coordinate.clone(),
                    samples,
                ));
            }
            return Ok(RawData::sparse(self.descriptor.layout().clone(), traces)?);
        }

        let storage_shape = storage_shape(shape, component_lanes)?;
        let total = product(&storage_shape)?;
        enforce_sample_limit(
            total,
            self.max_materialized_bytes,
            "materialized acquisition",
        )?;
        let mut samples = reserve_complex_samples(total)?;
        samples.resize(total, Complex64::default());
        if let Some(schedule) = &self.sampling {
            for (acquisition, coordinate) in schedule.coordinates().iter().enumerate() {
                let trace = self.source.read_scheduled_trace_controlled(
                    control,
                    acquisition,
                    coordinate.as_slice(),
                )?;
                crate::raw::model::scatter_trace(
                    &mut samples,
                    &storage_shape,
                    shape,
                    component_lanes,
                    coordinate.as_slice(),
                    &trace,
                )?;
            }
        } else {
            let indirect_shape = &shape[..shape.len() - 1];
            for index in 0..product(indirect_shape)? {
                let coordinate = unflatten(indirect_shape, index)?;
                let trace = self.source.read_trace_controlled(control, &coordinate)?;
                crate::raw::model::scatter_trace(
                    &mut samples,
                    &storage_shape,
                    shape,
                    component_lanes,
                    &coordinate,
                    &trace,
                )?;
            }
        }
        Ok(RawData::dense(self.descriptor.layout().clone(), samples)?)
    }
}
