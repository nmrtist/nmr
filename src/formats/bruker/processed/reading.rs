use crate::MetadataField;
use crate::ReadLimits;
use crate::ReadWarning;
use crate::WarningImpact;
use crate::processed::ComponentBasis;
use crate::processed::ProcessedData;
use crate::processed::ProcessedDataset;
use crate::processed::ProcessedDescriptor;
use crate::processed::ProcessedOrigin;
use crate::processed::ProcessedProvenance;
use crate::processed::SourceMetadata;
use crate::provenance::SourceFile;
use crate::provenance::SourceKind;
use crate::read_error::ReadError;
use crate::read_error::ReadResource;
use crate::reading::processed::ProcessedRead;
use crate::reading::resolver::ResolvedCandidate;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::axis::axis;
use super::parameters::{Parameters, invalid_parameter, read_metadata};
use super::storage::{Storage, decode_file_recorded};

pub(crate) fn read(
    control: &mut crate::ExecutionContext<'_>,
    candidate: &ResolvedCandidate,
    limits: ReadLimits,
) -> Result<ProcessedRead, ReadError> {
    let direct_path = candidate
        .companion("procs")
        .map(Path::to_path_buf)
        .unwrap_or_else(|| candidate.root.join("procs"));
    if candidate
        .primary
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("2rr"))
    {
        enforce_file_total(
            &[
                direct_path.clone(),
                candidate
                    .companion("proc2s")
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| candidate.root.join("proc2s")),
            ],
            limits.metadata_bytes(),
            ReadResource::MetadataBytes,
        )?;
    }
    let direct_text = read_metadata(control, &direct_path, limits.metadata_bytes())?;
    let direct = Parameters::parse(&direct_text, &direct_path)?;
    if candidate
        .primary
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("2rr"))
    {
        read_2d(control, candidate, limits, direct, direct_path, direct_text)
    } else {
        read_1d(control, candidate, limits, direct, direct_path, direct_text)
    }
}

pub(super) fn read_1d(
    control: &mut crate::ExecutionContext<'_>,
    candidate: &ResolvedCandidate,
    limits: ReadLimits,
    parameters: Parameters,
    parameter_path: PathBuf,
    parameter_text: String,
) -> Result<ProcessedRead, ReadError> {
    let si = parameters.usize("SI", &parameter_path)?;
    let storage = Storage::from_parameters(&parameters, &parameter_path)?;
    let real_path = candidate.primary.clone();
    let imaginary_path = candidate
        .companion("1i")
        .map(Path::to_path_buf)
        .unwrap_or_else(|| candidate.root.join("1i"));
    let mut source_paths = vec![real_path.clone()];
    let has_imaginary = imaginary_path.is_file();
    if has_imaginary {
        source_paths.push(imaginary_path.clone());
    }
    let source_bytes = enforce_source_total(&source_paths, limits.source_bytes())?;
    let component_count = if has_imaginary { 2 } else { 1 };
    let materialized_samples = si
        .checked_mul(component_count)
        .ok_or(ReadError::SizeOverflow)?;
    enforce_materialized(materialized_samples, limits.materialized_bytes())?;
    let decoded_temporary = if has_imaginary {
        materialized_samples
            .checked_mul(std::mem::size_of::<f64>())
            .ok_or(ReadError::SizeOverflow)?
    } else {
        0
    };
    enforce_working_bytes(
        source_bytes
            .checked_add(decoded_temporary)
            .ok_or(ReadError::SizeOverflow)?,
        limits.working_bytes(),
    )?;
    let (real, real_source) = decode_file_recorded(control, &real_path, si, storage, "1r")?;
    let mut sources = vec![real_source];
    let (basis, samples) = if has_imaginary {
        let (imaginary, source) =
            decode_file_recorded(control, &imaginary_path, si, storage, "1i")?;
        sources.push(source);
        let mut samples = reserve_f64(si.checked_mul(2).ok_or(ReadError::SizeOverflow)?)?;
        for (real, imaginary) in real.into_iter().zip(imaginary) {
            samples.extend([real, imaginary]);
        }
        (ComponentBasis::Cartesian, samples)
    } else {
        (ComponentBasis::Scalar, real)
    };
    let mut warnings = Vec::new();
    let axis = axis(
        &parameters,
        &parameter_path,
        si,
        "F1",
        0,
        basis,
        &mut warnings,
    )?;
    let descriptor = ProcessedDescriptor::new(vec![axis])?;
    let data = ProcessedData::new(vec![si], vec![component_count], samples)?;
    sources.push(SourceFile::from_consumed_bytes(
        SourceKind::Parameters,
        "procs",
        &parameter_path,
        parameter_text.as_bytes(),
    ));
    for (ordinal, source) in sources.iter_mut().enumerate() {
        source.assign_id(ordinal);
    }
    let documents = vec![crate::processed::SourceParameterText::new(
        sources.last().unwrap().id().unwrap(),
        parameter_text,
    )];
    let record = crate::provenance::ProcessedReadRecord::bruker(
        storage.nc_proc(),
        storage.dtypp() == 2,
        storage.bytordp() == 1,
        sources
            .iter()
            .filter(|source| source.kind() == SourceKind::Data)
            .map(|source| source.id().expect("reader assigned source IDs"))
            .collect(),
        (0..component_count)
            .map(|component| vec![component])
            .collect(),
        crate::canonical_digest::processed_digests(
            &descriptor,
            &data,
            &crate::processing::contracts::state::PlanState::from_descriptor(&descriptor),
        ),
    );
    let provenance =
        ProcessedProvenance::new(ProcessedOrigin::Imported, sources)?.with_read_record(record);
    let acquisition = parameters.text("EXP");
    if acquisition.is_none() {
        warnings.push(ReadWarning::MissingMetadata {
            field: MetadataField::Acquisition,
            axis: None,
            impact: WarningImpact::Identity,
        });
    }
    let metadata = SourceMetadata::bruker(parameters.into_retained(), documents);
    let dataset = ProcessedDataset::new_imported(descriptor, data, provenance, metadata)?;
    Ok(ProcessedRead {
        dataset,
        acquisition,
        warnings,
    })
}

pub(super) fn read_2d(
    control: &mut crate::ExecutionContext<'_>,
    candidate: &ResolvedCandidate,
    limits: ReadLimits,
    direct: Parameters,
    direct_path: PathBuf,
    direct_text: String,
) -> Result<ProcessedRead, ReadError> {
    let component_paths = ["2ri", "2ir", "2ii"].map(|name| {
        candidate
            .companion(name)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| candidate.root.join(name))
    });
    let present = component_paths.each_ref().map(|p| p.is_file());
    // Only rectangular Cartesian sets have a complete tensor interpretation.
    let counts = match present {
        [false, false, false] => [1, 1],
        [true, false, false] => [2, 1],
        [false, true, false] => [1, 2],
        [true, true, true] => [2, 2],
        _ => {
            return Err(ReadError::incomplete(
                candidate.root.clone().into(),
                "Bruker processed planes do not form a complete Cartesian component set",
            ));
        }
    };
    let indirect_path = candidate
        .companion("proc2s")
        .map(Path::to_path_buf)
        .unwrap_or_else(|| candidate.root.join("proc2s"));
    let indirect_text = read_metadata(control, &indirect_path, limits.metadata_bytes())?;
    let indirect = Parameters::parse(&indirect_text, &indirect_path)?;
    let direct_si = direct.usize("SI", &direct_path)?;
    let indirect_si = indirect.usize("SI", &indirect_path)?;
    let tiles = [
        indirect.usize("XDIM", &indirect_path)?,
        direct.usize("XDIM", &direct_path)?,
    ];
    if tiles.contains(&0) {
        return Err(invalid_parameter(&direct_path, "XDIM"));
    }
    let stored_count = indirect_si
        .div_ceil(tiles[0])
        .checked_mul(direct_si.div_ceil(tiles[1]))
        .and_then(|v| v.checked_mul(tiles[0]))
        .and_then(|v| v.checked_mul(tiles[1]))
        .ok_or(ReadError::SizeOverflow)?;
    let storage = Storage::from_parameters(&direct, &direct_path)?;
    for (name, expected) in [
        ("DTYPP", storage.dtypp()),
        ("BYTORDP", storage.bytordp()),
        ("NC_PROC", storage.nc_proc()),
    ] {
        if let Some(actual) = indirect.optional_i32(name, &indirect_path)? {
            if actual != expected {
                return Err(ReadError::corrupt(
                    indirect_path.clone().into(),
                    format!("{name} conflicts with procs"),
                ));
            }
        }
    }
    let mut data_paths = vec![candidate.primary.clone()];
    data_paths.extend(
        component_paths
            .iter()
            .zip(present)
            .filter_map(|(p, yes)| yes.then_some(p.clone())),
    );
    enforce_source_total(&data_paths, limits.source_bytes())?;
    let count = indirect_si
        .checked_mul(direct_si)
        .ok_or(ReadError::SizeOverflow)?;
    let component_total = counts[0] * counts[1];
    let materialized_count = count
        .checked_mul(component_total)
        .ok_or(ReadError::SizeOverflow)?;
    enforce_materialized(materialized_count, limits.materialized_bytes())?;
    // Final output, one decoded padded plane and its encoded source coexist.
    let source_plane_bytes = stored_count
        .checked_mul(storage.bytes())
        .ok_or(ReadError::SizeOverflow)?;
    let decoded_temporary = stored_count
        .checked_add(materialized_count)
        .and_then(|v| v.checked_mul(std::mem::size_of::<f64>()))
        .ok_or(ReadError::SizeOverflow)?;
    enforce_working_bytes(
        source_plane_bytes
            .checked_add(decoded_temporary)
            .ok_or(ReadError::SizeOverflow)?,
        limits.working_bytes(),
    )?;
    let mut samples = reserve_f64(materialized_count)?;
    samples.resize(materialized_count, 0.0);
    let mut provenance_sources = Vec::with_capacity(component_total + 2);
    let mut component_indices = Vec::with_capacity(component_total);
    for (ordinal, (name, components)) in [
        ("2rr", [0, 0]),
        ("2ri", [1, 0]),
        ("2ir", [0, 1]),
        ("2ii", [1, 1]),
    ]
    .into_iter()
    .enumerate()
    {
        if ordinal > 0 && !present[ordinal - 1] {
            continue;
        }
        let path = if ordinal == 0 {
            &candidate.primary
        } else {
            &component_paths[ordinal - 1]
        };
        let (plane, source) = decode_file_recorded(control, path, stored_count, storage, name)?;
        provenance_sources.push(source);
        component_indices.push(components.to_vec());
        let tile_columns = direct_si.div_ceil(tiles[1]);
        for row in 0..indirect_si {
            control.check_cancelled()?;
            for col in 0..direct_si {
                let source_index =
                    ((row / tiles[0]) * tile_columns + col / tiles[1]) * tiles[0] * tiles[1]
                        + (row % tiles[0]) * tiles[1]
                        + col % tiles[1];
                let output_index = (row * counts[0] + components[0]) * direct_si * counts[1]
                    + col * counts[1]
                    + components[1];
                samples[output_index] = plane[source_index];
            }
        }
    }
    let mut warnings = Vec::new();
    let axis_1 = axis(
        &indirect,
        &indirect_path,
        indirect_si,
        "F1",
        0,
        if counts[0] == 2 {
            ComponentBasis::Cartesian
        } else {
            ComponentBasis::Scalar
        },
        &mut warnings,
    )?;
    let axis_2 = axis(
        &direct,
        &direct_path,
        direct_si,
        "F2",
        1,
        if counts[1] == 2 {
            ComponentBasis::Cartesian
        } else {
            ComponentBasis::Scalar
        },
        &mut warnings,
    )?;
    let descriptor = ProcessedDescriptor::new(vec![axis_1, axis_2])?;
    let data = ProcessedData::new(vec![indirect_si, direct_si], counts.to_vec(), samples)?;
    provenance_sources.extend([
        SourceFile::from_consumed_bytes(
            SourceKind::Parameters,
            "procs",
            &direct_path,
            direct_text.as_bytes(),
        ),
        SourceFile::from_consumed_bytes(
            SourceKind::Parameters,
            "proc2s",
            &indirect_path,
            indirect_text.as_bytes(),
        ),
    ]);
    for (ordinal, source) in provenance_sources.iter_mut().enumerate() {
        source.assign_id(ordinal);
    }
    let documents = vec![
        crate::processed::SourceParameterText::new(
            provenance_sources[component_total].id().unwrap(),
            direct_text,
        ),
        crate::processed::SourceParameterText::new(
            provenance_sources[component_total + 1].id().unwrap(),
            indirect_text,
        ),
    ];
    let record = crate::provenance::ProcessedReadRecord::bruker(
        storage.nc_proc(),
        storage.dtypp() == 2,
        storage.bytordp() == 1,
        provenance_sources[..component_total]
            .iter()
            .map(|source| source.id().unwrap())
            .collect(),
        component_indices,
        crate::canonical_digest::processed_digests(
            &descriptor,
            &data,
            &crate::processing::contracts::state::PlanState::from_descriptor(&descriptor),
        ),
    );
    let provenance = ProcessedProvenance::new(ProcessedOrigin::Imported, provenance_sources)?
        .with_read_record(record);
    let acquisition = direct.text("EXP");
    if acquisition.is_none() {
        warnings.push(ReadWarning::MissingMetadata {
            field: MetadataField::Acquisition,
            axis: None,
            impact: WarningImpact::Identity,
        });
    }
    let mut retained = direct.into_retained();
    retained.extend(
        indirect
            .into_retained()
            .into_iter()
            .map(|(name, value)| (format!("F1.{name}"), value)),
    );
    let dataset = ProcessedDataset::new_imported(
        descriptor,
        data,
        provenance,
        SourceMetadata::bruker(retained, documents),
    )?;
    Ok(ProcessedRead {
        dataset,
        acquisition,
        warnings,
    })
}

pub(super) fn enforce_source_total(paths: &[PathBuf], limit: usize) -> Result<usize, ReadError> {
    enforce_file_total(paths, limit, ReadResource::SourceBytes)
}

pub(super) fn enforce_file_total(
    paths: &[PathBuf],
    limit: usize,
    resource: ReadResource,
) -> Result<usize, ReadError> {
    let mut total = 0usize;
    for path in paths {
        let length = fs::metadata(path)
            .map_err(|error| ReadError::io(path.as_path(), error))?
            .len();
        total = total
            .checked_add(usize::try_from(length).map_err(|_| ReadError::SizeOverflow)?)
            .ok_or(ReadError::SizeOverflow)?;
        if total > limit {
            return Err(ReadError::limit(resource, limit, total));
        }
    }
    Ok(total)
}

pub(super) fn enforce_working_bytes(bytes: usize, limit: usize) -> Result<(), ReadError> {
    if bytes > limit {
        return Err(ReadError::limit(ReadResource::WorkingBytes, limit, bytes));
    }
    Ok(())
}

pub(super) fn enforce_materialized(samples: usize, limit: usize) -> Result<(), ReadError> {
    let bytes = samples
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ReadError::SizeOverflow)?;
    if bytes > limit {
        return Err(ReadError::limit(
            ReadResource::MaterializedBytes,
            limit,
            bytes,
        ));
    }
    Ok(())
}

pub(super) fn reserve_f64(count: usize) -> Result<Vec<f64>, ReadError> {
    let bytes = count
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| ReadError::allocation(bytes))?;
    Ok(values)
}
