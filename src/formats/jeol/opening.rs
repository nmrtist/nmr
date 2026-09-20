use super::embedded;
use super::header::HEADER_LEN;
use super::header::JdfHeader;
use super::header::header_offset;
use super::layout::LayoutPlan;
use super::metadata::descriptor_from_header;
use super::parameters::Parameters;
use super::parameters::SampleTransform;
use super::parser::parse_parameter_records;
use super::reader::TraceReader;
use super::semantics::coordinate_overrides;
use super::semantics::decode_axis_lists;
use super::semantics::jeol_sampling_plan;
use super::semantics::validate_supported_layout;
use crate::Complex64;
use crate::Domain;
use crate::ReadError;
use crate::SourceFile;
use crate::VendorMetadata;
use crate::provenance::SourceKind;
use crate::raw::RawFormat;
use crate::raw::RawProvenance;
use crate::raw::ReadLimits;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;

/// Whether a path contains a JDF file.
pub(crate) fn is_dataset(path: impl AsRef<Path>) -> bool {
    let p = path.as_ref();
    if p.is_file() {
        if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jdf")) {
            return false;
        }
        let mut h = [0u8; 8];
        return fs::File::open(p)
            .ok()
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut h).ok())
            .is_some()
            && h == *b"JEOL.NMR";
    }
    p.is_dir()
        && fs::read_dir(p)
            .ok()
            .is_some_and(|mut it| it.any(|e| e.ok().is_some_and(|x| is_dataset(x.path()))))
}

/// Classifies a recognized JDF using only its fixed header.
pub(crate) fn is_frequency_domain(path: &Path) -> Result<bool, ReadError> {
    let mut file = fs::File::open(path).map_err(|error| ReadError::io(path, error))?;
    let mut bytes = [0u8; 40];
    let count = file
        .read(&mut bytes)
        .map_err(|error| ReadError::io(path, error))?;
    if count <= header_offset::DIMENSION {
        return Ok(false);
    }
    let dimensions = usize::from(bytes[header_offset::DIMENSION]).min(4);
    Ok((0..dimensions).any(|axis| {
        let offset = header_offset::AXIS_UNIT + axis * 2;
        bytes
            .get(offset..offset + 2)
            .is_some_and(|unit| unit[0] & 0x0f == 1 && matches!(unit[1], 13 | 26))
    }))
}

pub(crate) fn open_acquisition_declared(
    path: &Path,
    options: &ReadLimits,
    declaration: Option<&std::sync::Arc<crate::SamplingDeclaration>>,
    cancellation: &crate::CancellationToken,
) -> Result<crate::io::Reader, ReadError> {
    open_acquisition_inner(path, options, declaration, cancellation)
        .map_err(|error| error.with_format(RawFormat::JeolDelta))
}

pub(crate) fn open_acquisition_inner(
    path: &Path,
    options: &ReadLimits,
    declaration: Option<&std::sync::Arc<crate::SamplingDeclaration>>,
    cancellation: &crate::CancellationToken,
) -> Result<crate::io::Reader, ReadError> {
    let path = resolve(path)?;
    let source_len = crate::ensure_file_size(
        &path,
        options.source_bytes(),
        crate::raw::ReadResource::SourceBytes,
    )?;
    let raw_header = read_file_range(&path, 0, HEADER_LEN)?;
    let header = JdfHeader::parse_for_source(&raw_header, source_len, path.clone().into())?;
    if header
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
    let data_end = header
        .data_start
        .checked_add(needed_bytes)
        .ok_or(ReadError::SizeOverflow)?;
    if data_end > source_len {
        return Err(ReadError::truncated(
            header.input_source.clone(),
            data_end,
            source_len,
        ));
    }
    let pre_data_bytes = header
        .data_start
        .checked_sub(HEADER_LEN)
        .ok_or(ReadError::SizeOverflow)?;
    let trailing_bytes = source_len - data_end;
    let metadata_bytes = HEADER_LEN
        .checked_add(pre_data_bytes)
        .and_then(|value| value.checked_add(trailing_bytes))
        .ok_or(ReadError::SizeOverflow)?;
    if metadata_bytes > options.metadata_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::MetadataBytes,
            options.metadata_bytes(),
            metadata_bytes,
        ));
    }
    let raw_pre_data_records = read_file_range(&path, HEADER_LEN, pre_data_bytes)?;
    let raw_trailing_records = read_file_range(&path, data_end, trailing_bytes)?;
    let parsed_parameters = if header.param_start == 0 || header.param_length == 0 {
        BTreeMap::new()
    } else {
        let table_end = header
            .param_start
            .checked_add(header.param_length)
            .ok_or(ReadError::SizeOverflow)?;
        if header.param_start < data_end && table_end > header.data_start {
            return Err(ReadError::corrupt(
                header.input_source.clone(),
                "JEOL parameter table overlaps the sample payload",
            ));
        }
        let table = read_file_range(&path, header.param_start, header.param_length)?;
        parse_parameter_records(&table, &header, header.param_start)?
    };
    let axis_lists = decode_axis_lists(&header, |start, length| {
        read_file_range(&path, start, length)
    })?;
    let mut coordinate_overrides = coordinate_overrides(&header, &axis_lists)?;
    let sampling_plan = jeol_sampling_plan(
        &header,
        &parsed_parameters,
        &axis_lists,
        declaration,
        Some(cancellation),
    )?;

    let ndim = header.points_disk.len();
    let component_lanes: Vec<usize> = (0..ndim)
        .map(|output_axis| {
            let disk_axis = ndim - 1 - output_axis;
            usize::from(output_axis + 1 != ndim && header.axis_types[disk_axis] == 3) + 1
        })
        .collect();
    let mut shape = (0..ndim)
        .map(|output_axis| {
            let disk_axis = ndim - 1 - output_axis;
            header.offset_stop[disk_axis]
                .checked_sub(header.offset_start[disk_axis])
                .and_then(|value| value.checked_add(1))
                .ok_or(ReadError::SizeOverflow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(plan) = &sampling_plan {
        shape[0] = plan.logical_points;
        coordinate_overrides[0] = Some(plan.coordinates.clone());
    }
    let logical_shape: Vec<usize> = header.points_disk.iter().rev().copied().collect();
    let submatrix_edge = header.data_format.submatrix_edge();
    if logical_shape.len() > 1
        && (submatrix_edge == 0 || logical_shape.iter().any(|&size| size % submatrix_edge != 0))
    {
        return Err(ReadError::unsupported_code(
            header.input_source.clone(),
            crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
            "JEOL data shape is not divisible by its submatrix edge",
        ));
    }
    let component_count =
        crate::checked_product(&component_lanes).ok_or(ReadError::SizeOverflow)?;
    let trace_samples = shape[ndim - 1]
        .checked_mul(component_count)
        .ok_or(ReadError::SizeOverflow)?;
    let working_bytes = trace_samples
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    if working_bytes > options.working_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::TraceBytes,
            options.working_bytes(),
            working_bytes,
        ));
    }

    let sources = vec![SourceFile::new(SourceKind::Data, "jdf", &path)?];
    let embedded_axes = embedded::parse(
        &header,
        &raw_pre_data_records,
        &raw_trailing_records,
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
    options.validate_descriptor(&descriptor)?;
    let pn_y = super::metadata::pn_y(&header, &parsed_parameters)?;
    let metadata = Parameters {
        raw_header,
        raw_pre_data_records,
        raw_trailing_records,
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
    };
    let layout = LayoutPlan::new(
        header.input_source.clone(),
        header.data_start,
        header.precision,
        header.body_endian,
        sections,
        physical_count,
        logical_shape,
        shape,
        header.offset_start.iter().rev().copied().collect(),
        component_lanes,
        sampling_plan
            .as_ref()
            .map(|plan| plan.acquired_indices.clone()),
        submatrix_edge,
    )?;
    let source = TraceReader::open(path, layout, pn_y)?;
    Ok(crate::io::Reader::new(
        descriptor,
        RawProvenance::reader(
            RawFormat::JeolDelta,
            sources,
            VendorMetadata::jeol(metadata),
        ),
        sampling_plan.map(|plan| plan.schedule),
        Box::new(source),
        options.region_bytes(),
        options.materialized_bytes(),
        options.working_bytes(),
    ))
}

pub(crate) fn read_file_range(
    path: &Path,
    offset: usize,
    length: usize,
) -> Result<Vec<u8>, ReadError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| ReadError::allocation(length))?;
    output.resize(length, 0);
    if length == 0 {
        return Ok(output);
    }
    let mut file = fs::File::open(path).map_err(|error| ReadError::io(path, error))?;
    file.seek(SeekFrom::Start(
        u64::try_from(offset).map_err(|_| ReadError::SizeOverflow)?,
    ))
    .and_then(|_| file.read_exact(&mut output))
    .map_err(|error| ReadError::io(path, error))?;
    Ok(output)
}

pub(crate) fn resolve(path: &Path) -> Result<PathBuf, ReadError> {
    if path.is_file() {
        return is_dataset(path)
            .then(|| path.to_path_buf())
            .ok_or_else(|| ReadError::unrecognized(path.into(), "not a JEOL JDF"));
    }
    let mut candidates: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|error| ReadError::io(path, error))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| is_dataset(p))
        .collect();
    candidates.sort();
    match candidates.len() {
        0 => Err(ReadError::unrecognized(path.into(), "not a JEOL JDF")),
        1 => Ok(candidates.remove(0)),
        count => Err(ReadError::ambiguous(
            path.into(),
            format!("directory contains {count} JEOL JDF files"),
        )),
    }
}
