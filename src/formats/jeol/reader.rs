use super::decoder;
use super::layout::LayoutPlan;

use crate::ExecutionContext;
use crate::io::{ByteSource, TraceSource};
use crate::{Complex64, ReadError};
use std::path::PathBuf;

pub(super) struct TraceReader {
    source: ByteSource,
    layout: LayoutPlan,
    pn_y: bool,
}

impl TraceReader {
    pub(super) fn open(path: PathBuf, layout: LayoutPlan, pn_y: bool) -> Result<Self, ReadError> {
        Ok(Self {
            source: ByteSource::open(path)?,
            layout,
            pn_y,
        })
    }

    fn offset(
        &self,
        section: usize,
        coordinate: &[usize],
        direct: usize,
        observation: Option<usize>,
    ) -> Result<usize, ReadError> {
        match observation {
            Some(ordinal) => self
                .layout
                .sample_offset_observation(section, coordinate, direct, ordinal),
            None => self.layout.sample_offset(section, coordinate, direct),
        }
    }

    #[cfg(test)]
    fn bytes_read(&self) -> u64 {
        self.source.bytes_read()
    }

    fn read_trace_at(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
        observation: Option<usize>,
    ) -> Result<Vec<Complex64>, ReadError> {
        if coordinate.len() != self.layout.trace_rank() {
            return Err(ReadError::corrupt(
                self.layout.input_source().clone(),
                "JEOL trace coordinate rank mismatch",
            ));
        }
        let direct_points = self.layout.direct_points();
        let component_count = self.layout.component_count()?;
        let sample_count = direct_points
            .checked_mul(component_count)
            .ok_or(ReadError::SizeOverflow)?;
        let mut output = Vec::new();
        output.try_reserve_exact(sample_count).map_err(|_| {
            ReadError::allocation(sample_count.saturating_mul(std::mem::size_of::<Complex64>()))
        })?;
        output.resize(sample_count, Complex64::new(0.0, 0.0));
        // A fixed stack buffer preserves the existing heap-working contract.
        // Read only physical runs belonging to this trace, stopping at tile,
        // section and crop boundaries. No unrelated tile rows are fetched.
        let mut bytes = [0u8; 8192];
        let scalar_bytes = self.layout.precision().size();
        for section in 0..self.layout.sections() {
            let mut direct = 0;
            while direct < direct_points {
                let start = self.offset(section, coordinate, direct, observation)?;
                let mut count = 1;
                while direct + count < direct_points && count < bytes.len() / scalar_bytes {
                    let expected = start
                        .checked_add(count * scalar_bytes)
                        .ok_or(ReadError::SizeOverflow)?;
                    if self.offset(section, coordinate, direct + count, observation)? != expected {
                        break;
                    }
                    count += 1;
                }
                self.source.read_exact_at_into_controlled(
                    control,
                    start,
                    &mut bytes[..count * scalar_bytes],
                )?;
                for (index, encoded) in bytes[..count * scalar_bytes]
                    .chunks_exact(scalar_bytes)
                    .enumerate()
                {
                    let value = decoder::decode_value(
                        encoded,
                        self.layout.precision(),
                        self.layout.endian(),
                        self.layout.input_source(),
                    )?;
                    let value = if section >= 2 { -value } else { value };
                    let sample = &mut output[(section / 2) * direct_points + direct + index];
                    if section % 2 == 0 {
                        sample.re = value;
                    } else {
                        sample.im = -value;
                    }
                }
                direct += count;
            }
        }
        if self.pn_y {
            super::sections::pn_to_cartesian(&mut output, direct_points);
        }
        Ok(output)
    }
}

impl TraceSource for TraceReader {
    fn trace_numeric_bytes(&self) -> Result<usize, ReadError> {
        // Binary values use an 8 KiB fixed stack buffer, excluded from heap limits.
        self.layout
            .component_count()?
            .checked_mul(self.layout.direct_points())
            .and_then(|count| count.checked_mul(std::mem::size_of::<Complex64>()))
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
            .as_jeol()
            .ok_or_else(|| ReadError::source_changed(self.source.path().into()))?;
        self.source
            .verify_range_controlled(control, 0, parameters.raw_header(), working_limit)?;
        self.source.verify_range_controlled(
            control,
            super::HEADER_LEN,
            parameters.raw_pre_data_records(),
            working_limit,
        )?;
        let trailing = parameters.raw_trailing_records();
        let offset = self
            .source
            .length()?
            .checked_sub(trailing.len())
            .ok_or(ReadError::SizeOverflow)?;
        self.source
            .verify_range_controlled(control, offset, trailing, working_limit)?;
        Ok(Some(digest))
    }
    fn read_trace_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        self.read_trace_at(control, coordinate, None)
    }

    fn read_scheduled_trace_controlled(
        &self,
        control: &mut ExecutionContext<'_>,
        acquisition: usize,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        self.read_trace_at(control, coordinate, Some(acquisition))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::jeol::header::{BodyEndian, Precision};

    #[test]
    fn trace_reads_only_referenced_section_values() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut bytes = Vec::new();
        for value in 1_u64..=8 {
            bytes.extend_from_slice(&(value as f64).to_le_bytes());
        }
        std::fs::write(file.path(), bytes).unwrap();
        let layout = LayoutPlan::new(
            file.path().into(),
            0,
            Precision::F64,
            BodyEndian::Little,
            2,
            4,
            vec![2, 2],
            vec![2, 2],
            vec![0, 0],
            vec![1, 1],
            None,
            1,
        )
        .unwrap();
        let reader = TraceReader::open(file.path().to_path_buf(), layout, false).unwrap();

        assert_eq!(reader.trace_numeric_bytes().unwrap(), 32);
        let samples = reader
            .read_trace_controlled(&mut ExecutionContext::default(), &[1])
            .unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(reader.bytes_read(), 32);
        assert_eq!(reader.source.read_calls(), 2);
    }

    #[test]
    fn representative_scale_reads_are_batched_and_bit_exact() {
        use std::time::Instant;
        for (rows, columns, tile) in [(1, 32768, 8), (1, 65536, 8), (1024, 2048, 32)] {
            let count = rows * columns;
            let file = tempfile::NamedTempFile::new().unwrap();
            let mut bytes = Vec::with_capacity(count * 16);
            for section in 0..2 {
                for index in 0..count {
                    bytes.extend_from_slice(&((section * count + index + 1) as f64).to_le_bytes());
                }
            }
            std::fs::write(file.path(), &bytes).unwrap();
            drop(bytes);
            let shape = if rows == 1 {
                vec![columns]
            } else {
                vec![rows, columns]
            };
            let layout = LayoutPlan::new(
                file.path().into(),
                0,
                Precision::F64,
                BodyEndian::Little,
                2,
                count,
                shape.clone(),
                shape.clone(),
                vec![0; shape.len()],
                vec![1; shape.len()],
                None,
                tile,
            )
            .unwrap();
            let reader = TraceReader::open(file.path().to_path_buf(), layout, false).unwrap();
            let copy_start = Instant::now();
            reader.source.snapshot(65536).unwrap();
            let copy_ms = copy_start.elapsed().as_secs_f64() * 1000.0;
            let begin = Instant::now();
            for row in 0..rows {
                let coordinate = if rows == 1 { vec![] } else { vec![row] };
                let samples = reader
                    .read_trace_controlled(&mut ExecutionContext::default(), &coordinate)
                    .unwrap();
                assert_eq!(samples.capacity(), columns);
                for (column, sample) in samples.iter().enumerate() {
                    // Independent row/tile address arithmetic, not LayoutPlan offsets.
                    let index = if rows == 1 {
                        column
                    } else {
                        ((row / tile) * (columns / tile) + column / tile) * tile * tile
                            + (row % tile) * tile
                            + column % tile
                    };
                    assert_eq!(sample.re.to_bits(), ((index + 1) as f64).to_bits());
                    assert_eq!(
                        sample.im.to_bits(),
                        (-((count + index + 1) as f64)).to_bits()
                    );
                }
            }
            let elapsed_ms = begin.elapsed().as_secs_f64() * 1000.0;
            let expected_calls = if rows == 1 {
                count * 2 / 1024
            } else {
                rows * columns * 2 / tile
            };
            assert_eq!(reader.source.read_calls(), expected_calls as u64);
            assert_eq!(reader.bytes_read(), (count * 16) as u64);
            println!(
                "JEOL scale rows={rows} columns={columns} source_bytes={} snapshot_ms={copy_ms:.3} trace_scan_ms={elapsed_ms:.3} read_calls={} scalar_calls_before={} peak_trace_heap_bytes={} fixed_stack_bytes=8192 snapshot_heap_bytes=65536 temporary_disk_bytes={}",
                count * 16,
                expected_calls,
                count * 2,
                reader.trace_numeric_bytes().unwrap(),
                count * 16
            );
        }
    }
}
