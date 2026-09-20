use super::context::parameter_error;
use super::layout::LayoutPlan;
use super::metadata;
use super::metadata::AxisLayout;
use super::metadata::acquisition_metadata;
use super::metadata::normalized_axis;
use super::parameters::ParameterFile;
use super::parameters::Parameters;
use super::parser;
use super::reader::TraceReader;
use super::semantics::group_delay;
use super::semantics::indirect_layout;
use super::semantics::indirect_stored_points;
use super::semantics::resolved_descriptor;
use super::storage::direct_storage;
use super::storage::enforce_lazy_trace_limit;
use super::storage::ensure_decode_limit;
use super::storage::ensure_parameter_storage;
use super::storage::reserve_parameter_files;
use super::storage::trace_stride;
use super::storage::validate_zero_padding;
use crate::ReadError;
use crate::SamplingCoordinate;
use crate::SamplingSchedule;
use crate::SourceFile;
use crate::VendorMetadata;
use crate::acquisition::AxisIndex;
use crate::acquisition::DirectSamples;
use crate::acquisition::RawAxisKind;
use crate::provenance::SourceKind;
use crate::raw::ParameterErrorKind;
use crate::raw::RawFormat;
use crate::raw::RawProvenance;
use crate::raw::ReadLimits;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[cfg(test)]
pub(crate) fn open_resolved_acquisition(
    data: &Path,
    parameter_files: &[PathBuf],
    schedule: Option<&Path>,
    options: &ReadLimits,
) -> Result<crate::io::Reader, ReadError> {
    open_resolved_acquisition_declared(
        data,
        parameter_files,
        schedule,
        options,
        None,
        &crate::CancellationToken::new(),
    )
}

pub(crate) fn open_resolved_acquisition_declared(
    data: &Path,
    parameter_files: &[PathBuf],
    schedule: Option<&Path>,
    options: &ReadLimits,
    declaration: Option<&std::sync::Arc<crate::SamplingDeclaration>>,
    cancellation: &crate::CancellationToken,
) -> Result<crate::io::Reader, ReadError> {
    let kind = if data
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("ser"))
    {
        DataKind::Series
    } else {
        DataKind::Fid
    };
    open_resolved_acquisition_inner(
        data,
        Resolved {
            kind,
            data,
            parameter_files,
            schedule: schedule.filter(|path| path.is_file()),
        },
        options,
        declaration,
        cancellation,
    )
    .map_err(|error| error.with_format(RawFormat::BrukerRaw))
}

pub(crate) fn open_resolved_acquisition_inner(
    path: &Path,
    resolved: Resolved<'_>,
    options: &ReadLimits,
    declaration: Option<&std::sync::Arc<crate::SamplingDeclaration>>,
    cancellation: &crate::CancellationToken,
) -> Result<crate::io::Reader, ReadError> {
    let source_len = crate::ensure_file_size(
        resolved.data,
        options.source_bytes(),
        crate::raw::ReadResource::SourceBytes,
    )?;
    let (source_count, source_bytes) = source_table_storage(
        std::iter::once((resolved.data_role(), resolved.data))
            .chain(
                resolved
                    .parameter_files
                    .iter()
                    .enumerate()
                    .map(|(index, path)| {
                        let role = match index {
                            0 => "acqus",
                            1 => "acqu2s",
                            _ => "acqu3s", // Higher ranks are rejected before construction.
                        };
                        (role, path.as_path())
                    }),
            )
            .chain(
                resolved
                    .schedule
                    .iter()
                    .map(|path| ("sampling_schedule", *path)),
            ),
    )?;
    let source_bytes = source_bytes
        .checked_add(resolved.data.as_os_str().as_encoded_bytes().len())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<TraceReader>()))
        .ok_or(ReadError::SizeOverflow)?;
    let source_options = options.with_retained_working(source_bytes)?;
    let options = &source_options;
    let metadata_paths = resolved
        .parameter_files
        .iter()
        .map(PathBuf::as_path)
        .chain(resolved.schedule);
    let metadata_count = resolved
        .parameter_files
        .len()
        .checked_add(usize::from(resolved.schedule.is_some()))
        .ok_or(ReadError::SizeOverflow)?;
    let size_table_bytes = metadata_count
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(ReadError::SizeOverflow)?;
    ensure_decode_limit(size_table_bytes, options)?;
    let metadata_sizes =
        ensure_combined_metadata_size(metadata_paths, metadata_count, options.metadata_bytes())?;
    let text_table_bytes = resolved
        .parameter_files
        .len()
        .checked_mul(std::mem::size_of::<String>())
        .ok_or(ReadError::SizeOverflow)?;
    let owned_text_bytes = metadata_sizes.iter().try_fold(
        text_table_bytes
            .checked_add(size_table_bytes)
            .ok_or(ReadError::SizeOverflow)?,
        |sum, &bytes| sum.checked_add(bytes).ok_or(ReadError::SizeOverflow),
    )?;
    ensure_decode_limit(owned_text_bytes, options)?;
    let mut parameter_texts = Vec::new();
    parameter_texts
        .try_reserve_exact(resolved.parameter_files.len())
        .map_err(|_| ReadError::allocation(text_table_bytes))?;
    for (path, &size) in resolved.parameter_files.iter().zip(&metadata_sizes) {
        parameter_texts.push(read_sized_text(path, size)?);
    }
    let schedule_text = resolved
        .schedule
        .as_ref()
        .map(|path| read_sized_text(path, metadata_sizes[resolved.parameter_files.len()]))
        .transpose()?;
    if parameter_texts.is_empty() {
        return Err(ReadError::incomplete(
            path.into(),
            "Bruker acquisition requires acqus parameters",
        ));
    }

    let parameter_bytes = ensure_parameter_storage(
        parameter_texts.iter().map(String::as_str),
        owned_text_bytes,
        options,
    )?;
    let retained_options = options.with_retained_working(
        parameter_bytes
            .checked_add(owned_text_bytes)
            .ok_or(ReadError::SizeOverflow)?,
    )?;
    let options = &retained_options;
    let direct_path = &resolved.parameter_files[0];
    let direct = ParameterFile::parse_at(&parameter_texts[0], direct_path)?;
    let parmode = direct.usize("PARMODE", direct_path)?;
    if parmode > 2 {
        return Err(ReadError::unsupported(
            direct_path.clone().into(),
            format!(
                "PARMODE={parmode}; this release validates one-, two-, and selected three-dimensional raw data"
            ),
        ));
    }
    if parameter_texts.len() != parmode + 1 {
        return Err(ReadError::incomplete(
            path.into(),
            format!(
                "PARMODE={parmode} requires {} acquisition status file(s), found {}",
                parmode + 1,
                parameter_texts.len()
            ),
        ));
    }
    let mut parameter_files = reserve_parameter_files(parameter_texts.len())?;
    parameter_files.push(direct);
    for (text, path) in parameter_texts.iter().zip(resolved.parameter_files).skip(1) {
        parameter_files.push(ParameterFile::parse_at(text, path)?);
    }

    let mut sources = Vec::new();
    sources
        .try_reserve_exact(source_count)
        .map_err(|_| ReadError::allocation(source_count * std::mem::size_of::<SourceFile>()))?;
    sources.push(SourceFile::new(
        SourceKind::Data,
        resolved.data_role(),
        resolved.data,
    )?);
    for (index, path) in resolved.parameter_files.iter().enumerate() {
        sources.push(SourceFile::from_consumed_bytes(
            SourceKind::Parameters,
            match index {
                0 => "acqus",
                1 => "acqu2s",
                2 => "acqu3s",
                _ => unreachable!("PARMODE was bounded"),
            },
            path,
            parameter_texts[index].as_bytes(),
        ));
    }
    if let Some(path) = &resolved.schedule {
        sources.push(SourceFile::from_consumed_bytes(
            SourceKind::SamplingSchedule,
            "sampling_schedule",
            path,
            schedule_text.as_deref().unwrap_or_default().as_bytes(),
        ));
    }

    let descriptor_bytes = metadata::descriptor_storage_bound(&parameter_files)?;
    let descriptor_options = options.with_retained_working(descriptor_bytes)?;
    let options = &descriptor_options;
    let mut reader_retained_bytes = parameter_bytes
        .checked_add(source_bytes)
        .and_then(|bytes| bytes.checked_add(descriptor_bytes))
        .ok_or(ReadError::SizeOverflow)?;
    let (descriptor, sampling, layout, storage) = match parmode {
        0 => {
            if schedule_text.is_some() || declaration.is_some() {
                return Err(ReadError::unsupported_code(
                    direct_path.clone().into(),
                    crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
                    "nuslist is not valid for one-dimensional data",
                ));
            }
            let storage = direct_storage(&parameter_files[0], direct_path)?;
            let payload_bytes = storage
                .td
                .checked_mul(storage.sample.bytes())
                .ok_or(ReadError::SizeOverflow)?;
            if source_len < payload_bytes {
                return Err(ReadError::truncated(
                    resolved.data.into(),
                    payload_bytes,
                    source_len,
                ));
            }
            validate_zero_padding(resolved.data, payload_bytes, source_len)?;
            let direct_points = storage.td / 2;
            enforce_lazy_trace_limit(payload_bytes, direct_points, 1, options)?;
            let group_delay = group_delay(&parameter_files[0], direct_path)?;
            let axis = normalized_axis(
                &parameter_files[0],
                direct_path,
                AxisLayout {
                    kind: RawAxisKind::Direct(DirectSamples::Complex),
                    points: direct_points,
                    label: "F1",
                    group_delay,
                },
            )?;
            let descriptor = resolved_descriptor(
                vec![axis],
                acquisition_metadata(&parameter_files[0], direct_path)?,
            )?;
            (
                descriptor,
                None,
                LayoutPlan::OneD { payload_bytes },
                storage,
            )
        }
        1 => {
            let acqu2s_path = &resolved.parameter_files[1];
            let indirect = &parameter_files[1];
            if parameter_files[0]
                .optional_integer("AQSEQ", direct_path)?
                .unwrap_or(0)
                != 0
            {
                return Err(ReadError::unsupported_code(
                    direct_path.clone().into(),
                    crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
                    "nonzero AQSEQ is not verified for two-dimensional data",
                ));
            }
            let (indirect_components, indirect_lanes) = indirect_layout(
                indirect.integer("FnMODE", acqu2s_path)?,
                AxisIndex::new(0),
                acqu2s_path,
            )?;
            let stored_indirect = indirect.usize("TD", acqu2s_path)?;
            if stored_indirect < indirect_lanes || stored_indirect % indirect_lanes != 0 {
                return Err(parameter_error(
                    acqu2s_path,
                    Some("TD"),
                    ParameterErrorKind::Invalid,
                    format!("value {stored_indirect}: expected a trace count divisible by the FnMODE lane count {indirect_lanes}"),
                )
                .into());
            }
            let fn_type = parameter_files[0]
                .optional_integer("FnTYPE", direct_path)?
                .unwrap_or(0);
            let (grid_stored_indirect, schedule_text) = match fn_type {
                0 if schedule_text.is_none() && declaration.is_none() => (stored_indirect, None),
                0 => {
                    return Err(ReadError::unsupported_code(
                        direct_path.clone().into(),
                        crate::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT,
                        "nuslist is not valid when acqus FnTYPE=0",
                    ));
                }
                2 => {
                    if schedule_text.is_none() && declaration.is_none() {
                        return Err(ReadError::incomplete(
                            direct_path.clone().into(),
                            "Bruker FnTYPE=2 requires a nuslist",
                        ));
                    }
                    let nus_td = indirect.usize("NusTD", acqu2s_path)?;
                    if nus_td < stored_indirect || nus_td % indirect_lanes != 0 {
                        return Err(parameter_error(
                            acqu2s_path,
                            Some("NusTD"),
                            ParameterErrorKind::Invalid,
                            format!("value {nus_td}: expected a full-grid trace count divisible by {indirect_lanes} and no smaller than TD"),
                        )
                        .into());
                    }
                    (nus_td, schedule_text.as_deref())
                }
                value => {
                    return Err(ReadError::unsupported(
                        direct_path.clone().into(),
                        format!(
                            "acqus FnTYPE={value}; validated modes are 0 (traditional) and 2 (NUS)"
                        ),
                    ));
                }
            };
            let storage = direct_storage(&parameter_files[0], direct_path)?;
            let direct_points = storage.td / 2;
            let payload_bytes = storage
                .td
                .checked_mul(storage.sample.bytes())
                .ok_or(ReadError::SizeOverflow)?;
            let stride = trace_stride(
                &parameter_files[0],
                payload_bytes,
                stored_indirect,
                source_len,
                direct_path,
            )?;
            let expected = stride
                .checked_mul(stored_indirect)
                .ok_or(ReadError::SizeOverflow)?;
            if source_len < expected {
                return Err(ReadError::truncated(
                    resolved.data.into(),
                    expected,
                    source_len,
                ));
            }
            if source_len > expected {
                let allocated = stride
                    .checked_mul(grid_stored_indirect)
                    .ok_or(ReadError::SizeOverflow)?;
                if fn_type != 2 || source_len != allocated {
                    return Err(ReadError::corrupt(
                        resolved.data.into(),
                        "bytes remain beyond the declared ser layout",
                    ));
                }
                validate_zero_padding(resolved.data, expected, source_len)?;
            }
            let logical_indirect = grid_stored_indirect / indirect_lanes;
            let schedule_entries = stored_indirect / indirect_lanes;
            let (nus_retained, nus_peak) = if schedule_text.is_some() || declaration.is_some() {
                lazy_nus_storage(logical_indirect, schedule_entries)?
            } else {
                (0, 0)
            };
            ensure_decode_limit(nus_peak, options)?;
            let nus_options = options.with_retained_working(nus_retained)?;
            enforce_lazy_trace_limit(payload_bytes, direct_points, indirect_lanes, &nus_options)?;
            reader_retained_bytes = reader_retained_bytes
                .checked_add(nus_retained)
                .ok_or(ReadError::SizeOverflow)?;
            let coordinates = schedule_text
                .map(|text| {
                    parser::parse_nuslist(
                        text,
                        logical_indirect,
                        schedule_entries,
                        resolved
                            .schedule
                            .expect("schedule text comes from the resolved nuslist path"),
                    )
                })
                .transpose()?;
            let mut sampling = coordinates
                .map(|coordinates| SamplingSchedule::new(vec![logical_indirect], coordinates))
                .transpose()?;
            if let Some(declaration) = declaration {
                sampling = Some(declaration.resolve(
                    &[logical_indirect],
                    &[indirect_lanes],
                    schedule_entries,
                    sampling.as_ref(),
                    Some(cancellation),
                )?);
            }
            let (sampling, acquisition_rows) = if let Some(sampling) = sampling {
                let requested_bytes = logical_indirect
                    .checked_mul(std::mem::size_of::<Option<usize>>())
                    .ok_or(ReadError::SizeOverflow)?;
                let mut rows = Vec::new();
                rows.try_reserve_exact(logical_indirect)
                    .map_err(|_| ReadError::allocation(requested_bytes))?;
                rows.resize(logical_indirect, None);
                for (acquisition, coordinate) in sampling.coordinates().iter().enumerate() {
                    rows[coordinate.as_slice()[0]] = Some(acquisition);
                }
                (Some(sampling), Some(rows))
            } else {
                (None, None)
            };
            let indirect_axis = normalized_axis(
                indirect,
                acqu2s_path,
                AxisLayout {
                    kind: RawAxisKind::Indirect(indirect_components),
                    points: logical_indirect,
                    label: "F1",
                    group_delay: None,
                },
            )?;
            let group_delay = group_delay(&parameter_files[0], direct_path)?;
            let direct_axis = normalized_axis(
                &parameter_files[0],
                direct_path,
                AxisLayout {
                    kind: RawAxisKind::Direct(DirectSamples::Complex),
                    points: direct_points,
                    label: "F2",
                    group_delay,
                },
            )?;
            let descriptor = resolved_descriptor(
                vec![indirect_axis, direct_axis],
                acquisition_metadata(&parameter_files[0], direct_path)?,
            )?;
            (
                descriptor,
                sampling,
                LayoutPlan::TwoD {
                    stride,
                    payload_bytes,
                    direct_points,
                    indirect_lanes,
                    acquisition_rows,
                },
                storage,
            )
        }
        2 => {
            if schedule_text.is_some() || declaration.is_some() {
                return Err(ReadError::unsupported_code(
                    direct_path.clone().into(),
                    crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
                    "three-dimensional Bruker NUS is not yet supported",
                ));
            }
            if parameter_files[0]
                .optional_integer("FnTYPE", direct_path)?
                .unwrap_or(0)
                != 0
            {
                return Err(ReadError::unsupported_code(
                    direct_path.clone().into(),
                    crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
                    "three-dimensional Bruker FnTYPE must be traditional (0)",
                ));
            }
            if parameter_files[0]
                .optional_integer("AQSEQ", direct_path)?
                .unwrap_or(0)
                != 0
            {
                return Err(ReadError::unsupported_feature(
                    direct_path.clone().into(),
                    crate::raw::UnsupportedFeatureCode::NON_SEPARABLE_COMPONENT_LAYOUT,
                    None,
                    vec![
                        "only Bruker 3D AQSEQ=0 (3-2-1) has a proven separable mapping".to_owned(),
                    ],
                ));
            }
            let fast_path = &resolved.parameter_files[1];
            let slow_path = &resolved.parameter_files[2];
            let fast = &parameter_files[1];
            let slow = &parameter_files[2];
            let (fast_components, fast_lanes) = indirect_layout(
                fast.integer("FnMODE", fast_path)?,
                AxisIndex::new(1),
                fast_path,
            )?;
            let (slow_components, slow_lanes) = indirect_layout(
                slow.integer("FnMODE", slow_path)?,
                AxisIndex::new(0),
                slow_path,
            )?;
            let stored_fast = indirect_stored_points(fast, fast_path, fast_lanes)?;
            let stored_slow = indirect_stored_points(slow, slow_path, slow_lanes)?;
            let storage = direct_storage(&parameter_files[0], direct_path)?;
            let direct_points = storage.td / 2;
            let payload_bytes = storage
                .td
                .checked_mul(storage.sample.bytes())
                .ok_or(ReadError::SizeOverflow)?;
            let rows = stored_slow
                .checked_mul(stored_fast)
                .ok_or(ReadError::SizeOverflow)?;
            let stride = trace_stride(
                &parameter_files[0],
                payload_bytes,
                rows,
                source_len,
                direct_path,
            )?;
            let expected = stride.checked_mul(rows).ok_or(ReadError::SizeOverflow)?;
            if source_len < expected {
                return Err(ReadError::truncated(
                    resolved.data.into(),
                    expected,
                    source_len,
                ));
            }
            if source_len > expected {
                return Err(ReadError::corrupt(
                    resolved.data.into(),
                    "bytes remain beyond the declared 3D ser layout",
                ));
            }
            let indirect_components = slow_lanes
                .checked_mul(fast_lanes)
                .ok_or(ReadError::SizeOverflow)?;
            enforce_lazy_trace_limit(payload_bytes, direct_points, indirect_components, options)?;
            let slow_axis = normalized_axis(
                slow,
                slow_path,
                AxisLayout {
                    kind: RawAxisKind::Indirect(slow_components),
                    points: stored_slow / slow_lanes,
                    label: "F1",
                    group_delay: None,
                },
            )?;
            let fast_axis = normalized_axis(
                fast,
                fast_path,
                AxisLayout {
                    kind: RawAxisKind::Indirect(fast_components),
                    points: stored_fast / fast_lanes,
                    label: "F2",
                    group_delay: None,
                },
            )?;
            let direct_axis = normalized_axis(
                &parameter_files[0],
                direct_path,
                AxisLayout {
                    kind: RawAxisKind::Direct(DirectSamples::Complex),
                    points: direct_points,
                    label: "F3",
                    group_delay: group_delay(&parameter_files[0], direct_path)?,
                },
            )?;
            let descriptor = resolved_descriptor(
                vec![slow_axis, fast_axis, direct_axis],
                acquisition_metadata(&parameter_files[0], direct_path)?,
            )?;
            (
                descriptor,
                None,
                LayoutPlan::ThreeD {
                    stride,
                    payload_bytes,
                    direct_points,
                    stored_second_indirect: stored_fast,
                    slow_lanes,
                    fast_lanes,
                },
                storage,
            )
        }
        _ => unreachable!("PARMODE was bounded"),
    };

    options.validate_descriptor(&descriptor)?;
    let vendor_metadata = VendorMetadata::bruker(Parameters::new(parameter_files));
    let source = TraceReader::open(resolved.data.to_path_buf(), storage, layout)?;
    crate::io::Reader::new(
        descriptor,
        RawProvenance::reader(RawFormat::BrukerRaw, sources, vendor_metadata),
        sampling,
        Box::new(source),
        options.region_bytes(),
        options.materialized_bytes(),
        options.working_bytes(),
    )
    .with_retained_bytes(reader_retained_bytes)
}

#[derive(Clone, Debug)]
pub(crate) struct Resolved<'a> {
    pub(super) kind: DataKind,
    pub(super) data: &'a Path,
    pub(super) parameter_files: &'a [PathBuf],
    pub(super) schedule: Option<&'a Path>,
}

impl Resolved<'_> {
    pub(super) fn data_role(&self) -> &'static str {
        match self.kind {
            DataKind::Fid => "fid",
            DataKind::Series => "ser",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum DataKind {
    Fid,
    Series,
}

// Each entry owns one role and one platform-encoded path. The table is reserved
// exactly once; identities, digests and IDs are inline in SourceFile.
pub(crate) fn source_table_storage<'a>(
    mut entries: impl Iterator<Item = (&'a str, &'a Path)>,
) -> Result<(usize, usize), ReadError> {
    entries.try_fold((0usize, 0usize), |(count, bytes), (role, path)| {
        let bytes = bytes
            .checked_add(std::mem::size_of::<SourceFile>())
            .and_then(|bytes| bytes.checked_add(role.len()))
            .and_then(|bytes| bytes.checked_add(path.as_os_str().as_encoded_bytes().len()))
            .filter(|&bytes| bytes <= isize::MAX as usize)
            .ok_or(ReadError::SizeOverflow)?;
        Ok((count.checked_add(1).ok_or(ReadError::SizeOverflow)?, bytes))
    })
}

pub(crate) fn ensure_combined_metadata_size<'a>(
    paths: impl Iterator<Item = &'a Path>,
    count: usize,
    limit: usize,
) -> Result<Vec<usize>, ReadError> {
    let mut total = 0usize;
    let mut sizes = Vec::new();
    let bytes = count
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(ReadError::SizeOverflow)?;
    sizes
        .try_reserve_exact(count)
        .map_err(|_| ReadError::allocation(bytes))?;
    for path in paths {
        let length = fs::metadata(path)
            .map_err(|error| ReadError::io(path, error))?
            .len();
        let length = usize::try_from(length).map_err(|_| ReadError::SizeOverflow)?;
        total = total.checked_add(length).ok_or(ReadError::SizeOverflow)?;
        sizes.push(length);
        if total > limit {
            return Err(ReadError::limit(
                crate::raw::ReadResource::MetadataBytes,
                limit,
                total,
            ));
        }
    }
    Ok(sizes)
}

pub(crate) fn read_sized_text(path: &Path, size: usize) -> Result<String, ReadError> {
    let bytes = crate::io::read_sized_file(path, size)?;
    String::from_utf8(bytes).map_err(|_| ReadError::corrupt(path.into(), "metadata is not UTF-8"))
}

pub(crate) fn lazy_nus_storage(
    grid: usize,
    observations: usize,
) -> Result<(usize, usize), ReadError> {
    let coordinate_bytes = observations
        .checked_mul(std::mem::size_of::<SamplingCoordinate>() + std::mem::size_of::<usize>())
        .ok_or(ReadError::SizeOverflow)?;
    let rows = grid
        .checked_mul(std::mem::size_of::<Option<usize>>())
        .ok_or(ReadError::SizeOverflow)?;
    let retained = coordinate_bytes
        .checked_add(rows)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<usize>()))
        .ok_or(ReadError::SizeOverflow)?;
    let peak = observations
        .checked_mul(std::mem::size_of::<&SamplingCoordinate>())
        .and_then(|bytes| retained.checked_add(bytes))
        .ok_or(ReadError::SizeOverflow)?;
    Ok((retained, peak))
}
