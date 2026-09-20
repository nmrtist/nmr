use super::layout::LayoutPlan;
use super::metadata::descriptor_from_layout;
use super::options::ReadAssertions;
use super::options::VarianOptions;
use super::ordering::canonical_to_disk_mapping;
use super::parser::parse_procpar;
use super::reader::TraceReader;
use super::semantics::nus_enabled;
use super::semantics::parse_schedule;
use super::semantics::resolve_layout_from_procpar;
use super::semantics::scalar_parameter;
use super::semantics::select_trace_order;
use super::semantics::validate_assertions;
use crate::Complex64;
use crate::ReadError;
use crate::SourceFile;
use crate::VendorMetadata;
use crate::provenance::SourceKind;
use crate::raw::RawFormat;
use crate::raw::RawProvenance;
use crate::raw::ReadLimits;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn open_resolved_acquisition(
    data: &Path,
    procpar: &Path,
    schedule: &Path,
    limits: &ReadLimits,
    assertions: Option<&ReadAssertions>,
) -> Result<crate::io::Reader, ReadError> {
    let options = VarianOptions::new(*limits, assertions);
    open_resolved_acquisition_inner(
        (
            data.to_path_buf(),
            procpar.to_path_buf(),
            schedule.to_path_buf(),
        ),
        &options,
    )
    .map_err(|error| error.with_format(RawFormat::VarianRaw))
}

pub(crate) fn open_resolved_acquisition_inner(
    paths: (PathBuf, PathBuf, PathBuf),
    options: &VarianOptions,
) -> Result<crate::io::Reader, ReadError> {
    let source_len = crate::ensure_file_size(
        &paths.0,
        options.max_input_bytes,
        crate::raw::ReadResource::SourceBytes,
    )?;
    let procpar_bytes = crate::ensure_file_size(
        &paths.1,
        options.max_metadata_bytes,
        crate::raw::ReadResource::MetadataBytes,
    )?;
    let schedule_bytes = if paths.2.is_file() {
        crate::ensure_file_size(
            &paths.2,
            options.max_metadata_bytes,
            crate::raw::ReadResource::MetadataBytes,
        )?
    } else {
        0
    };
    if procpar_bytes
        .checked_add(schedule_bytes)
        .is_none_or(|bytes| bytes > options.max_metadata_bytes)
    {
        let required = procpar_bytes
            .checked_add(schedule_bytes)
            .ok_or(ReadError::SizeOverflow)?;
        return Err(ReadError::limit(
            crate::raw::ReadResource::MetadataBytes,
            options.max_metadata_bytes,
            required,
        ));
    }

    let read_text = |path: &Path, size| {
        let bytes = crate::io::read_sized_file(path, size)?;
        String::from_utf8(bytes)
            .map_err(|_| ReadError::corrupt(path.into(), "metadata is not UTF-8"))
    };
    let procpar = read_text(&paths.1, procpar_bytes)?;
    let schedule_text = paths
        .2
        .is_file()
        .then(|| read_text(&paths.2, schedule_bytes))
        .transpose()?;
    let (binary, file_header, block_headers) = LayoutPlan::parse(&paths.0, source_len, options)?;

    let mut parameters = parse_procpar(&procpar, paths.1.clone().into())?;
    parameters.file_header = Some(file_header);
    parameters.block_headers = block_headers;
    if schedule_text.is_none() && scalar_parameter(&parameters, "nus")?.is_some_and(nus_enabled) {
        return Err(ReadError::incomplete(
            paths.2.clone().into(),
            "Varian NUS dataset requires sampling.sch",
        ));
    }

    let layout = resolve_layout_from_procpar(
        &parameters,
        binary.direct_points,
        binary.trace_count,
        schedule_text.is_some(),
        options.assertions.as_ref(),
    )?;
    validate_assertions(
        options.assertions.as_ref(),
        binary.complex,
        binary.trace_count,
        &layout,
    )?;
    let order = select_trace_order(&parameters, &layout, None)?;
    let sampling = schedule_text
        .as_deref()
        .map(|text| parse_schedule(text, &layout.shape, &paths.2.clone().into()))
        .transpose()?;
    let indirect_lanes = layout.component_lanes[..layout.component_lanes.len() - 1].to_vec();
    let component_count = crate::checked_product(&indirect_lanes).ok_or(ReadError::SizeOverflow)?;
    let logical_trace_count = sampling.as_ref().map_or_else(
        || {
            crate::checked_product(&layout.shape[..layout.shape.len() - 1])
                .ok_or(ReadError::SizeOverflow)
        },
        |schedule| Ok(schedule.coordinates().len()),
    )?;
    let expected_traces = logical_trace_count
        .checked_mul(component_count)
        .ok_or(ReadError::SizeOverflow)?;
    if expected_traces != binary.trace_count {
        return Err(ReadError::invalid_metadata(
            paths.1.clone().into(),
            None::<String>,
            format!(
                "procpar/schedule describes {expected_traces} traces but fid stores {}",
                binary.trace_count
            ),
        ));
    }
    let trace_mapping = canonical_to_disk_mapping(
        &layout.shape[..layout.shape.len() - 1],
        &indirect_lanes,
        order,
        layout.asserted_trace_permutation.as_deref(),
    )?;
    let working_bytes = component_count
        .checked_mul(binary.trace_bytes)
        .and_then(|raw| {
            component_count
                .checked_mul(binary.direct_points)?
                .checked_mul(std::mem::size_of::<Complex64>())?
                .checked_add(raw)
        })
        .ok_or(ReadError::SizeOverflow)?;
    if working_bytes > options.max_decode_bytes {
        return Err(ReadError::limit(
            crate::raw::ReadResource::TraceBytes,
            options.max_decode_bytes,
            working_bytes,
        ));
    }

    let mut sources = vec![
        SourceFile::new(SourceKind::Data, "fid", &paths.0)?,
        SourceFile::from_consumed_bytes(
            SourceKind::Parameters,
            "procpar",
            &paths.1,
            procpar.as_bytes(),
        ),
    ];
    if let Some(text) = &schedule_text {
        sources.push(SourceFile::from_consumed_bytes(
            SourceKind::SamplingSchedule,
            "sampling_schedule",
            &paths.2,
            text.as_bytes(),
        ));
    }
    let descriptor = descriptor_from_layout(&parameters, &layout, binary.complex)?;
    options.limits.validate_descriptor(&descriptor)?;
    let schedule_positions = sampling.as_ref().map(|schedule| {
        schedule
            .coordinates()
            .iter()
            .enumerate()
            .map(|(index, coordinate)| (coordinate.as_slice().to_vec(), index))
            .collect::<BTreeMap<_, _>>()
    });
    let source = TraceReader::open(
        paths.0,
        binary,
        layout.shape[..layout.shape.len() - 1].to_vec(),
        indirect_lanes,
        trace_mapping,
        schedule_positions,
    )?;
    Ok(crate::io::Reader::new(
        descriptor,
        RawProvenance::reader(
            RawFormat::VarianRaw,
            sources,
            VendorMetadata::varian(parameters),
        ),
        sampling,
        Box::new(source),
        options.max_region_bytes,
        options.max_materialized_bytes,
        options.limits.working_bytes(),
    ))
}
