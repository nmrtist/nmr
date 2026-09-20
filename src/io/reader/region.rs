use crate::Complex64;
use crate::ExecutionContext;
use crate::ReadError;
use crate::execution::ExecutionStage;
use crate::raw::RawData;
use crate::raw::RawLayout;
use crate::raw::Region;
use crate::raw::RegionData;
use crate::raw::SparseTrace;

use super::Reader;
use super::storage::{
    enforce_sample_limit, product, reserve_complex_samples, storage_shape, unflatten,
};

impl Reader {
    /// Uses default synchronous execution controls.
    pub fn read_region(&self, region: &Region, max_bytes: usize) -> Result<RegionData, ReadError> {
        self.read_region_with_context(&mut ExecutionContext::default(), region, max_bytes)
    }

    /// Reads a checked logical region relative to the full acquisition.
    pub fn read_region_with_context(
        &self,
        control: &mut ExecutionContext<'_>,
        region: &Region,
        max_bytes: usize,
    ) -> Result<RegionData, ReadError> {
        control.begin(ExecutionStage::Reading, None, None)?;
        self.read_region_inner(control, region, max_bytes.min(self.max_region_bytes))
            .map_err(|error| self.with_format(error))
    }

    pub(super) fn read_region_inner(
        &self,
        control: &mut ExecutionContext<'_>,
        region: &Region,
        max_bytes: usize,
    ) -> Result<RegionData, ReadError> {
        crate::raw::model::validate_region(
            self.descriptor.layout().logical_shape(),
            region.start(),
            region.shape(),
        )?;
        let data = if self.is_sparse()? {
            self.read_sparse_region(control, region, max_bytes)?
        } else {
            self.read_dense_region(control, region, max_bytes)?
        };
        Ok(RegionData::new(region.clone(), data))
    }

    pub(super) fn read_dense_region(
        &self,
        control: &mut ExecutionContext<'_>,
        region: &Region,
        max_bytes: usize,
    ) -> Result<RawData, ReadError> {
        let component_lanes = self.descriptor.layout().lane_counts();
        let total = product(region.shape())?
            .checked_mul(product(component_lanes)?)
            .ok_or(ReadError::SizeOverflow)?;
        enforce_sample_limit(total, max_bytes, "region")?;
        self.check_numeric_working(
            total,
            product(component_lanes)?
                .checked_mul(region.shape()[region.shape().len() - 1])
                .ok_or(ReadError::SizeOverflow)?,
            self.block_metadata_bytes(true, None)?,
        )?;
        let storage_shape = storage_shape(region.shape(), component_lanes)?;
        let mut output = reserve_complex_samples(total)?;
        output.resize(total, Complex64::default());

        let indirect_shape = &region.shape()[..region.shape().len() - 1];
        for index in 0..product(indirect_shape)? {
            let relative = unflatten(indirect_shape, index)?;
            let mut coordinate = relative.clone();
            for (value, &start) in coordinate.iter_mut().zip(region.start()) {
                *value = value.checked_add(start).ok_or(ReadError::SizeOverflow)?;
            }
            let trace = self.source.read_trace_controlled(control, &coordinate)?;
            let cropped = crop_trace(
                &trace,
                self.direct_points(),
                region.start()[region.start().len() - 1],
                region.shape()[region.shape().len() - 1],
                product(component_lanes)?,
            )?;
            crate::raw::model::scatter_trace(
                &mut output,
                &storage_shape,
                region.shape(),
                component_lanes,
                &relative,
                &cropped,
            )?;
        }

        Ok(RawData::dense(
            RawLayout::from_region(
                region.shape().to_vec(),
                component_lanes.to_vec(),
                region.start().to_vec(),
            )?,
            output,
        )?)
    }

    pub(super) fn read_sparse_region(
        &self,
        control: &mut ExecutionContext<'_>,
        region: &Region,
        max_bytes: usize,
    ) -> Result<RawData, ReadError> {
        let schedule = self
            .sampling
            .as_ref()
            .expect("sparse acquisition has a sampling schedule");
        let matching = schedule
            .coordinates()
            .iter()
            .enumerate()
            .filter(|(_, coordinate)| inside_indirect_region(coordinate.as_slice(), region));
        // Count without allocating an index proportional to the schedule before
        // enforcing the output limit. Replaying this borrowed iterator preserves
        // acquisition ordinals, including duplicate observations.
        let matching_count = matching.clone().count();
        if matching_count == 0 {
            return Err(crate::AccessError::UnsampledRegion {
                start: region.start().to_vec(),
                shape: region.shape().to_vec(),
            }
            .into());
        }

        let component_lanes = self.descriptor.layout().lane_counts();
        let output_trace_len = product(component_lanes)?
            .checked_mul(region.shape()[region.shape().len() - 1])
            .ok_or(ReadError::SizeOverflow)?;
        let output_samples = output_trace_len
            .checked_mul(matching_count)
            .ok_or(ReadError::SizeOverflow)?;
        enforce_sample_limit(output_samples, max_bytes, "region")?;
        self.check_numeric_working(
            output_samples,
            output_trace_len,
            self.block_metadata_bytes(true, Some(matching_count))?,
        )?;

        let trace_bytes = matching_count
            .checked_mul(std::mem::size_of::<SparseTrace>())
            .ok_or(ReadError::SizeOverflow)?;
        let mut traces = Vec::new();
        traces
            .try_reserve_exact(matching_count)
            .map_err(|_| ReadError::allocation(trace_bytes))?;
        for (acquisition, coordinate) in matching {
            let samples = self.source.read_scheduled_trace_controlled(
                control,
                acquisition,
                coordinate.as_slice(),
            )?;
            let samples = crop_trace(
                &samples,
                self.direct_points(),
                region.start()[region.start().len() - 1],
                region.shape()[region.shape().len() - 1],
                product(component_lanes)?,
            )?;
            traces.push(SparseTrace::new(
                crate::raw::ObservationOrdinal::new(acquisition),
                coordinate.clone(),
                samples,
            ));
        }

        Ok(RawData::sparse(
            RawLayout::from_region(
                region.shape().to_vec(),
                component_lanes.to_vec(),
                region.start().to_vec(),
            )?,
            traces,
        )?)
    }
}

pub(super) fn inside_indirect_region(coordinate: &[usize], region: &Region) -> bool {
    coordinate.iter().enumerate().all(|(axis, &value)| {
        value >= region.start()[axis] && value < region.start()[axis] + region.shape()[axis]
    })
}

pub(super) fn crop_trace(
    samples: &[Complex64],
    source_direct_points: usize,
    direct_start: usize,
    direct_length: usize,
    component_count: usize,
) -> Result<Vec<Complex64>, ReadError> {
    let expected = source_direct_points
        .checked_mul(component_count)
        .ok_or(ReadError::SizeOverflow)?;
    if samples.len() != expected {
        return Err(ReadError::corrupt(
            crate::raw::InputSource::memory("decoded trace"),
            "decoded trace length does not match its component layout",
        ));
    }
    let end = direct_start
        .checked_add(direct_length)
        .ok_or(ReadError::SizeOverflow)?;
    let output_len = direct_length
        .checked_mul(component_count)
        .ok_or(ReadError::SizeOverflow)?;
    let mut output = reserve_complex_samples(output_len)?;
    for component in 0..component_count {
        let start = component
            .checked_mul(source_direct_points)
            .and_then(|value| value.checked_add(direct_start))
            .ok_or(ReadError::SizeOverflow)?;
        let stop = component
            .checked_mul(source_direct_points)
            .and_then(|value| value.checked_add(end))
            .ok_or(ReadError::SizeOverflow)?;
        output.extend_from_slice(&samples[start..stop]);
    }
    Ok(output)
}
