use super::header::BodyEndian;
use super::header::JdfHeader;
use super::header::Precision;
use super::parts::Parts;
use super::parts::read_from_parts_inner;
use super::semantics::explicit_experiment;
use super::semantics::require_experimental_opt_in;
use crate::Acquisition;
use crate::Domain;
use crate::ReadError;
use crate::SourceFile;
use crate::provenance::SourceKind;
use crate::raw::InputSource;
use crate::raw::ReadLimits;
use std::path::Path;

/// Reads a supported frequency-domain JDF into the processed model.
pub(crate) fn read_processed(
    control: &mut crate::ExecutionContext<'_>,
    path: &Path,
    limits: ReadLimits,
) -> Result<crate::reading::processed::ProcessedRead, ReadError> {
    let source_bytes = crate::ensure_file_size(
        path,
        limits.source_bytes(),
        crate::raw::ReadResource::SourceBytes,
    )?;
    if source_bytes > limits.working_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::WorkingBytes,
            limits.working_bytes(),
            source_bytes,
        ));
    }
    let bytes = crate::io::read_sized_file_controlled(control, path, source_bytes)?;
    let raw = read_from_parts_inner(&bytes, &limits, true, source_bytes)?;
    let source = SourceFile::from_consumed_bytes(SourceKind::Data, "jdf", path, &bytes);
    let header = JdfHeader::parse(&bytes)?;
    // Decoding and conversion do not need simultaneous owned source buffers.
    drop(bytes);
    control.check_cancelled()?;
    finish_processed(raw, source, header, limits)
}

/// Reads supported frequency-domain JDF Parts into the processed model.
pub fn read_processed_parts(
    parts: Parts<'_>,
) -> Result<crate::processed::ProcessedDataset, ReadError> {
    read_processed_parts_with_limits(parts, ReadLimits::default())
}

/// Reads processed Parts with explicit limits, without charging borrowed source bytes as newly owned working memory.
pub fn read_processed_parts_with_limits(
    parts: Parts<'_>,
    limits: ReadLimits,
) -> Result<crate::processed::ProcessedDataset, ReadError> {
    require_experimental_opt_in(
        parts.experimental_vendor_semantics,
        InputSource::memory("jdf"),
    )
    .and_then(|()| processed_parts_inner(parts, limits))
    .map_err(|error| error.with_format(crate::processed::Format::JeolDelta))
}

pub(crate) fn processed_parts_inner(
    parts: Parts<'_>,
    limits: ReadLimits,
) -> Result<crate::processed::ProcessedDataset, ReadError> {
    let raw = read_from_parts_inner(parts.jdf, &limits, true, 0)?;
    let source = SourceFile::from_consumed_bytes(SourceKind::Data, "jdf", Path::new(""), parts.jdf);
    let header = JdfHeader::parse(parts.jdf)?;
    finish_processed(raw, source, header, limits).map(|result| result.dataset)
}

pub(crate) fn finish_processed(
    raw: Acquisition,
    mut source: SourceFile,
    header: JdfHeader,
    limits: ReadLimits,
) -> Result<crate::reading::processed::ProcessedRead, ReadError> {
    let path = source.path().to_path_buf();
    let path = path.as_path();
    let raw_axes = raw.descriptor().axes();
    let parameters = raw
        .provenance()
        .source_metadata()
        .as_jeol()
        .ok_or_else(|| ReadError::corrupt(path.into(), "JEOL metadata was not retained"))?;
    let axis_types = parameters.axis_types();
    if !matches!(axis_types, [1] | [3] | [3, 1] | [3, 3]) {
        return Err(ReadError::unsupported_code(
            path.into(),
            crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
            "unverified JEOL processed section layout",
        ));
    }
    let raw_axis = raw_axes.last().unwrap();
    if raw_axis.domain() != Domain::Frequency {
        return Err(ReadError::unsupported_code(
            path.into(),
            crate::raw::UnsupportedFeatureCode::SPECTRUM_REPRESENTATION,
            "JEOL direct axis is not frequency domain",
        ));
    }
    let component_count = if axis_types[0] == 3 { 2 } else { 1 };
    let axes = raw_axes
        .iter()
        .enumerate()
        .map(|(a, axis)| {
            let count = if a + 1 == raw_axes.len() {
                component_count
            } else {
                axis.component_lanes()
            };
            crate::processed::ProcessedAxis::new(
                if axis.domain() == Domain::Parameter {
                    crate::axis::AxisRole::ArrayParameter
                } else {
                    crate::axis::AxisRole::Signal
                },
                axis.domain(),
                axis.unit(),
                axis.points(),
                axis.coordinates().clone(),
                if count == 2 {
                    crate::processed::ComponentBasis::Cartesian
                } else {
                    crate::processed::ComponentBasis::Scalar
                },
            )?
            .with_label(axis.label().map(str::to_owned))
            .with_quantity(axis.quantity())?
            .with_nucleus(axis.nucleus().map(str::to_owned))?
            .with_spectral_width_hz(axis.spectral_width_hz())?
            .with_frequency_evidence(axis.frequency_evidence())
        })
        .collect::<Result<Vec<_>, crate::processed::ProcessedValidationError>>()?;
    let descriptor = crate::processed::ProcessedDescriptor::new(axes)?;
    let complex = raw
        .data()
        .dense_samples()
        .ok_or_else(|| ReadError::corrupt(path.into(), "processed JEOL data is not dense"))?;
    let sample_count = complex
        .len()
        .checked_mul(component_count)
        .ok_or(ReadError::SizeOverflow)?;
    let result_bytes = sample_count
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ReadError::SizeOverflow)?;
    if result_bytes > limits.materialized_bytes() {
        return Err(ReadError::limit(
            crate::raw::ReadResource::MaterializedBytes,
            limits.materialized_bytes(),
            result_bytes,
        ));
    }
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(sample_count)
        .map_err(|_| ReadError::allocation(result_bytes))?;
    for value in complex {
        samples.push(value.re);
        if component_count == 2 {
            samples.push(value.im);
        }
    }
    let data = crate::processed::ProcessedData::new(
        descriptor.logical_shape(),
        descriptor.component_counts(),
        samples,
    )?;
    source.assign_id(0);
    let source_id = source.id().unwrap();
    let transform = crate::provenance::ProcessedReadTransform::JeolDelta {
        float64: header.precision == Precision::F64,
        big_endian: header.body_endian == BodyEndian::Big,
        disk_points: header.points_disk[0],
        crop_start: header.offset_start[0],
        crop_points: raw_axis.points(),
        imaginary_multiplier: parameters.sample_transform().direct_imaginary_multiplier(),
        submatrix_edge: header.data_format.submatrix_edge(),
        coordinate_scale: 10f64.powi(i32::from(header.raw_axis_units[0].prefix_exponent)),
        explicit_coordinates: header.list_length[0] != 0,
    };
    let digests = crate::canonical_digest::processed_digests(
        &descriptor,
        &data,
        &crate::processing::contracts::state::PlanState::from_descriptor(&descriptor),
    );
    let record = if raw_axes.len() == 1 {
        crate::provenance::ProcessedReadRecord::jeol(source_id, transform, component_count, digests)
    } else {
        crate::provenance::ProcessedReadRecord::jeol_2d(
            source_id,
            crate::provenance::ProcessedReadTransform::JeolDelta2D {
                float64: header.precision == Precision::F64,
                big_endian: header.body_endian == BodyEndian::Big,
                disk_points: [header.points_disk[0], header.points_disk[1]],
                crop_start: [header.offset_start[0], header.offset_start[1]],
                crop_points: [raw_axes[1].points(), raw_axes[0].points()],
                submatrix_edge: header.data_format.submatrix_edge(),
                imaginary_multiplier: parameters.sample_transform().direct_imaginary_multiplier(),
            },
            &descriptor.component_counts(),
            digests,
        )
    };
    let provenance = crate::processed::ProcessedProvenance::new(
        crate::processed::ProcessedOrigin::Imported,
        vec![source],
    )?
    .with_read_record(record);
    let payload_end = header
        .points_disk
        .iter()
        .try_fold(1usize, |n, v| {
            n.checked_mul(*v).ok_or(ReadError::SizeOverflow)
        })?
        .checked_mul(header.sections()?)
        .and_then(|count| count.checked_mul(header.precision.size()))
        .and_then(|bytes| header.data_start.checked_add(bytes))
        .ok_or(ReadError::SizeOverflow)?;
    let acquisition = explicit_experiment(parameters);
    let dataset = crate::processed::ProcessedDataset::new_imported(
        descriptor,
        data,
        provenance,
        crate::processed::SourceMetadata::jeol(parameters.clone(), source_id, payload_end),
    )?;
    let warnings = if acquisition.is_none() {
        vec![crate::ReadWarning::MissingMetadata {
            field: crate::MetadataField::Acquisition,
            axis: None,
            impact: crate::WarningImpact::Identity,
        }]
    } else {
        Vec::new()
    };
    Ok(crate::reading::processed::ProcessedRead {
        dataset,
        acquisition,
        warnings,
    })
}
