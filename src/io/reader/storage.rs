use crate::Complex64;
use crate::ReadError;
use crate::raw::SparseTrace;

use super::Reader;

impl Reader {
    pub(super) fn check_materialized_limit(&self) -> Result<(), ReadError> {
        let logical_samples = if self.is_sparse()? {
            self.direct_points()
                .checked_mul(
                    self.sampling
                        .as_ref()
                        .expect("sparse acquisition has a schedule")
                        .coordinates()
                        .len(),
                )
                .ok_or(ReadError::SizeOverflow)?
        } else {
            product(self.descriptor.layout().logical_shape())?
        };
        let samples = logical_samples
            .checked_mul(product(self.descriptor.layout().lane_counts())?)
            .ok_or(ReadError::SizeOverflow)?;
        enforce_sample_limit(
            samples,
            self.max_materialized_bytes,
            "materialized acquisition",
        )?;
        let sparse_count = if self.is_sparse()? {
            Some(
                self.sampling
                    .as_ref()
                    .expect("sparse schedule")
                    .coordinates()
                    .len(),
            )
        } else {
            None
        };
        self.check_numeric_working(samples, 0, self.block_metadata_bytes(false, sparse_count)?)
    }

    pub(super) fn trace_metadata_bytes(&self, observation: bool) -> Result<usize, ReadError> {
        let rank = self.descriptor.axes().len();
        // Returned coordinate and lane arrays, plus observation lookup coordinate.
        rank.checked_add(
            (rank - 1)
                .checked_mul(if observation { 2 } else { 1 })
                .ok_or(ReadError::SizeOverflow)?,
        )
        .and_then(|words| words.checked_mul(std::mem::size_of::<usize>()))
        .ok_or(ReadError::SizeOverflow)
    }

    pub(super) fn block_metadata_bytes(
        &self,
        region: bool,
        sparse_count: Option<usize>,
    ) -> Result<usize, ReadError> {
        let rank = self.descriptor.axes().len();
        // Final layout (3R), optional returned Region (2R). Dense copies also
        // keep storage shape (R) and local/absolute iteration coordinates.
        let arrays = if region { 5usize } else { 3usize } + usize::from(sparse_count.is_none());
        let mut words = rank.checked_mul(arrays).ok_or(ReadError::SizeOverflow)?;
        if sparse_count.is_none() {
            words = words
                .checked_add(
                    (rank - 1)
                        .checked_mul(if region { 2 } else { 1 })
                        .ok_or(ReadError::SizeOverflow)?,
                )
                .ok_or(ReadError::SizeOverflow)?;
        }
        let bytes = words
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or(ReadError::SizeOverflow)?;
        let sparse = match sparse_count {
            Some(count) => (rank - 1)
                .checked_mul(std::mem::size_of::<usize>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<SparseTrace>()))
                .and_then(|bytes| bytes.checked_mul(count))
                .ok_or(ReadError::SizeOverflow)?,
            None => 0,
        };
        bytes.checked_add(sparse).ok_or(ReadError::SizeOverflow)
    }

    pub(super) fn check_numeric_working(
        &self,
        output_samples: usize,
        cropped_samples: usize,
        metadata_bytes: usize,
    ) -> Result<(), ReadError> {
        let decoding = self.source.trace_numeric_bytes()?;
        let cropping = if cropped_samples == 0 {
            0
        } else {
            self.descriptor
                .axes()
                .iter()
                .try_fold(self.direct_points(), |count, axis| {
                    count.checked_mul(axis.component_lanes())
                })
                .and_then(|count| count.checked_add(cropped_samples))
                .and_then(|count| count.checked_mul(std::mem::size_of::<Complex64>()))
                .ok_or(ReadError::SizeOverflow)?
        };
        let required = output_samples
            .checked_mul(std::mem::size_of::<Complex64>())
            .and_then(|bytes| bytes.checked_add(decoding.max(cropping)))
            .and_then(|bytes| bytes.checked_add(self.retained_bytes))
            .and_then(|bytes| bytes.checked_add(metadata_bytes))
            .ok_or(ReadError::SizeOverflow)?;
        if required > self.max_working_bytes {
            return Err(ReadError::limit(
                crate::raw::ReadResource::WorkingBytes,
                self.max_working_bytes,
                required,
            ));
        }
        Ok(())
    }
}

pub(super) fn reserve_complex_samples(length: usize) -> Result<Vec<Complex64>, ReadError> {
    let requested_bytes = length
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(length)
        .map_err(|_| ReadError::allocation(requested_bytes))?;
    Ok(samples)
}

pub(super) fn storage_shape(
    shape: &[usize],
    component_lanes: &[usize],
) -> Result<Vec<usize>, ReadError> {
    let bytes = shape
        .len()
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut storage = Vec::new();
    storage
        .try_reserve_exact(shape.len())
        .map_err(|_| ReadError::allocation(bytes))?;
    for (&points, &lanes) in shape.iter().zip(component_lanes) {
        storage.push(points.checked_mul(lanes).ok_or(ReadError::SizeOverflow)?);
    }
    Ok(storage)
}

pub(super) fn enforce_sample_limit(
    samples: usize,
    limit: usize,
    label: &str,
) -> Result<(), ReadError> {
    let bytes = samples
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    if bytes > limit {
        let resource = if label == "region" {
            crate::raw::ReadResource::RegionBytes
        } else {
            crate::raw::ReadResource::MaterializedBytes
        };
        return Err(ReadError::limit(resource, limit, bytes));
    }
    Ok(())
}

pub(super) fn product(values: &[usize]) -> Result<usize, ReadError> {
    values
        .iter()
        .try_fold(1usize, |product, &value| product.checked_mul(value))
        .ok_or(ReadError::SizeOverflow)
}

pub(super) fn unflatten(shape: &[usize], mut index: usize) -> Result<Vec<usize>, ReadError> {
    if index >= product(shape)? {
        return Err(ReadError::corrupt(
            crate::raw::InputSource::memory("normalized raw data"),
            "normalized logical coordinate is out of bounds",
        ));
    }
    let mut coordinate = vec![0; shape.len()];
    for axis in (0..shape.len()).rev() {
        coordinate[axis] = index % shape[axis];
        index /= shape[axis];
    }
    Ok(coordinate)
}
