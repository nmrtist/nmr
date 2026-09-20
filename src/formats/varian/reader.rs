use super::decoder;
use super::layout::LayoutPlan;
use super::ordering::canonical_trace_index;
use super::ordering::flatten_index;

use crate::ExecutionContext;
use crate::io::{ByteSource, TraceSource};
use crate::{Complex64, ReadError};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) struct TraceReader {
    source: ByteSource,
    binary: LayoutPlan,
    indirect_shape: Vec<usize>,
    indirect_lanes: Vec<usize>,
    trace_mapping: Vec<usize>,
    schedule_positions: Option<BTreeMap<Vec<usize>, usize>>,
}

impl TraceReader {
    pub(super) fn open(
        path: PathBuf,
        binary: LayoutPlan,
        indirect_shape: Vec<usize>,
        indirect_lanes: Vec<usize>,
        trace_mapping: Vec<usize>,
        schedule_positions: Option<BTreeMap<Vec<usize>, usize>>,
    ) -> Result<Self, ReadError> {
        Ok(Self {
            source: ByteSource::open(path)?,
            binary,
            indirect_shape,
            indirect_lanes,
            trace_mapping,
            schedule_positions,
        })
    }

    fn read_disk_trace(
        &self,
        control: &mut ExecutionContext<'_>,
        disk_trace: usize,
    ) -> Result<Vec<Complex64>, ReadError> {
        if disk_trace >= self.binary.trace_count {
            return Err(ReadError::corrupt(
                self.source.path().into(),
                "trace mapping exceeds Varian fid data",
            ));
        }
        let block = disk_trace / self.binary.traces_per_block;
        let trace_in_block = disk_trace % self.binary.traces_per_block;
        let offset = 32usize
            .checked_add(
                block
                    .checked_mul(self.binary.block_bytes)
                    .ok_or(ReadError::SizeOverflow)?,
            )
            .and_then(|value| value.checked_add(self.binary.block_header_bytes))
            .and_then(|value| {
                value.checked_add(trace_in_block.checked_mul(self.binary.trace_bytes)?)
            })
            .ok_or(ReadError::SizeOverflow)?;
        let bytes =
            self.source
                .read_exact_at_controlled(control, offset, self.binary.trace_bytes)?;

        decoder::decode_trace(&bytes, &self.binary, block, &|| self.source.path().into())
    }

    #[cfg(test)]
    fn bytes_read(&self) -> u64 {
        self.source.bytes_read()
    }
}

impl TraceSource for TraceReader {
    fn trace_numeric_bytes(&self) -> Result<usize, ReadError> {
        // The assembled component trace, one decoded disk trace and its input
        // bytes coexist inside read_disk_trace.
        crate::checked_product(&self.indirect_lanes)
            .and_then(|lanes| lanes.checked_add(1))
            .and_then(|lanes| lanes.checked_mul(self.binary.direct_points))
            .and_then(|count| count.checked_mul(std::mem::size_of::<Complex64>()))
            .and_then(|bytes| bytes.checked_add(self.binary.trace_bytes))
            .ok_or(ReadError::SizeOverflow)
    }

    fn snapshot_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        working_limit: usize,
        metadata: &crate::VendorMetadata,
    ) -> Result<Option<crate::provenance::SourceDigest>, ReadError> {
        let digest = self.source.snapshot_controlled(control, working_limit)?;
        let parameters = metadata
            .as_varian()
            .ok_or_else(|| ReadError::source_changed(self.source.path().into()))?;
        let header = parameters
            .file_header()
            .ok_or_else(|| ReadError::source_changed(self.source.path().into()))?;
        self.source
            .verify_range_controlled(control, 0, header.raw_bytes(), working_limit)?;
        for (index, block) in parameters.block_headers().iter().enumerate() {
            let offset = index
                .checked_mul(header.block_bytes())
                .and_then(|value| value.checked_add(32))
                .ok_or(ReadError::SizeOverflow)?;
            self.source.verify_range_controlled(
                control,
                offset,
                block.raw_bytes(),
                working_limit,
            )?;
        }
        Ok(Some(digest))
    }
    fn read_trace_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        let logical_trace = match &self.schedule_positions {
            Some(positions) => positions.get(coordinate).copied().ok_or_else(|| {
                crate::AccessError::UnsampledCoordinate {
                    coordinate: coordinate.to_vec(),
                }
            })?,
            None => flatten_index(&self.indirect_shape, coordinate)?,
        };
        let component_count =
            crate::checked_product(&self.indirect_lanes).ok_or(ReadError::SizeOverflow)?;
        let mut output = Vec::with_capacity(
            component_count
                .checked_mul(self.binary.direct_points)
                .ok_or(ReadError::SizeOverflow)?,
        );
        for component in 0..component_count {
            let canonical_trace = canonical_trace_index(
                &self.indirect_shape,
                &self.indirect_lanes,
                logical_trace,
                component,
            )?;
            let disk_trace = *self.trace_mapping.get(canonical_trace).ok_or_else(|| {
                ReadError::corrupt(
                    self.source.path().into(),
                    "canonical trace mapping is incomplete",
                )
            })?;
            output.extend(self.read_disk_trace(control, disk_trace)?);
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_index_interleaves_each_axis_without_changing_order() {
        for slow in 0..2 {
            for middle in 0..3 {
                for fast in 0..4 {
                    for first_lane in 0..2 {
                        for last_lane in 0..3 {
                            assert_eq!(
                                canonical_trace_index(
                                    &[2, 3, 4],
                                    &[2, 1, 3],
                                    slow * 12 + middle * 4 + fast,
                                    first_lane * 3 + last_lane,
                                )
                                .unwrap(),
                                (slow * 2 + first_lane) * 36 + middle * 12 + fast * 3 + last_lane,
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(canonical_trace_index(&[], &[], 0, 0).unwrap(), 0);
        assert!(canonical_trace_index(&[2], &[2], 2, 0).is_err());
        assert!(canonical_trace_index(&[2], &[2], 0, 2).is_err());
        assert!(canonical_trace_index(&[2], &[], 0, 0).is_err());
        assert!(canonical_trace_index(&[0], &[2], 0, 0).is_err());
        assert!(canonical_trace_index(&[usize::MAX], &[2], 0, 0).is_err());
    }

    #[test]
    fn trace_reads_only_its_block_payload() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut bytes = vec![0; 60];
        for value in [1_i16, 2, 3, 4] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        std::fs::write(file.path(), bytes).unwrap();
        let layout = LayoutPlan {
            traces_per_block: 1,
            stored_values: 4,
            bytes_per_value: 2,
            trace_bytes: 8,
            block_bytes: 36,
            block_header_bytes: 28,
            direct_points: 2,
            trace_count: 1,
            is_float: false,
            is_32_bit_integer: false,
            complex: true,
            scale_factors: vec![1.0],
        };
        let reader = TraceReader::open(
            file.path().to_path_buf(),
            layout,
            Vec::new(),
            Vec::new(),
            vec![0],
            None,
        )
        .unwrap();

        assert_eq!(reader.trace_numeric_bytes().unwrap(), 72);
        let samples = reader
            .read_trace_controlled(&mut ExecutionContext::default(), &[])
            .unwrap();
        assert_eq!(
            samples,
            [Complex64::new(1.0, -2.0), Complex64::new(3.0, -4.0)]
        );
        assert_eq!(reader.bytes_read(), 8);
    }
}
