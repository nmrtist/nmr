use super::decoder;
use super::embedded;
use super::header::HEADER_LEN;
use super::header::JdfHeader;
use super::metadata::descriptor_from_header;
use super::parameters::Parameters;
use super::parameters::SampleTransform;
use super::parser::parse_parameter_table;
use super::sections::combine_sections;
use super::sections::crop_region;
use super::sections::reorder_section;
use super::semantics::coordinate_overrides;
use super::semantics::decode_axis_lists;
use super::semantics::jeol_sampling_plan;
use super::semantics::require_experimental_opt_in;
use super::semantics::validate_supported_layout;
use crate::Acquisition;
use crate::AcquisitionData;
use crate::Complex64;
use crate::Domain;
use crate::ReadError;
use crate::SamplingCoordinate;
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

/// Borrowed bytes of one complete in-memory JDF file.
#[derive(Clone, Copy, Debug)]
pub struct Parts<'a> {
    pub(super) jdf: &'a [u8],
    pub(super) experimental_vendor_semantics: bool,
}

impl<'a> Parts<'a> {
    /// Creates parts from a complete JDF byte stream.
    pub fn new(jdf: &'a [u8]) -> Self {
        Self {
            jdf,
            experimental_vendor_semantics: false,
        }
    }

    /// Explicitly accepts experimental JEOL section/sign, tile, list and filter interpretations.
    ///
    /// Independent evidence is incomplete. These interpretations may change;
    /// opting in does not permit layouts outside the documented subset.
    pub fn allow_experimental_vendor_semantics(mut self, value: bool) -> Self {
        self.experimental_vendor_semantics = value;
        self
    }
}

/// Decodes one checked in-memory JDF file.
pub fn read_parts(parts: Parts<'_>) -> Result<Acquisition, ReadError> {
    read_parts_with_limits(parts, ReadLimits::default())
}

/// Decodes borrowed raw JDF parts with explicit source, metadata and numerical limits.
pub fn read_parts_with_limits(
    parts: Parts<'_>,
    limits: ReadLimits,
) -> Result<Acquisition, ReadError> {
    require_experimental_opt_in(
        parts.experimental_vendor_semantics,
        InputSource::memory("jdf"),
    )
    .map_err(|error| error.with_format(RawFormat::JeolDelta))?;
    read_from_parts(parts.jdf, &limits)
}

pub(crate) fn read_from_parts(
    bytes: &[u8],
    options: &ReadLimits,
) -> Result<Acquisition, ReadError> {
    read_from_parts_inner(bytes, options, false, 0)
        .map(|dataset| {
            dataset.with_reader_sources(vec![SourceFile::from_consumed_bytes(
                SourceKind::Data,
                "jdf",
                Path::new(""),
                bytes,
            )])
        })
        .map_err(|error| error.with_format(RawFormat::JeolDelta))
}

pub(crate) fn read_from_parts_inner(
    bytes: &[u8],
    options: &ReadLimits,
    allow_frequency: bool,
    owned_source_bytes: usize,
) -> Result<Acquisition, ReadError> {
    if bytes.len() > options.source_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::SourceBytes,
            options.source_bytes(),
            bytes.len(),
        ));
    }
    let header = JdfHeader::parse(bytes)?;
    if !allow_frequency
        && header
            .axis_units
            .iter()
            .flatten()
            .any(|unit| unit.domain() == Domain::Frequency)
    {
        return Err(ReadError::unsupported_code(
            header.input_source.clone(),
            crate::raw::UnsupportedFeatureCode::SPECTRUM_REPRESENTATION,
            "frequency-domain JDF requires a separate spectrum model",
        ));
    }
    validate_supported_layout(&header.axis_types, &header.input_source)?;
    let sections = header.sections()?;
    let ndim = header.points_disk.len();
    let logical: Vec<usize> = header.points_disk.iter().rev().copied().collect();
    let physical_count =
        crate::checked_product(&header.points_disk).ok_or(ReadError::SizeOverflow)?;
    let value_count = physical_count
        .checked_mul(sections)
        .ok_or(ReadError::SizeOverflow)?;
    let needed_bytes = value_count
        .checked_mul(header.precision.size())
        .ok_or(ReadError::SizeOverflow)?;
    if header.data_length != 0 && header.data_length < needed_bytes {
        return Err(ReadError::truncated(
            header.input_source.clone(),
            header
                .data_start
                .checked_add(needed_bytes)
                .ok_or(ReadError::SizeOverflow)?,
            header
                .data_start
                .checked_add(header.data_length)
                .ok_or(ReadError::SizeOverflow)?,
        ));
    }
    if header.data_length > needed_bytes {
        return Err(ReadError::corrupt(
            header.input_source.clone(),
            format!(
                "JEOL data length {} does not match the supported layout size {needed_bytes}",
                header.data_length
            ),
        ));
    }
    let intermediate_bytes = value_count
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ReadError::SizeOverflow)?;
    let result_count = if sections == 4 && ndim == 2 && header.axis_types == [3, 3] {
        physical_count
            .checked_mul(2)
            .ok_or(ReadError::SizeOverflow)?
    } else {
        physical_count
    };
    let result_bytes = result_count
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let decode_bytes = intermediate_bytes
        .checked_mul(2)
        .and_then(|value| value.checked_add(result_bytes.checked_mul(2)?))
        .and_then(|value| value.checked_add(owned_source_bytes))
        .ok_or(ReadError::SizeOverflow)?;
    if decode_bytes > options.working_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::WorkingBytes,
            options.working_bytes(),
            decode_bytes,
        ));
    }
    let data_end = header
        .data_start
        .checked_add(needed_bytes)
        .ok_or(ReadError::SizeOverflow)?;
    let metadata_bytes = header
        .data_start
        .checked_sub(HEADER_LEN)
        .and_then(|before| before.checked_add(bytes.len().saturating_sub(data_end)))
        .and_then(|records| records.checked_add(HEADER_LEN))
        .ok_or(ReadError::SizeOverflow)?;
    if metadata_bytes > options.metadata_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::MetadataBytes,
            options.metadata_bytes(),
            metadata_bytes,
        ));
    }
    let parsed_parameters = parse_parameter_table(bytes, &header, data_end)?;
    let axis_lists = decode_axis_lists(&header, |start, length| {
        bytes
            .get(start..start + length)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| {
                ReadError::truncated(header.input_source.clone(), start + length, bytes.len())
            })
    })?;
    let mut coordinate_overrides = coordinate_overrides(&header, &axis_lists)?;
    let sampling_plan = jeol_sampling_plan(&header, &parsed_parameters, &axis_lists, None, None)?;
    let payload = bytes.get(header.data_start..data_end).ok_or_else(|| {
        ReadError::truncated(header.input_source.clone(), header.data_start, bytes.len())
    })?;
    if payload.len() < needed_bytes {
        let expected = header
            .data_start
            .checked_add(needed_bytes)
            .ok_or(ReadError::SizeOverflow)?;
        return Err(ReadError::truncated(
            header.input_source.clone(),
            expected,
            header.data_start + payload.len(),
        ));
    }
    let raw_bytes = value_count
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(value_count)
        .map_err(|_| ReadError::allocation(raw_bytes))?;
    for chunk in payload[..needed_bytes].chunks_exact(header.precision.size()) {
        raw.push(decoder::decode_value(
            chunk,
            header.precision,
            header.body_endian,
            &header.input_source,
        )?);
    }
    let reordered: Vec<Vec<f64>> = raw
        .chunks_exact(physical_count)
        .map(|section| {
            reorder_section(
                section,
                &logical,
                header.data_format.submatrix_edge(),
                &header.input_source,
            )
        })
        .collect::<Result<_, _>>()?;
    let (mut storage_shape, mut samples) = combine_sections(
        &reordered,
        &logical,
        &header.axis_types,
        &header.input_source,
    )?;
    if super::metadata::pn_y(&header, &parsed_parameters)? {
        super::sections::pn_to_cartesian(&mut samples, logical[1]);
    }
    let component_lanes: Vec<usize> = (0..ndim)
        .map(|output_axis| {
            let disk_axis = ndim - 1 - output_axis;
            usize::from(output_axis + 1 != ndim && header.axis_types[disk_axis] == 3) + 1
        })
        .collect();
    let crop = allow_frequency
        || !(ndim == 1
            && header.axis_units[0].is_some_and(|unit| unit.domain() == Domain::Frequency));
    if crop {
        let mut starts = Vec::with_capacity(ndim);
        let mut ends = Vec::with_capacity(ndim);
        for (output_axis, disk_axis) in (0..ndim).rev().enumerate() {
            let multiplier = component_lanes[output_axis];
            let start = header.offset_start[disk_axis]
                .checked_mul(multiplier)
                .ok_or(ReadError::SizeOverflow)?;
            let end = header.offset_stop[disk_axis]
                .checked_add(1)
                .and_then(|value| value.checked_mul(multiplier))
                .and_then(|value| value.checked_sub(1))
                .ok_or(ReadError::SizeOverflow)?;
            starts.push(start);
            ends.push(end);
        }
        (storage_shape, samples) = crop_region(
            &storage_shape,
            &samples,
            &starts,
            &ends,
            &header.input_source,
        )?;
    }
    let physical_shape: Vec<usize> = storage_shape
        .iter()
        .zip(&component_lanes)
        .map(|(&stored, &lanes)| stored / lanes)
        .collect();
    let mut shape = physical_shape.clone();
    if let Some(plan) = &sampling_plan {
        shape[0] = plan.logical_points;
        coordinate_overrides[0] = Some(plan.coordinates.clone());
    }
    let embedded_axes = embedded::parse(
        &header,
        &bytes[HEADER_LEN..header.data_start],
        &bytes[data_end..],
        options,
        metadata_bytes,
    )?;
    let descriptor = descriptor_from_header(
        &header,
        &parsed_parameters,
        &shape,
        &component_lanes,
        &coordinate_overrides,
        &embedded_axes,
    )?;
    let data = match &sampling_plan {
        Some(plan) => {
            let physical = AcquisitionData::dense(
                RawLayout::from_parts(physical_shape, component_lanes.clone())?,
                samples,
            )?;
            let mut traces = Vec::with_capacity(plan.acquired_indices.len());
            for (row, &logical_index) in plan.acquired_indices.iter().enumerate() {
                let samples = physical.read_trace(&[row])?.samples().to_vec();
                traces.push(SparseTrace::new(
                    crate::raw::ObservationOrdinal::new(row),
                    SamplingCoordinate::new(vec![logical_index]),
                    samples,
                ));
            }
            AcquisitionData::sparse(
                RawLayout::from_parts(shape, component_lanes.clone())?,
                traces,
            )?
        }
        None => AcquisitionData::dense(
            RawLayout::from_parts(shape, component_lanes.clone())?,
            samples,
        )?,
    };
    Ok(Acquisition::from_reader(
        descriptor,
        data,
        RawProvenance::reader(
            RawFormat::JeolDelta,
            Vec::new(),
            VendorMetadata::jeol(Parameters {
                raw_header: bytes[..HEADER_LEN].to_vec(),
                raw_pre_data_records: bytes[HEADER_LEN..header.data_start].to_vec(),
                raw_trailing_records: bytes[data_end..].to_vec(),
                embedded_axes,
                axis_units: header.raw_axis_units.clone(),
                axis_types: header.axis_types.clone(),
                sample_transform: SampleTransform {
                    direct_imaginary_multiplier: if sections > 1 { -1 } else { 1 },
                    indirect_lane_multipliers: component_lanes
                        .iter()
                        .take(ndim.saturating_sub(1))
                        .flat_map(|&lanes| if lanes == 2 { vec![1, -1] } else { vec![1] })
                        .collect(),
                },
                values: parsed_parameters,
            }),
        ),
        sampling_plan.map(|plan| plan.schedule),
    )?)
}
