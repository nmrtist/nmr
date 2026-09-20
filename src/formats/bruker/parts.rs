use super::decode_2d::TwoDimensionalPaths;
use super::decode_2d::decode_two_dimensional;
use super::decode_3d::decode_three_dimensional;
use super::metadata;
use super::opening::source_table_storage;
use super::parameters::ParameterFile;
use super::raw_assembly::decode_one_dimensional;
use super::semantics::require_experimental_nus;
use super::storage::ensure_decode_limit;
use super::storage::ensure_parameter_storage;
use super::storage::reserve_parameter_files;
use crate::Acquisition;
use crate::Complex64;
use crate::ReadError;
use crate::SamplingCoordinate;
use crate::SourceFile;
use crate::SparseTrace;
use crate::provenance::SourceKind;
use crate::raw::InputSource;
use crate::raw::RawFormat;
use crate::raw::ReadLimits;
use std::path::Path;

/// Decodes a Bruker acquisition from in-memory binary and metadata parts.
///
/// Acquisition status texts are direct-first: `acqus`, then `acqu2s` and
/// `acqu3s` when present. A `nuslist` is required for sparse 2D `FnTYPE=2`
/// data and rejected for traditional data.
pub(crate) fn read_from_parts(
    data: &[u8],
    acquisition_parameters: &[&str],
    nuslist: Option<&str>,
) -> Result<Acquisition, ReadError> {
    read_from_parts_with_options(
        data,
        acquisition_parameters,
        nuslist,
        &ReadLimits::default(),
    )
}

/// Borrowed in-memory parts of one Bruker raw acquisition.
#[derive(Clone, Copy, Debug)]
pub struct Parts<'a> {
    pub(super) data: &'a [u8],
    pub(super) acquisition_parameters: &'a [&'a str],
    pub(super) nuslist: Option<&'a str>,
    pub(super) experimental_vendor_semantics: bool,
}

impl<'a> Parts<'a> {
    /// Creates parts from binary data and direct-first acquisition status texts.
    pub fn new(data: &'a [u8], acquisition_parameters: &'a [&'a str]) -> Self {
        Self {
            data,
            acquisition_parameters,
            nuslist: None,
            experimental_vendor_semantics: false,
        }
    }

    /// Adds the `nuslist` text required by an NUS acquisition.
    pub fn nuslist(mut self, value: &'a str) -> Self {
        self.nuslist = Some(value);
        self
    }

    /// Opts into experimental NUS schedule and preallocation interpretations.
    /// Dense layouts do not require this option. It does not restore the retired
    /// digital-filter fallback table.
    pub fn allow_experimental_vendor_semantics(mut self, value: bool) -> Self {
        self.experimental_vendor_semantics = value;
        self
    }
}

/// Decodes checked in-memory Bruker parts.
pub fn read_parts(parts: Parts<'_>) -> Result<Acquisition, ReadError> {
    if parts.nuslist.is_some() {
        require_experimental_nus(
            parts.experimental_vendor_semantics,
            InputSource::memory("nuslist"),
        )
        .map_err(|error| error.with_format(RawFormat::BrukerRaw))?;
    }
    read_from_parts(parts.data, parts.acquisition_parameters, parts.nuslist)
}

/// Decodes borrowed Bruker parts with explicit read limits.
pub fn read_parts_with_limits(
    parts: Parts<'_>,
    limits: ReadLimits,
) -> Result<Acquisition, ReadError> {
    if parts.nuslist.is_some() {
        require_experimental_nus(
            parts.experimental_vendor_semantics,
            InputSource::memory("nuslist"),
        )
        .map_err(|error| error.with_format(RawFormat::BrukerRaw))?;
    }
    read_from_parts_with_options(
        parts.data,
        parts.acquisition_parameters,
        parts.nuslist,
        &limits,
    )
}

pub(crate) fn parts_nus_storage(observations: usize) -> Result<usize, ReadError> {
    // Retained schedule coordinates, its temporary uniqueness references,
    // output SparseTrace containers and their independent coordinate copies.
    // These phases are conservatively summed; sample payload is charged separately.
    let per_observation = std::mem::size_of::<SamplingCoordinate>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<&SamplingCoordinate>()
        + std::mem::size_of::<SparseTrace>()
        + std::mem::size_of::<usize>();
    observations
        .checked_mul(per_observation)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<usize>()))
        .ok_or(ReadError::SizeOverflow)
}

/// Decodes Bruker data with explicit resource limits.
pub(crate) fn read_from_parts_with_options(
    data: &[u8],
    acquisition_parameters: &[&str],
    nuslist: Option<&str>,
    options: &ReadLimits,
) -> Result<Acquisition, ReadError> {
    (|| {
        let role = if acquisition_parameters.len() == 1 {
            "fid"
        } else {
            "ser"
        };
        let empty_path = Path::new("");
        let (source_count, source_bytes) = source_table_storage(
            std::iter::once((role, empty_path))
                .chain(acquisition_parameters.iter().enumerate().map(|(index, _)| {
                    (
                        match index {
                            0 => "acqus",
                            1 => "acqu2s",
                            _ => "acqu3s",
                        },
                        empty_path,
                    )
                }))
                .chain(nuslist.map(|_| ("sampling_schedule", empty_path))),
        )?;
        // InputSource roles in the inner context, the retained direct context
        // clone, and at most one parameter parser's owned context.
        let context_bytes =
            "acqusacqu2sacqu3sfid or sernuslist".len() + "acqus".len() + "acqu3s".len();
        let options = options.with_retained_working(
            source_bytes
                .checked_add(context_bytes)
                .ok_or(ReadError::SizeOverflow)?,
        )?;
        let dataset =
            read_from_parts_with_options_inner(data, acquisition_parameters, nuslist, &options)?;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(source_count)
            .map_err(|_| ReadError::allocation(source_count * std::mem::size_of::<SourceFile>()))?;
        sources.push(SourceFile::from_consumed_bytes(
            SourceKind::Data,
            role,
            empty_path,
            data,
        ));
        for (index, text) in acquisition_parameters.iter().enumerate() {
            let role = match index {
                0 => "acqus",
                1 => "acqu2s",
                2 => "acqu3s",
                _ => unreachable!("PARMODE and parameter count were validated"),
            };
            sources.push(SourceFile::from_consumed_bytes(
                SourceKind::Parameters,
                role,
                empty_path,
                text.as_bytes(),
            ));
        }
        if let Some(text) = nuslist {
            sources.push(SourceFile::from_consumed_bytes(
                SourceKind::SamplingSchedule,
                "sampling_schedule",
                empty_path,
                text.as_bytes(),
            ));
        }
        Ok(dataset.with_reader_sources(sources))
    })()
    .map_err(|error: ReadError| error.with_format(RawFormat::BrukerRaw))
}

pub(crate) fn read_from_parts_with_options_inner(
    data: &[u8],
    acquisition_parameters: &[&str],
    nuslist: Option<&str>,
    options: &ReadLimits,
) -> Result<Acquisition, ReadError> {
    let parameter_sources = [
        InputSource::memory("acqus"),
        InputSource::memory("acqu2s"),
        InputSource::memory("acqu3s"),
    ];
    let source_count = acquisition_parameters.len().min(parameter_sources.len());
    let data_source = InputSource::memory("fid or ser");
    let nuslist_source = InputSource::memory("nuslist");
    read_parts_with_context(
        data,
        acquisition_parameters,
        nuslist,
        options,
        &data_source,
        &parameter_sources[..source_count],
        &nuslist_source,
    )
}

pub(crate) fn read_parts_with_context(
    data: &[u8],
    acquisition_parameters: &[&str],
    nuslist: Option<&str>,
    options: &ReadLimits,
    data_source: &InputSource,
    parameter_sources: &[InputSource],
    nuslist_source: &InputSource,
) -> Result<Acquisition, ReadError> {
    if data.len() > options.source_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::SourceBytes,
            options.source_bytes(),
            data.len(),
        ));
    }
    let metadata_bytes = acquisition_parameters
        .iter()
        .try_fold(nuslist.map_or(0, str::len), |total, text| {
            total.checked_add(text.len()).ok_or(ReadError::SizeOverflow)
        })?;
    if metadata_bytes > options.metadata_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::MetadataBytes,
            options.metadata_bytes(),
            metadata_bytes,
        ));
    }
    if acquisition_parameters.is_empty() {
        return Err(ReadError::incomplete(
            InputSource::memory("acqus"),
            "Bruker acquisition requires acqus parameters",
        ));
    }
    let parameter_bytes =
        ensure_parameter_storage(acquisition_parameters.iter().copied(), 0, options)?;
    let retained_options = options.with_retained_working(parameter_bytes)?;
    let options = &retained_options;
    let direct_source = parameter_sources
        .first()
        .cloned()
        .unwrap_or_else(|| InputSource::memory("acqus"));
    let direct = ParameterFile::parse_at_source(acquisition_parameters[0], direct_source.clone())?;
    let parmode = direct.usize("PARMODE", &direct_source)?;
    if parmode > 2 {
        return Err(ReadError::unsupported(
            direct_source,
            format!(
                "PARMODE={parmode}; this release validates one-, two-, and selected three-dimensional raw data"
            ),
        ));
    }
    if acquisition_parameters.len() != parmode + 1 {
        return Err(ReadError::incomplete(
            InputSource::memory("acquisition parameters"),
            format!(
                "PARMODE={parmode} requires {} acquisition status file(s), found {}",
                parmode + 1,
                acquisition_parameters.len()
            ),
        ));
    }
    let mut parameter_files = reserve_parameter_files(parmode + 1)?;
    parameter_files.push(direct);
    for (index, (text, source)) in acquisition_parameters
        .iter()
        .zip(parameter_sources)
        .enumerate()
        .skip(1)
    {
        let indirect_source = source.clone();
        debug_assert_eq!(index, parameter_files.len());
        parameter_files.push(ParameterFile::parse_at_source(text, indirect_source)?);
    }
    let descriptor_bytes = metadata::descriptor_storage_bound(&parameter_files)?
        .checked_add((parmode + 1) * 5 * std::mem::size_of::<usize>())
        .ok_or(ReadError::SizeOverflow)?;
    // Besides the descriptor, Parts keeps a separate data layout and may keep
    // shape/lane vectors while building the two-dimensional output.
    let descriptor_options = options.with_retained_working(descriptor_bytes)?;
    let options = &descriptor_options;
    let dataset = match parmode {
        0 => {
            if nuslist.is_some() {
                return Err(ReadError::unsupported_code(
                    nuslist_source.clone(),
                    crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
                    "nuslist is not valid for one-dimensional data",
                ));
            }
            let td = parameter_files[0].usize("TD", &direct_source)?;
            let decoded_bytes = td
                .checked_div(2)
                .and_then(|points| points.checked_mul(std::mem::size_of::<Complex64>()))
                .ok_or(ReadError::SizeOverflow)?;
            ensure_decode_limit(decoded_bytes, options)?;
            decode_one_dimensional(
                data,
                parameter_files
                    .into_iter()
                    .next()
                    .expect("one file checked"),
                &direct_source,
                data_source,
            )
        }
        1 => decode_two_dimensional(
            data,
            parameter_files,
            nuslist,
            options,
            TwoDimensionalPaths {
                ser: data_source,
                acqus: &parameter_sources[0],
                acqu2s: &parameter_sources[1],
                nuslist: nuslist_source,
            },
        ),
        2 => decode_three_dimensional(
            data,
            parameter_files,
            nuslist,
            options,
            parameter_sources,
            data_source,
            nuslist_source,
        ),
        _ => unreachable!("PARMODE was bounded"),
    }?;
    options.validate_descriptor(dataset.descriptor())?;
    Ok(dataset)
}
