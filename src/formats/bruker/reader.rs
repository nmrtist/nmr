use super::layout::LayoutPlan;
use super::sample_decode;
use super::storage::DirectStorage;

use crate::ExecutionContext;
use crate::io::{ByteSource, TraceSource};
use crate::{Complex64, ReadError};
use std::path::PathBuf;

pub(super) struct TraceReader {
    source: ByteSource,
    storage: DirectStorage,
    layout: LayoutPlan,
}

impl TraceReader {
    pub(super) fn open(
        path: PathBuf,
        storage: DirectStorage,
        layout: LayoutPlan,
    ) -> Result<Self, ReadError> {
        Ok(Self {
            source: ByteSource::open(path)?,
            storage,
            layout,
        })
    }

    #[cfg(test)]
    pub(super) fn bytes_read(&self) -> u64 {
        self.source.bytes_read()
    }

    fn read_two_dimensional_acquisition(
        &self,
        control: &mut ExecutionContext<'_>,
        acquisition: usize,
        stride: usize,
        payload_bytes: usize,
        direct_points: usize,
        indirect_lanes: usize,
    ) -> Result<Vec<Complex64>, ReadError> {
        let sample_count = direct_points
            .checked_mul(indirect_lanes)
            .ok_or(ReadError::SizeOverflow)?;
        let mut samples = vec![Complex64::default(); sample_count];
        for lane in 0..indirect_lanes {
            let row = acquisition
                .checked_mul(indirect_lanes)
                .and_then(|value| value.checked_add(lane))
                .ok_or(ReadError::SizeOverflow)?;
            let offset = row.checked_mul(stride).ok_or(ReadError::SizeOverflow)?;
            let bytes = self
                .source
                .read_exact_at_controlled(control, offset, payload_bytes)?;
            sample_decode::decode_complex_pairs_into(
                &bytes,
                self.storage,
                "ser",
                self.source.path(),
                &mut samples[lane * direct_points..(lane + 1) * direct_points],
            )?;
        }
        Ok(samples)
    }

    #[allow(clippy::too_many_arguments)]
    fn read_three_dimensional_trace(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
        stride: usize,
        payload_bytes: usize,
        direct_points: usize,
        stored_second_indirect: usize,
        slow_lanes: usize,
        fast_lanes: usize,
    ) -> Result<Vec<Complex64>, ReadError> {
        if coordinate.len() != 2 {
            return Err(ReadError::corrupt(
                self.source.path().into(),
                "Bruker 3D trace coordinate must have rank two",
            ));
        }
        let component_count = slow_lanes
            .checked_mul(fast_lanes)
            .ok_or(ReadError::SizeOverflow)?;
        let sample_count = direct_points
            .checked_mul(component_count)
            .ok_or(ReadError::SizeOverflow)?;
        let mut samples = vec![Complex64::default(); sample_count];
        for slow_lane in 0..slow_lanes {
            let stored_slow = coordinate[0]
                .checked_mul(slow_lanes)
                .and_then(|value| value.checked_add(slow_lane))
                .ok_or(ReadError::SizeOverflow)?;
            for fast_lane in 0..fast_lanes {
                let stored_fast = coordinate[1]
                    .checked_mul(fast_lanes)
                    .and_then(|value| value.checked_add(fast_lane))
                    .ok_or(ReadError::SizeOverflow)?;
                let row = stored_slow
                    .checked_mul(stored_second_indirect)
                    .and_then(|value| value.checked_add(stored_fast))
                    .ok_or(ReadError::SizeOverflow)?;
                let offset = row.checked_mul(stride).ok_or(ReadError::SizeOverflow)?;
                let bytes = self
                    .source
                    .read_exact_at_controlled(control, offset, payload_bytes)?;
                let component = slow_lane
                    .checked_mul(fast_lanes)
                    .and_then(|value| value.checked_add(fast_lane))
                    .ok_or(ReadError::SizeOverflow)?;
                let start = component
                    .checked_mul(direct_points)
                    .ok_or(ReadError::SizeOverflow)?;
                sample_decode::decode_complex_pairs_into(
                    &bytes,
                    self.storage,
                    "ser",
                    self.source.path(),
                    &mut samples[start..start + direct_points],
                )?;
            }
        }
        Ok(samples)
    }
}

impl TraceSource for TraceReader {
    fn trace_numeric_bytes(&self) -> Result<usize, ReadError> {
        let (points, lanes, payload) = match &self.layout {
            LayoutPlan::OneD { payload_bytes } => (self.storage.td / 2, 1, *payload_bytes),
            LayoutPlan::TwoD {
                direct_points,
                indirect_lanes,
                payload_bytes,
                ..
            } => (*direct_points, *indirect_lanes, *payload_bytes),
            LayoutPlan::ThreeD {
                direct_points,
                slow_lanes,
                fast_lanes,
                payload_bytes,
                ..
            } => (
                *direct_points,
                slow_lanes
                    .checked_mul(*fast_lanes)
                    .ok_or(ReadError::SizeOverflow)?,
                *payload_bytes,
            ),
        };
        points
            .checked_mul(lanes)
            .and_then(|count| count.checked_mul(std::mem::size_of::<Complex64>()))
            .and_then(|bytes| bytes.checked_add(payload))
            .ok_or(ReadError::SizeOverflow)
    }

    fn snapshot_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        working_limit: usize,
        _metadata: &crate::VendorMetadata,
    ) -> Result<Option<crate::provenance::SourceDigest>, ReadError> {
        self.source
            .snapshot_controlled(control, working_limit)
            .map(Some)
    }
    fn read_trace_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        match &self.layout {
            LayoutPlan::OneD { payload_bytes } => {
                if !coordinate.is_empty() {
                    return Err(ReadError::corrupt(
                        self.source.path().into(),
                        "Bruker 1D trace coordinate must be empty",
                    ));
                }
                let bytes = self
                    .source
                    .read_exact_at_controlled(control, 0, *payload_bytes)?;
                Ok(sample_decode::decode_complex_pairs(
                    &bytes,
                    self.storage,
                    "fid",
                    self.source.path(),
                    self.storage.td / 2,
                )?)
            }
            LayoutPlan::TwoD {
                stride,
                payload_bytes,
                direct_points,
                indirect_lanes,
                acquisition_rows,
            } => {
                if coordinate.len() != 1 {
                    return Err(ReadError::corrupt(
                        self.source.path().into(),
                        "Bruker 2D trace coordinate must have rank one",
                    ));
                }
                let acquisition = match acquisition_rows {
                    Some(rows) => {
                        rows.get(coordinate[0])
                            .and_then(|row| *row)
                            .ok_or_else(|| crate::AccessError::UnsampledCoordinate {
                                coordinate: coordinate.to_vec(),
                            })?
                    }
                    None => coordinate[0],
                };
                self.read_two_dimensional_acquisition(
                    control,
                    acquisition,
                    *stride,
                    *payload_bytes,
                    *direct_points,
                    *indirect_lanes,
                )
            }
            LayoutPlan::ThreeD {
                stride,
                payload_bytes,
                direct_points,
                stored_second_indirect,
                slow_lanes,
                fast_lanes,
            } => self.read_three_dimensional_trace(
                control,
                coordinate,
                *stride,
                *payload_bytes,
                *direct_points,
                *stored_second_indirect,
                *slow_lanes,
                *fast_lanes,
            ),
        }
    }

    fn read_scheduled_trace_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        acquisition: usize,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        match &self.layout {
            LayoutPlan::TwoD {
                stride,
                payload_bytes,
                direct_points,
                indirect_lanes,
                ..
            } => self.read_two_dimensional_acquisition(
                control,
                acquisition,
                *stride,
                *payload_bytes,
                *direct_points,
                *indirect_lanes,
            ),
            LayoutPlan::OneD { .. } | LayoutPlan::ThreeD { .. } => {
                self.read_trace_controlled(control, coordinate)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::bruker::storage::{Endian, SampleFormat};

    #[test]
    fn two_dimensional_trace_reads_only_component_payloads() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut bytes = Vec::new();
        for value in 1_i32..=8 {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        std::fs::write(file.path(), bytes).unwrap();

        let reader = TraceReader::open(
            file.path().to_path_buf(),
            DirectStorage {
                td: 4,
                endian: Endian::Little,
                sample: SampleFormat::I32,
            },
            LayoutPlan::TwoD {
                stride: 16,
                payload_bytes: 16,
                direct_points: 2,
                indirect_lanes: 2,
                acquisition_rows: None,
            },
        )
        .unwrap();

        assert_eq!(reader.trace_numeric_bytes().unwrap(), 80);
        let samples = reader
            .read_trace_controlled(&mut ExecutionContext::default(), &[0])
            .unwrap();
        assert_eq!(samples.len(), 4);
        assert_eq!(reader.bytes_read(), 32);
    }
}
