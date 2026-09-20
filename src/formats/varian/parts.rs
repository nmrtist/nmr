use super::decoder;
use super::metadata::descriptor_from_layout;
use super::options::ReadAssertions;
use super::options::VarianOptions;
use super::ordering::canonical_to_disk_mapping;
use super::ordering::canonical_trace_group;
use super::ordering::reorder_dense;
use super::ordering::scatter_logical_trace;
use super::parser;
use super::parser::parse_procpar;
use super::semantics::nus_enabled;
use super::semantics::parse_schedule;
use super::semantics::resolve_layout_from_procpar;
use super::semantics::scalar_parameter;
use super::semantics::select_trace_order;
use super::semantics::validate_assertions;
use crate::Acquisition;
use crate::AcquisitionData;
use crate::Complex64;
use crate::ReadError;
use crate::SourceFile;
use crate::SparseTrace;
use crate::VendorMetadata;
use crate::provenance::SourceKind;
use crate::raw::InputSource;
use crate::raw::RawFormat;
use crate::raw::RawLayout;
use crate::raw::RawProvenance;
use crate::raw::ReadLimits;
use std::path::Path;

/// Borrowed in-memory parts of one Varian raw acquisition.
#[derive(Clone, Debug)]
pub struct Parts<'a> {
    pub(super) fid: &'a [u8],
    pub(super) procpar: &'a str,
    pub(super) schedule: Option<&'a str>,
    pub(super) assertions: Option<ReadAssertions>,
}

impl<'a> Parts<'a> {
    /// Creates parts from `fid` bytes and `procpar` text.
    pub fn new(fid: &'a [u8], procpar: &'a str) -> Self {
        Self {
            fid,
            procpar,
            schedule: None,
            assertions: None,
        }
    }

    /// Adds `sampling.sch` text for an NUS acquisition.
    pub fn sampling_schedule(mut self, value: &'a str) -> Self {
        self.schedule = Some(value);
        self
    }

    /// Supplies complete conflict-checked acquisition assertions.
    pub fn assertions(mut self, value: ReadAssertions) -> Self {
        self.assertions = Some(value);
        self
    }
}

/// Decodes checked in-memory Varian parts.
pub fn read_parts(parts: Parts<'_>) -> Result<Acquisition, ReadError> {
    read_parts_with_limits(parts, ReadLimits::default())
}

/// Decodes borrowed Varian parts with explicit limits and their supplied assertions.
pub fn read_parts_with_limits(
    parts: Parts<'_>,
    limits: ReadLimits,
) -> Result<Acquisition, ReadError> {
    let options = VarianOptions::new(limits, parts.assertions.as_ref());
    read_from_parts(parts.fid, parts.procpar, parts.schedule, &options)
}

pub(crate) fn read_from_parts(
    fid: &[u8],
    procpar: &str,
    schedule: Option<&str>,
    o: &VarianOptions,
) -> Result<Acquisition, ReadError> {
    read_from_parts_inner(fid, procpar, schedule, o)
        .map(|dataset| {
            let mut sources = vec![
                SourceFile::from_consumed_bytes(SourceKind::Data, "fid", Path::new(""), fid),
                SourceFile::from_consumed_bytes(
                    SourceKind::Parameters,
                    "procpar",
                    Path::new(""),
                    procpar.as_bytes(),
                ),
            ];
            if let Some(text) = schedule {
                sources.push(SourceFile::from_consumed_bytes(
                    SourceKind::SamplingSchedule,
                    "sampling_schedule",
                    Path::new(""),
                    text.as_bytes(),
                ));
            }
            dataset.with_reader_sources(sources)
        })
        .map_err(|error| error.with_format(RawFormat::VarianRaw))
}

pub(crate) fn read_from_parts_inner(
    fid: &[u8],
    procpar: &str,
    schedule: Option<&str>,
    o: &VarianOptions,
) -> Result<Acquisition, ReadError> {
    if fid.len() > o.max_input_bytes {
        return Err(ReadError::limit(
            crate::raw::ReadResource::SourceBytes,
            o.max_input_bytes,
            fid.len(),
        ));
    }
    if procpar
        .len()
        .checked_add(schedule.map_or(0, str::len))
        .is_none_or(|bytes| bytes > o.max_metadata_bytes)
    {
        let required = procpar
            .len()
            .checked_add(schedule.map_or(0, str::len))
            .ok_or(ReadError::SizeOverflow)?;
        return Err(ReadError::limit(
            crate::raw::ReadResource::MetadataBytes,
            o.max_metadata_bytes,
            required,
        ));
    }
    if fid.len() < 32 {
        return Err(ReadError::truncated(
            crate::raw::InputSource::memory("fid"),
            32,
            fid.len(),
        ));
    }
    let fid_source = InputSource::memory("fid");
    let parsed_binary = parser::parse_binary(&fid[..32], fid.len(), o, &fid_source, |offset| {
        let end = offset.checked_add(28).ok_or(ReadError::SizeOverflow)?;
        fid.get(offset..end)
            .ok_or_else(|| {
                ReadError::truncated(crate::raw::InputSource::memory("fid"), end, fid.len())
            })?
            .try_into()
            .map_err(|_| {
                ReadError::corrupt(fid_source.clone(), "invalid Varian block header length")
            })
    })?;
    let samples = decoder::decode_all(fid, &parsed_binary.layout, o.max_decode_bytes, &fid_source)?;
    let points = parsed_binary.layout.direct_points;
    let traces = parsed_binary.layout.trace_count;
    let complex = parsed_binary.layout.complex;
    let sample_bytes = samples
        .len()
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut pp = parse_procpar(procpar, InputSource::memory("procpar"))?;
    pp.file_header = Some(parsed_binary.file_header);
    pp.block_headers = parsed_binary.block_headers;
    if schedule.is_none() && scalar_parameter(&pp, "nus")?.is_some_and(nus_enabled) {
        return Err(ReadError::incomplete(
            InputSource::memory("sampling.sch"),
            "Varian NUS dataset requires sampling.sch",
        ));
    }
    let layout = resolve_layout_from_procpar(
        &pp,
        points,
        traces,
        schedule.is_some(),
        o.assertions.as_ref(),
    )?;
    let dims = &layout.shape;
    let component_lanes = &layout.component_lanes;
    validate_assertions(o.assertions.as_ref(), complex, traces, &layout)?;
    let order = select_trace_order(&pp, &layout, None)?;
    if schedule.is_some() && !layout.array_dimensions.is_empty() {
        return Err(ReadError::unsupported_code(
            pp.input_source.clone(),
            crate::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT,
            "Varian NUS combined with a non-phase array axis is not verified",
        ));
    }
    let sched = if let Some(text) = schedule {
        Some(parse_schedule(
            text,
            dims,
            &InputSource::memory("sampling.sch"),
        )?)
    } else {
        None
    };
    let mapping_copies = if sched.is_some() { 3 } else { 2 };
    let mapping_peak_bytes = sample_bytes
        .checked_mul(mapping_copies)
        .ok_or(ReadError::SizeOverflow)?;
    if mapping_peak_bytes > o.max_decode_bytes {
        return Err(ReadError::limit(
            crate::raw::ReadResource::WorkingBytes,
            o.max_decode_bytes,
            mapping_peak_bytes,
        ));
    }
    let component_count = crate::checked_product(&component_lanes[..component_lanes.len() - 1])
        .ok_or(ReadError::SizeOverflow)?;
    let logical_trace_count = if let Some(schedule) = &sched {
        schedule.coordinates().len()
    } else {
        crate::checked_product(&dims[..dims.len() - 1]).ok_or(ReadError::SizeOverflow)?
    };
    let expected_traces = logical_trace_count
        .checked_mul(component_count)
        .ok_or(ReadError::SizeOverflow)?;
    if expected_traces != traces {
        return Err(ReadError::invalid_metadata(
            pp.input_source.clone(),
            None::<String>,
            format!("procpar/schedule describes {expected_traces} traces but fid stores {traces}"),
        ));
    }
    let trace_mapping = canonical_to_disk_mapping(
        &dims[..dims.len() - 1],
        &component_lanes[..component_lanes.len() - 1],
        order,
        layout.asserted_trace_permutation.as_deref(),
    )?;
    let data = if let Some(s) = &sched {
        let grid_count =
            crate::checked_product(&dims[..dims.len() - 1]).ok_or(ReadError::SizeOverflow)?;
        let logical_traces = s
            .coordinates()
            .iter()
            .enumerate()
            .map(|(index, _)| {
                canonical_trace_group(
                    &samples,
                    index,
                    &dims[..dims.len() - 1],
                    points,
                    &component_lanes[..component_lanes.len() - 1],
                    &trace_mapping,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if s.coordinates().len() < grid_count {
            AcquisitionData::sparse(
                RawLayout::from_parts(dims.clone(), component_lanes.clone())?,
                s.coordinates()
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        SparseTrace::new(
                            crate::raw::ObservationOrdinal::new(i),
                            c.clone(),
                            logical_traces[i].clone(),
                        )
                    })
                    .collect(),
            )?
        } else {
            let storage_shape: Vec<usize> = dims
                .iter()
                .zip(component_lanes)
                .map(|(&logical, &lanes)| logical.checked_mul(lanes).ok_or(ReadError::SizeOverflow))
                .collect::<Result<_, _>>()?;
            let mut canonical = vec![Complex64::new(0.0, 0.0); samples.len()];
            for (trace, coordinate) in s.coordinates().iter().enumerate() {
                scatter_logical_trace(
                    &mut canonical,
                    &storage_shape,
                    coordinate.as_slice(),
                    &component_lanes[..component_lanes.len() - 1],
                    points,
                    &logical_traces[trace],
                )?;
            }
            AcquisitionData::dense(
                RawLayout::from_parts(dims.clone(), component_lanes.clone())?,
                canonical,
            )?
        }
    } else {
        let canonical = reorder_dense(
            &samples,
            &dims[..dims.len() - 1],
            &component_lanes[..component_lanes.len() - 1],
            points,
            &trace_mapping,
        )?;
        AcquisitionData::dense(
            RawLayout::from_parts(dims.clone(), component_lanes.clone())?,
            canonical,
        )?
    };
    let descriptor = descriptor_from_layout(&pp, &layout, complex)?;
    o.limits.validate_descriptor(&descriptor)?;
    Ok(Acquisition::from_reader(
        descriptor,
        data,
        RawProvenance::reader(RawFormat::VarianRaw, Vec::new(), VendorMetadata::varian(pp)),
        sched,
    )?)
}
