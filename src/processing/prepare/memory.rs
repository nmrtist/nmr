//! Allocation-free payload accounting for copied processing metadata containers.
//! Axis backing and processing-state arrays are accounted by processing.rs.

use crate::CancellationToken;

fn check_cancel(token: Option<&CancellationToken>) -> Result<(), ProcessingError> {
    if let Some(token) = token {
        token.check()?;
    }
    Ok(())
}

use crate::processed::{ProcessedDescriptor, ProcessedOrigin, RawDatasetSnapshot, SourceMetadata};
use crate::processing::contracts::error::ProcessingError;
use crate::processing::contracts::history::{HistoryInput, ProcessingHistory, ProcessingRecord};
use crate::processing::contracts::operation::ResolvedOperation;
use crate::provenance::{
    InputAxisRef, ProcessedReadRecord, SampleNormalization, SourceFile, SourceId,
};
use crate::raw::{RawAxis, RawDataset, RawDescriptor, SamplingCoordinate, SamplingSchedule};

pub(crate) fn sum(values: impl IntoIterator<Item = usize>) -> Result<usize, ProcessingError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| total.checked_add(value))
        .ok_or(ProcessingError::SizeOverflow)
}

pub(crate) fn array<T>(count: usize) -> Result<usize, ProcessingError> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(ProcessingError::SizeOverflow)
}

fn text(value: Option<&str>) -> usize {
    value.map_or(0, str::len)
}

pub(crate) fn sources(values: &[SourceFile]) -> Result<usize, ProcessingError> {
    sources_with_cancellation(values, None)
}

pub(crate) fn sources_with_cancellation(
    values: &[SourceFile],
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    values
        .iter()
        .try_fold(array::<SourceFile>(values.len())?, |total, source| {
            check_cancel(token)?;
            sum([
                total,
                source.role().len(),
                source.path().as_os_str().as_encoded_bytes().len(),
            ])
        })
}

fn normalization(value: &SampleNormalization) -> Result<usize, ProcessingError> {
    array::<f64>(value.source_block_scale_factors().len())
}

fn schedule(value: Option<&SamplingSchedule>) -> Result<usize, ProcessingError> {
    schedule_with_cancellation(value, None)
}

fn schedule_with_cancellation(
    value: Option<&SamplingSchedule>,
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    let Some(value) = value else {
        return Ok(0);
    };
    value.coordinates().iter().try_fold(
        sum([
            array::<usize>(value.grid().len())?,
            array::<SamplingCoordinate>(value.coordinates().len())?,
        ])?,
        |total, coordinate| {
            check_cancel(token)?;
            sum([total, array::<usize>(coordinate.as_slice().len())?])
        },
    )
}

fn raw_descriptor(value: &RawDescriptor) -> Result<usize, ProcessingError> {
    raw_descriptor_with_cancellation(value, None)
}

fn raw_descriptor_with_cancellation(
    value: &RawDescriptor,
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    let mut bytes = sum([
        array::<RawAxis>(value.axes().len())?,
        array::<usize>(value.layout().logical_shape().len())?,
        array::<usize>(value.layout().lane_counts().len())?,
        array::<usize>(value.layout().absolute_origin().len())?,
    ])?;
    for axis in value.axes() {
        check_cancel(token)?;
        let coordinates = match axis.coordinates() {
            crate::axis::AxisCoordinates::Explicit(values) => array::<f64>(values.len())?,
            _ => 0,
        };
        bytes = sum([bytes, text(axis.label()), text(axis.nucleus()), coordinates])?;
    }
    let metadata = value.acquisition();
    bytes = sum([
        bytes,
        text(metadata.title()),
        text(metadata.solvent()),
        text(metadata.pulse_program()),
    ])?;
    if let Some(value) = metadata.diffusion() {
        bytes = sum([
            bytes,
            value.gradient_parameter().len(),
            value.gradient_pulse_duration_parameter().len(),
            value.diffusion_time_parameter().len(),
            text(value.recovery_delay_parameter()),
            text(value.gradient_shape()),
            text(value.gradient_shape_parameter()),
        ])?;
    }
    Ok(bytes)
}

pub(crate) fn raw_input(value: &RawDataset) -> Result<usize, ProcessingError> {
    raw_input_with_cancellation(value, None)
}

pub(crate) fn raw_input_with_cancellation(
    value: &RawDataset,
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    sum([
        raw_descriptor_with_cancellation(value.descriptor(), token)?,
        schedule_with_cancellation(value.sampling_schedule(), token)?,
        array::<usize>(value.data().layout().absolute_origin().len())?,
    ])
}

pub(crate) fn raw_snapshot(value: &RawDataset) -> Result<usize, ProcessingError> {
    raw_snapshot_with_cancellation(value, None)
}

pub(crate) fn raw_snapshot_with_cancellation(
    value: &RawDataset,
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    sum([
        array::<RawDatasetSnapshot>(1)?,
        raw_input_with_cancellation(value, token)?,
        sources_with_cancellation(value.provenance().sources(), token)?,
        normalization(value.provenance().sample_normalization())?,
    ])
}

pub(crate) fn snapshot(value: &RawDatasetSnapshot) -> Result<usize, ProcessingError> {
    sum([
        array::<RawDatasetSnapshot>(1)?,
        raw_descriptor(value.descriptor())?,
        sources(value.sources())?,
        normalization(value.sample_normalization())?,
        schedule(value.sampling_schedule())?,
        array::<usize>(value.absolute_origin().len())?,
    ])
}

pub(crate) fn origin(value: &ProcessedOrigin) -> Result<usize, ProcessingError> {
    match value {
        ProcessedOrigin::Library(boundary) => boundary.metadata_bytes(),
        ProcessedOrigin::DeclaredRaw {
            snapshot: value,
            axis_lineage,
        } => sum([snapshot(value)?, array::<InputAxisRef>(axis_lineage.len())?]),
        ProcessedOrigin::DerivedRaw(value) => sum([
            snapshot(value.snapshot())?,
            array::<InputAxisRef>(value.axis_lineage().len())?,
        ]),
        ProcessedOrigin::External(boundary) => {
            let parent = boundary.parent();
            let p = parent.provenance();
            let d = boundary.declaration();
            sum([
                std::mem::size_of_val(boundary.as_ref()),
                descriptor(parent.descriptor())?,
                descriptor(boundary.descriptor())?,
                origin(p.origin())?,
                sources(p.sources())?,
                p.history().map(history).transpose()?.unwrap_or(0),
                source_metadata(p.source_metadata())?,
                read_record(p.read_record())?,
                aggregate(parent.metadata())?,
                array::<crate::external::ExternalAxisSource>(boundary.axes().len())?,
                d.algorithm().len(),
                d.version().len(),
                d.statement().len(),
                d.parameter_format().len(),
                d.parameters().len(),
            ])
        }
        ProcessedOrigin::Unknown | ProcessedOrigin::Imported => Ok(0),
    }
}

pub(crate) fn read_record(value: Option<&ProcessedReadRecord>) -> Result<usize, ProcessingError> {
    let Some(value) = value else {
        return Ok(0);
    };
    value.component_indices().iter().try_fold(
        sum([
            array::<SourceId>(value.components().len())?,
            array::<Vec<usize>>(value.component_indices().len())?,
        ])?,
        |total, indices| sum([total, array::<usize>(indices.len())?]),
    )
}

pub(crate) fn input(value: &HistoryInput) -> Result<usize, ProcessingError> {
    match value {
        HistoryInput::Raw {
            sources: values,
            normalization: scale,
            ..
        } => sum([sources(values)?, normalization(scale)?]),
        HistoryInput::Processed {
            sources: values,
            read_record: record,
            ..
        } => sum([sources(values)?, read_record(record.as_ref())?]),
    }
}

pub(crate) fn raw_binding_with_cancellation(
    value: &RawDataset,
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    sum([
        sources_with_cancellation(value.provenance().sources(), token)?,
        normalization(value.provenance().sample_normalization())?,
    ])
}

pub(crate) fn descriptor(value: &ProcessedDescriptor) -> Result<usize, ProcessingError> {
    array::<crate::processed::ProcessedAxis>(value.axes().len())
}

pub(crate) fn record(value: &ProcessingRecord) -> Result<usize, ProcessingError> {
    let descriptors = match value.descriptor_transition() {
        Some((before, after)) => sum([
            descriptor(before)?,
            descriptor(after)?,
            array::<ResolvedOperation>(1)?,
        ])?,
        None => 0,
    };
    sum([
        descriptors,
        match value.resolved() {
            Some(ResolvedOperation::EstimatedBaseline {
                values,
                coefficients,
            }) => array::<f64>(sum([values.len(), coefficients.len()])?)?,
            _ => 0,
        },
        array::<crate::processing::ProcessingDiagnostic>(value.diagnostics().len())?,
    ])
}

pub(crate) fn records(values: &[ProcessingRecord]) -> Result<usize, ProcessingError> {
    values
        .iter()
        .try_fold(array::<ProcessingRecord>(values.len())?, |total, value| {
            sum([total, record(value)?])
        })
}

pub(crate) fn history(value: &ProcessingHistory) -> Result<usize, ProcessingError> {
    let mut bytes = sum([
        descriptor(value.initial_descriptor())?,
        array::<HistoryInput>(value.inputs().len())?,
        array::<InputAxisRef>(value.axis_lineage().len())?,
        records(value.records())?,
        array::<crate::processing::ExecutionSegment>(value.segments().len())?,
    ])?;
    for segment in value.segments() {
        let e = segment.environment();
        bytes = sum([
            bytes,
            e.crate_version().len(),
            e.architecture().len(),
            e.operating_system().len(),
            e.explicit_fft_backend().len(),
            text(e.build_identifier()),
            text(e.numerical_dependency_versions()),
            text(e.floating_point_configuration()),
        ])?;
    }
    for binding in value.inputs() {
        bytes = sum([bytes, input(binding)?])?;
    }
    Ok(bytes)
}

pub(crate) fn source_metadata(value: &SourceMetadata) -> Result<usize, ProcessingError> {
    let (pairs, documents) = if let Some(value) = value.bruker_topspin() {
        (value.parameters(), value.documents())
    } else if let Some(value) = value.jcamp_dx() {
        (value.labels(), value.records())
    } else {
        // JEOL parameters and evidence are already resident shared Arc backing.
        return Ok(0);
    };
    pairs.iter().try_fold(
        sum([
            array::<(String, String)>(pairs.len())?,
            array::<crate::processed::SourceParameterText>(documents.len())?,
        ])?,
        |total, (key, value)| sum([total, key.len(), value.len()]),
    )
}

pub(crate) fn aggregate(value: &crate::dataset::DatasetMetadata) -> Result<usize, ProcessingError> {
    aggregate_with_cancellation(value, None)
}

pub(crate) fn aggregate_with_cancellation(
    value: &crate::dataset::DatasetMetadata,
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    let identity = value.identity();
    let mut bytes = sum([
        text(identity.subject()),
        text(identity.acquisition()),
        text(identity.source_label()),
        value.selection().map_or(0, |selection| {
            selection.path().as_os_str().as_encoded_bytes().len()
        }),
        array::<crate::ReadWarning>(value.warnings().len())?,
    ])?;
    for warning in value.warnings() {
        check_cancel(token)?;
        if let crate::ReadWarning::MissingOptionalSource { role, path, .. } = warning {
            bytes = sum([bytes, role.len(), path.as_os_str().as_encoded_bytes().len()])?;
        }
    }
    Ok(bytes)
}

fn new_records(rank: usize, count: usize) -> Result<usize, ProcessingError> {
    sum([
        array::<ProcessingRecord>(count)?,
        array::<ResolvedOperation>(count)?,
        array::<crate::processed::ProcessedAxis>(
            count
                .checked_mul(2)
                .and_then(|n| n.checked_mul(rank))
                .ok_or(ProcessingError::SizeOverflow)?,
        )?,
    ])
}

fn history_shell(rank: usize, segments: usize) -> Result<usize, ProcessingError> {
    // finish_segment uses Vec::push: reserve the maximum geometric growth plus
    // the previous allocation during reallocation, independent of allocator reuse.
    let segment_capacity = segments
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .map(|n| n.max(4))
        .ok_or(ProcessingError::SizeOverflow)?;
    sum([
        array::<HistoryInput>(1)?,
        array::<InputAxisRef>(rank)?,
        array::<crate::processing::ExecutionSegment>(segment_capacity)?,
        array::<crate::processing::ExecutionSegment>(segments)?,
    ])
}

pub(crate) fn raw_apply(value: &RawDataset, count: usize) -> Result<usize, ProcessingError> {
    raw_apply_with_cancellation(value, count, None)
}

pub(crate) fn raw_apply_with_cancellation(
    value: &RawDataset,
    count: usize,
    token: Option<&CancellationToken>,
) -> Result<usize, ProcessingError> {
    check_cancel(token)?;
    let rank = value.descriptor().axes().len();
    sum([
        raw_input_with_cancellation(value, token)?,
        raw_snapshot_with_cancellation(value, token)?,
        array::<InputAxisRef>(rank)?,
        sources_with_cancellation(value.provenance().sources(), token)?,
        raw_binding_with_cancellation(value, token)?,
        new_records(rank, count)?,
        history_shell(rank, 0)?,
        array::<crate::processed::ProcessedAxis>(rank)?,
    ])
}

fn processed_provenance(
    value: &crate::processed::ProcessedDataset,
) -> Result<usize, ProcessingError> {
    let p = value.provenance();
    sum([
        origin(p.origin())?,
        sources(p.sources())?,
        read_record(p.read_record())?,
        source_metadata(p.source_metadata())?,
    ])
}

pub(crate) fn processed_apply(
    value: &crate::processed::ProcessedDataset,
    count: usize,
) -> Result<usize, ProcessingError> {
    let rank = value.descriptor().axes().len();
    let p = value.provenance();
    let (seed, old_count, segments) = if let Some(h) = p.history() {
        (
            sum([
                descriptor(h.initial_descriptor())?,
                input(h.input())?,
                records(h.records())?,
            ])?,
            h.records().len(),
            h.segments().len(),
        )
    } else {
        (
            sum([
                descriptor(value.descriptor())?,
                sources(p.sources())?,
                read_record(p.read_record())?,
            ])?,
            0,
            0,
        )
    };
    // The old record allocation can coexist with the exactly reserved combined
    // allocation. New records' descriptors/boxes are charged independently.
    sum([
        seed,
        processed_provenance(value)?,
        new_records(rank, count)?,
        array::<ProcessingRecord>(old_count)?,
        history_shell(rank, segments)?,
    ])
}

pub(crate) fn raw_replay(
    value: &RawDataset,
    h: &ProcessingHistory,
) -> Result<usize, ProcessingError> {
    sum([
        raw_input(value)?,
        raw_snapshot(value)?,
        array::<InputAxisRef>(value.descriptor().axes().len())?,
        sources(value.provenance().sources())?,
        history(h)?,
        history_shell(value.descriptor().axes().len(), 0)?,
    ])
}

pub(crate) fn processed_replay(
    value: &crate::processed::ProcessedDataset,
    h: &ProcessingHistory,
) -> Result<usize, ProcessingError> {
    // Previous and new segment output provenance coexist. Every prefix is
    // bounded by the full borrowed history; this does not multiply by segments.
    let per_output = sum([
        processed_provenance(value)?,
        history(h)?,
        history_shell(h.initial_descriptor().axes().len(), h.segments().len())?,
    ])?;
    sum([per_output, per_output])
}
