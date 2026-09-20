//! Owned region blocks and bounded access to an immutable Dataset.
//!
//! ```no_run
//! use nmr::data::Region;
//! use nmr::resource::MemoryLimits;
//! let input = nmr::read("data/experiment")?;
//! let region = Region::new([0], [128])?;
//! let estimate = input.region_resources(&region)?;
//! let block = input.read_region(&region, MemoryLimits::new())?;
//! drop(input);
//! assert_eq!(block.region(), &region);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::Dataset;
use crate::processed::ProcessedData;
use crate::resource::{LimitExceeded, MemoryLimits, ResourceEstimate, ResourceKind};

pub use crate::raw::Region;

/// A failure to validate or copy a dataset region.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum AccessError {
    /// Cooperative execution control failed.
    #[error(transparent)]
    Execution(#[from] crate::execution::ExecutionError),
    /// Logical region coordinates are invalid or unsampled.
    #[error(transparent)]
    Region(#[from] crate::raw::AccessError),
    /// A sample, metadata or total working bound exceeds its limit.
    #[error(transparent)]
    LimitExceeded(#[from] LimitExceeded),
    /// A checked capacity or index calculation overflowed.
    #[error("region size computation overflow")]
    SizeOverflow,
    /// A region sample buffer could not be allocated.
    #[error("could not allocate {requested_bytes} bytes for region samples")]
    Allocation {
        /// Requested sample payload capacity.
        requested_bytes: usize,
    },
    /// Raw access failed; the structured underlying reason is preserved.
    #[error(transparent)]
    Raw(#[from] crate::ReadError),
    /// Constructing the processed block failed validation.
    #[error(transparent)]
    Processed(#[from] crate::processed::ProcessedValidationError),
}

/// An independent owned copy of one logical region, retaining every component.
/// A block is not a new processing origin and carries no invented history.
#[derive(Debug)]
pub struct DataBlock(BlockData);

#[derive(Debug)]
enum BlockData {
    Raw(crate::raw::RegionData),
    Processed { region: Region, data: ProcessedData },
}

impl DataBlock {
    /// Returns the absolute region represented by this block.
    pub fn region(&self) -> &Region {
        match &self.0 {
            BlockData::Raw(value) => value.region(),
            BlockData::Processed { region, .. } => region,
        }
    }

    /// Borrows raw storage, including absolute coordinates and sparse ordinals.
    pub fn as_raw(&self) -> Option<&crate::raw::RegionData> {
        match &self.0 {
            BlockData::Raw(value) => Some(value),
            _ => None,
        }
    }

    /// Borrows processed storage with local logical coordinates and all components.
    pub fn as_processed(&self) -> Option<&ProcessedData> {
        match &self.0 {
            BlockData::Processed { data, .. } => Some(data),
            _ => None,
        }
    }
}

impl Dataset {
    /// Computes allocation bounds for an owned region without copying samples.
    /// Caller-owned input is excluded; temporary and returned metadata are included.
    pub fn region_resources(&self, region: &Region) -> Result<ResourceEstimate, AccessError> {
        let (shape, components, origin) = if let Some(raw) = self.as_raw() {
            (
                raw.data().shape(),
                raw.data().component_lanes(),
                Some(raw.data().layout().absolute_origin()),
            )
        } else {
            let data = self
                .as_dense_processed()
                .expect("dataset representation is exhaustive");
            (data.shape(), data.component_counts(), None)
        };
        validate_region(shape, origin, region)?;
        let rank = shape.len();
        let (samples, metadata, width) = if let Some(raw) = self.as_raw() {
            // Output Region (2 rank arrays), RawLayout (3), relative start (1).
            let mut metadata =
                array_bytes::<usize>(rank.checked_mul(6).ok_or(AccessError::SizeOverflow)?)?;
            let samples = if let Some(traces) = raw.data().sparse_traces() {
                let count =
                    traces
                        .iter()
                        .filter(|trace| {
                            trace.coordinate().as_slice().iter().enumerate().all(
                                |(axis, &point)| {
                                    point >= region.start()[axis]
                                        && point < region.start()[axis] + region.shape()[axis]
                                },
                            )
                        })
                        .count();
                if count == 0 {
                    return Err(crate::raw::AccessError::UnsampledRegion {
                        start: region.start().to_vec(),
                        shape: region.shape().to_vec(),
                    }
                    .into());
                }
                // Each output trace owns a coordinate; validation may sort ordinals.
                let per_trace = std::mem::size_of::<crate::raw::SparseTrace>()
                    .checked_add(array_bytes::<usize>(rank - 1)?)
                    .and_then(|bytes| {
                        bytes.checked_add(std::mem::size_of::<crate::raw::ObservationOrdinal>())
                    })
                    .ok_or(AccessError::SizeOverflow)?;
                metadata = count
                    .checked_mul(per_trace)
                    .and_then(|bytes| metadata.checked_add(bytes))
                    .ok_or(AccessError::SizeOverflow)?;
                components
                    .iter()
                    .try_fold(count, |count, &lanes| count.checked_mul(lanes))
                    .and_then(|count| count.checked_mul(region.shape()[rank - 1]))
                    .ok_or(AccessError::SizeOverflow)?
            } else {
                sample_count(region.shape(), components)?
            };
            (samples, metadata, std::mem::size_of::<crate::Complex64>())
        } else {
            // Output Region plus ProcessedData shape and component-count arrays.
            (
                sample_count(region.shape(), components)?,
                array_bytes::<usize>(rank.checked_mul(4).ok_or(AccessError::SizeOverflow)?)?,
                std::mem::size_of::<f64>(),
            )
        };
        let output = samples
            .checked_mul(width)
            .filter(|&bytes| bytes <= isize::MAX as usize)
            .ok_or(AccessError::SizeOverflow)?;
        let working = output
            .checked_add(metadata)
            .ok_or(AccessError::SizeOverflow)?;
        Ok(ResourceEstimate::new(output, metadata, working))
    }

    /// Copies a region under independent output, metadata and total working limits.
    /// All bounds are checked before allocating the block. It outlives the input.
    pub fn read_region(
        &self,
        region: &Region,
        limits: MemoryLimits,
    ) -> Result<DataBlock, AccessError> {
        self.read_region_with_context(region, limits, &mut crate::ExecutionContext::default())
    }
    /// Copies a region with cooperative cancellation and progress.
    pub fn read_region_with_context(
        &self,
        region: &Region,
        limits: MemoryLimits,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<DataBlock, AccessError> {
        control.begin(crate::execution::ExecutionStage::Access, None, None)?;

        let estimate = self.region_resources(region)?;
        for (resource, required, limit) in [
            (
                ResourceKind::OutputBytes,
                estimate.output_bytes(),
                limits.output_bytes(),
            ),
            (
                ResourceKind::MetadataBytes,
                estimate.metadata_bytes(),
                limits.metadata_bytes(),
            ),
            (
                ResourceKind::WorkingBytes,
                estimate.working_bytes(),
                limits.working_bytes(),
            ),
        ] {
            if required > limit {
                return Err(LimitExceeded {
                    resource,
                    required,
                    limit,
                }
                .into());
            }
        }
        if let Some(raw) = self.as_raw() {
            return Ok(DataBlock(BlockData::Raw(
                raw.data()
                    .read_region_with_context(region, limits.output_bytes(), control)?,
            )));
        }
        let source = self
            .as_dense_processed()
            .expect("dataset representation is exhaustive");
        control.observe_payload(estimate.working_bytes());
        let count = estimate.output_bytes() / std::mem::size_of::<f64>();
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(count)
            .map_err(|_| AccessError::Allocation {
                requested_bytes: estimate.output_bytes(),
            })?;
        for output_index in 0..count {
            if output_index % 4096 == 0 {
                control.advance((count - output_index).min(4096) as u128)?;
            }
            let mut remaining = output_index;
            let mut source_index = 0usize;
            let mut stride = 1usize;
            for axis in (0..source.shape().len()).rev() {
                let components = source.component_counts()[axis];
                let extent = region.shape()[axis]
                    .checked_mul(components)
                    .ok_or(AccessError::SizeOverflow)?;
                let coordinate = region.start()[axis]
                    .checked_mul(components)
                    .and_then(|start| start.checked_add(remaining % extent))
                    .ok_or(AccessError::SizeOverflow)?;
                remaining /= extent;
                source_index = coordinate
                    .checked_mul(stride)
                    .and_then(|index| source_index.checked_add(index))
                    .ok_or(AccessError::SizeOverflow)?;
                stride = stride
                    .checked_mul(source.shape()[axis])
                    .and_then(|stride| stride.checked_mul(components))
                    .ok_or(AccessError::SizeOverflow)?;
            }
            samples.push(source.samples()[source_index]);
        }
        Ok(DataBlock(BlockData::Processed {
            region: region.clone(),
            data: ProcessedData::new(
                region.shape().to_vec(),
                source.component_counts().to_vec(),
                samples,
            )?,
        }))
    }
}

fn array_bytes<T>(count: usize) -> Result<usize, AccessError> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(AccessError::SizeOverflow)
}

fn sample_count(shape: &[usize], components: &[usize]) -> Result<usize, AccessError> {
    shape
        .iter()
        .zip(components)
        .try_fold(1usize, |count, (&points, &lanes)| {
            count.checked_mul(points)?.checked_mul(lanes)
        })
        .ok_or(AccessError::SizeOverflow)
}

fn validate_region(
    shape: &[usize],
    origin: Option<&[usize]>,
    region: &Region,
) -> Result<(), AccessError> {
    if region.shape().len() != shape.len() {
        return Err(crate::raw::AccessError::RegionRankMismatch {
            expected: shape.len(),
            start_rank: region.start().len(),
            shape_rank: region.shape().len(),
        }
        .into());
    }
    for (axis, &points) in shape.iter().enumerate() {
        let origin = origin.map_or(0, |values| values[axis]);
        let end = origin
            .checked_add(points)
            .ok_or(AccessError::SizeOverflow)?;
        let start = region.start()[axis];
        let length = region.shape()[axis];
        if start < origin || start.checked_add(length).is_none_or(|stop| stop > end) {
            return Err(crate::raw::AccessError::RegionOutOfBounds {
                axis,
                start,
                length,
                points: end,
            }
            .into());
        }
    }
    Ok(())
}
