use crate::Complex64;
use crate::axis::AxisRole;
use crate::processed::{ProcessedAxis, ProcessedDataset, ProcessedDescriptor, ProcessedOrigin};
use crate::processing::contracts::{
    error::*, history::*, operation::*, options::*, prepared_step::*, profile::*,
};
use crate::processing::kernels::fft::fft_work;
use crate::processing::kernels::spectrum;
use crate::raw::RawDataset;

pub(in crate::processing) fn step_work(step: &PreparedStep) -> Result<u128, ProcessingError> {
    let count = (descriptor_bytes(&step.before)? / 8) as u128;
    let work = match &step.resolved {
        ResolvedOperation::Spectrum(operation) => spectrum::work(
            operation,
            step.before.axes()[step.axis()].points(),
            count as usize,
        )?,
        ResolvedOperation::FourierTransform(_)
        | ResolvedOperation::TimeDomainShiftFoldV1 { .. } => {
            let axis = &step.before.axes()[step.axis()];
            let traces = count / (axis.points() * axis.component_count()) as u128;
            traces
                * fft_work(axis.points())
                * if matches!(
                    step.resolved,
                    ResolvedOperation::TimeDomainShiftFoldV1 { .. }
                ) {
                    2
                } else {
                    1
                }
        }
        ResolvedOperation::BaselineCorrection(_) => count * PositivePeaksV1::MAX_SOLVES as u128,
        ResolvedOperation::ComponentTransform { .. } => count * 2,
        _ => count,
    };
    Ok(work)
}

pub(in crate::processing) fn descriptor_bytes(
    descriptor: &ProcessedDescriptor,
) -> Result<usize, ProcessingError> {
    descriptor
        .axes()
        .iter()
        .try_fold(std::mem::size_of::<f64>(), |bytes, axis| {
            bytes
                .checked_mul(axis.points())?
                .checked_mul(axis.component_count())
        })
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn check_working_limit(
    before: &ProcessedDescriptor,
    after: &ProcessedDescriptor,
    axis: Option<usize>,
    resolved: &ResolvedOperation,
    options: ProcessingOptions,
) -> Result<(), ProcessingError> {
    if matches!(resolved, ResolvedOperation::BaselineCorrection(_)) {
        check_resource(
            crate::resource::ResourceKind::MetadataBytes,
            checked_sum(&[
                metadata_reservation(options)?,
                checked_times(
                    before.axes()[axis.expect("baseline has an axis")].points(),
                    std::mem::size_of::<f64>(),
                )?,
            ])?,
            options.max_metadata_bytes,
        )?;
    }
    check_sample_peak(
        numerical_working_bytes(before, after, axis, resolved)?,
        0,
        options,
    )
}

pub(in crate::processing) fn check_sample_peak(
    peak: usize,
    retained: usize,
    options: ProcessingOptions,
) -> Result<(), ProcessingError> {
    let metadata = metadata_reservation(options)?;
    if metadata > options.max_metadata_bytes {
        return Err(crate::resource::LimitExceeded {
            resource: crate::resource::ResourceKind::MetadataBytes,
            limit: options.max_metadata_bytes,
            required: metadata,
        }
        .into());
    }
    check_resource(
        crate::resource::ResourceKind::WorkingBytes,
        checked_sum(&[peak, retained, metadata])?,
        options.max_working_bytes,
    )
}

pub(in crate::processing) fn check_resource(
    resource: crate::resource::ResourceKind,
    required: usize,
    limit: usize,
) -> Result<(), ProcessingError> {
    if required > limit {
        return Err(crate::resource::LimitExceeded {
            resource,
            required,
            limit,
        }
        .into());
    }
    Ok(())
}

pub(in crate::processing) fn metadata_reservation(
    options: ProcessingOptions,
) -> Result<usize, ProcessingError> {
    checked_sum(&[
        options.prepared_bytes,
        options.axis_bytes,
        options.state_bytes,
        options.container_bytes,
        options.context_bytes,
    ])
}

pub(in crate::processing) fn estimate_resources(
    descriptor: &ProcessedDescriptor,
    steps: &[PreparedStep],
    options: ProcessingOptions,
) -> Result<crate::resource::ResourceEstimate, ProcessingError> {
    let output = descriptor_bytes(descriptor)?;
    let metadata = metadata_reservation(options)?;
    let mut numerical = output;
    let mut coordinate_bytes = 0;
    for step in steps {
        numerical = numerical.max(numerical_working_bytes(
            &step.before,
            &step.after,
            step.axis,
            &step.resolved,
        )?);
        if matches!(step.resolved, ResolvedOperation::BaselineCorrection(_)) {
            coordinate_bytes = coordinate_bytes.max(checked_times(
                step.before.axes()[step.axis()].points(),
                std::mem::size_of::<f64>(),
            )?);
        }
    }
    let metadata_with_coordinates = metadata
        .checked_add(coordinate_bytes)
        .ok_or(ProcessingError::SizeOverflow)?;
    if metadata_with_coordinates > options.max_metadata_bytes {
        return Err(crate::resource::LimitExceeded {
            resource: crate::resource::ResourceKind::MetadataBytes,
            limit: options.max_metadata_bytes,
            required: metadata_with_coordinates,
        }
        .into());
    }
    Ok(crate::resource::ResourceEstimate::new(
        output,
        metadata_with_coordinates,
        metadata
            .checked_add(numerical)
            .ok_or(ProcessingError::SizeOverflow)?,
    ))
}

// Scope: step vector, its two descriptor handle arrays per step, final
// descriptor handle array, and the requested-operation vector when present.
// ProcessedAxis clones share their backing storage. New backing storage and
// output/history allocations need separate accounting; this is not their bound.
pub(crate) fn prepared_retained_bytes(
    rank: usize,
    steps: usize,
    requested: bool,
) -> Result<usize, ProcessingError> {
    let descriptors = steps
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_mul(rank))
        .and_then(|n| n.checked_mul(std::mem::size_of::<ProcessedAxis>()))
        .ok_or(ProcessingError::SizeOverflow)?;
    let per_step = std::mem::size_of::<PreparedStep>()
        .checked_add(if requested {
            std::mem::size_of::<ProcessingOperation>()
        } else {
            // Segmented replay keeps the original IntoIter allocation while
            // collecting one segment's steps into its own vector.
            std::mem::size_of::<PreparedStep>()
        })
        .ok_or(ProcessingError::SizeOverflow)?;
    steps
        .checked_mul(per_step)
        .and_then(|bytes| bytes.checked_add(descriptors))
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn reserve_prepared(
    rank: usize,
    steps: usize,
    requested: bool,
    mut options: ProcessingOptions,
) -> Result<ProcessingOptions, ProcessingError> {
    if !(1..=2).contains(&rank) {
        return Err(ProcessingError::UnsupportedRank { rank });
    }
    options.prepared_bytes = prepared_retained_bytes(rank, steps, requested)?;
    check_sample_peak(0, 0, options)?;
    Ok(options)
}

#[derive(Clone, Copy, Default)]
pub(in crate::processing) struct AxisStorage {
    pub(in crate::processing) bytes: usize,
    pub(in crate::processing) points: usize,
    pub(in crate::processing) signal: bool,
}

pub(in crate::processing) fn raw_axis_storage(
    axes: &[crate::raw::RawAxis],
) -> Result<[AxisStorage; 2], ProcessingError> {
    let mut storage = [AxisStorage::default(); 2];
    if axes.len() > storage.len() {
        return Err(ProcessingError::UnsupportedRank { rank: axes.len() });
    }
    for (slot, axis) in storage.iter_mut().zip(axes) {
        *slot = AxisStorage {
            bytes: ProcessedAxis::backing_bytes(axis.label(), axis.nucleus(), axis.coordinates())
                .ok_or(ProcessingError::SizeOverflow)?,
            signal: axis.role().is_signal(),
            points: axis.points(),
        };
    }
    Ok(storage)
}

pub(in crate::processing) fn axis_storage<'a>(
    axes: impl Iterator<Item = &'a ProcessedAxis>,
) -> Result<[AxisStorage; 2], ProcessingError> {
    let mut storage = [AxisStorage::default(); 2];
    for (index, axis) in axes.enumerate() {
        let slot = storage
            .get_mut(index)
            .ok_or(ProcessingError::UnsupportedRank { rank: index + 1 })?;
        *slot = AxisStorage {
            bytes: ProcessedAxis::backing_bytes(axis.label(), axis.nucleus(), axis.coordinates())
                .ok_or(ProcessingError::SizeOverflow)?,
            signal: axis.role().is_signal(),
            points: axis.points(),
        };
    }
    Ok(storage)
}

pub(in crate::processing) fn axis_reservation<'a>(
    mut axes: [AxisStorage; 2],
    operations: impl Iterator<Item = &'a ProcessingOperation>,
    raw_baseline: bool,
) -> Result<usize, ProcessingError> {
    let mut bytes = if raw_baseline {
        axes[0]
            .bytes
            .checked_add(axes[1].bytes)
            .ok_or(ProcessingError::SizeOverflow)?
    } else {
        0
    };
    for operation in operations {
        if let Some(index) = operation.axis().filter(|i| *i < axes.len()) {
            let stored = &mut axes[index];
            match operation {
                ProcessingOperation::ZeroFill { zero_fill, .. } => {
                    stored.points = stored.points.max(zero_fill.target_points())
                }
                ProcessingOperation::StandardZeroFill { .. } => {
                    stored.points = stored
                        .points
                        .checked_mul(2)
                        .and_then(usize::checked_next_power_of_two)
                        .ok_or(ProcessingError::SizeOverflow)?
                }
                ProcessingOperation::Spectrum { .. } => {
                    // Includes coordinate materialization before transition allocation.
                    bytes = checked_sum(&[bytes, checked_times(stored.points, 8)?])?;
                }
                _ => {}
            }
        }
        let rebuild = match operation {
            ProcessingOperation::ZeroFill { .. }
            | ProcessingOperation::StandardZeroFill { .. }
            | ProcessingOperation::FourierTransform { .. }
            | ProcessingOperation::ComponentTransform { .. }
            | ProcessingOperation::ResolveFrequencyFrame { .. }
            | ProcessingOperation::ReverseAxis { .. }
            | ProcessingOperation::DigitalFilterCorrection {
                correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 { .. },
                ..
            } => true,
            ProcessingOperation::Projection { .. } | ProcessingOperation::Spectrum { .. } => true,
            ProcessingOperation::Window { .. }
            | ProcessingOperation::PhaseCorrection { .. }
            | ProcessingOperation::BaselineCorrection { .. }
            | ProcessingOperation::DigitalFilterCorrection { .. } => false,
        };
        if !rebuild {
            continue;
        }
        for (index, axis) in axes.iter().enumerate() {
            let selected = if matches!(
                operation,
                ProcessingOperation::Spectrum {
                    operation: SpectrumOperation::Slice { .. },
                    ..
                }
            ) {
                // Shared-complex slicing can rebuild the surviving axis to
                // own a Cartesian pair, including its explicit coordinates.
                true
            } else if matches!(operation, ProcessingOperation::Projection { .. }) {
                axis.signal
            } else {
                Some(index) == operation.axis()
            };
            if selected {
                bytes = bytes
                    .checked_add(axis.bytes)
                    .ok_or(ProcessingError::SizeOverflow)?;
            }
        }
        if let ProcessingOperation::Spectrum { axis, operation } = operation {
            if operation.removes_axis() && *axis < 2 {
                if *axis == 0 {
                    axes[0] = axes[1];
                }
                axes[1] = AxisStorage::default();
            }
        }
    }
    Ok(bytes)
}

pub(crate) fn reserved_raw_axis_bytes(
    raw: &RawDataset,
    operations: &[ProcessingOperation],
) -> Result<usize, ProcessingError> {
    raw_plan_axis_bytes(raw.descriptor().axes(), operations)
}

pub(in crate::processing) fn raw_plan_axis_bytes(
    axes: &[crate::raw::RawAxis],
    operations: &[ProcessingOperation],
) -> Result<usize, ProcessingError> {
    let storage = raw_axis_storage(axes)?;
    checked_sum(&[
        checked_times(axis_reservation(storage, std::iter::empty(), true)?, 4)?,
        checked_times(axis_reservation(storage, operations.iter(), false)?, 2)?,
    ])
}

pub(in crate::processing) fn reserved_processed_axis_bytes(
    input: &ProcessedDataset,
    operations: &[ProcessingOperation],
) -> Result<usize, ProcessingError> {
    let new_axes = axis_reservation(
        axis_storage(input.descriptor().axes().iter())?,
        operations.iter(),
        false,
    )?;
    let old_axes = input
        .provenance()
        .history()
        .map_or(Ok(0), history_axis_bytes)?;
    let raw_baseline = if let ProcessedOrigin::DerivedRaw(origin) = input.provenance().origin() {
        axis_reservation(
            raw_axis_storage(origin.snapshot().descriptor().axes())?,
            std::iter::empty(),
            true,
        )?
    } else {
        0
    };
    checked_sum(&[checked_times(new_axes, 2)?, old_axes, raw_baseline])
}

pub(in crate::processing) fn history_axis_bytes(
    history: &ProcessingHistory,
) -> Result<usize, ProcessingError> {
    history.records().iter().try_fold(0usize, |bytes, record| {
        let extra = match record {
            ProcessingRecord::Applied {
                requested,
                input_descriptor,
                ..
            } => axis_reservation(
                axis_storage(input_descriptor.axes().iter())?,
                requested.explicit().into_iter(),
                false,
            )?,
            ProcessingRecord::Attempted { .. } => 0,
        };
        bytes
            .checked_add(extra)
            .ok_or(ProcessingError::SizeOverflow)
    })
}

pub(in crate::processing) fn checked_sum(values: &[usize]) -> Result<usize, ProcessingError> {
    values
        .iter()
        .try_fold(0usize, |sum, &value| sum.checked_add(value))
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn checked_times(
    value: usize,
    count: usize,
) -> Result<usize, ProcessingError> {
    value
        .checked_mul(count)
        .ok_or(ProcessingError::SizeOverflow)
}

pub(crate) fn state_storage_bytes(
    rank: usize,
    map_points: usize,
) -> Result<usize, ProcessingError> {
    // Four independent ownership slots: history seed, resolving state, previous
    // segment output, and new output. Sum their bounds even when lifetimes differ.
    // Arc construction may temporarily coexist with its source Vec.
    let origin = checked_times(rank, std::mem::size_of::<usize>())?;
    let mapping = checked_times(map_points, std::mem::size_of::<usize>())?;
    if origin > isize::MAX as usize || mapping > isize::MAX as usize {
        return Err(ProcessingError::SizeOverflow);
    }
    let arc_header = 3 * std::mem::size_of::<usize>();
    let per_state = checked_sum(&[
        checked_times(
            rank,
            std::mem::size_of::<crate::processing::contracts::state::AxisState>(),
        )?,
        checked_times(origin, 2)?,
        arc_header,
        if map_points == 0 {
            0
        } else {
            checked_sum(&[checked_times(mapping, 2)?, arc_header])?
        },
    ])?;
    // Additional descriptor ownership slots and kernel/output index arrays.
    // Their audited inventory is recorded in ADR 0005; the counts bound actual
    // holders and do not scale with sample count or history length.
    checked_sum(&[
        checked_times(per_state, 4)?,
        checked_times(rank, 10 * std::mem::size_of::<ProcessedAxis>())?,
        checked_times(rank, 9 * std::mem::size_of::<usize>())?,
        checked_times(rank, std::mem::size_of::<crate::provenance::InputAxisRef>())?,
    ])
}

pub(in crate::processing) fn reserve_states(
    rank: usize,
    map_points: usize,
    mut options: ProcessingOptions,
) -> Result<ProcessingOptions, ProcessingError> {
    options.state_bytes = options
        .state_bytes
        .max(state_storage_bytes(rank, map_points)?);
    check_sample_peak(0, 0, options)?;
    Ok(options)
}

pub(in crate::processing) fn raw_map_points(raw: &RawDataset) -> usize {
    raw.sampling_schedule()
        .map_or(0, |_| raw.descriptor().axes()[0].points())
}

pub(in crate::processing) fn reserve_axes(
    bytes: usize,
    mut options: ProcessingOptions,
) -> Result<ProcessingOptions, ProcessingError> {
    options.axis_bytes = options.axis_bytes.max(bytes);
    check_sample_peak(0, 0, options)?;
    Ok(options)
}

pub(in crate::processing) fn reserve_containers(
    bytes: usize,
    mut options: ProcessingOptions,
) -> Result<ProcessingOptions, ProcessingError> {
    options.container_bytes = options.container_bytes.max(bytes);
    check_sample_peak(0, 0, options)?;
    Ok(options)
}

pub(crate) fn reserve_context(
    metadata: &crate::dataset::DatasetMetadata,
    mut options: ProcessingOptions,
) -> Result<ProcessingOptions, ProcessingError> {
    options.context_bytes = crate::processing::prepare::memory::aggregate(metadata)?;
    check_sample_peak(0, 0, options)?;
    Ok(options)
}

pub(in crate::processing) fn numerical_working_bytes(
    before: &ProcessedDescriptor,
    after: &ProcessedDescriptor,
    axis: Option<usize>,
    resolved: &ResolvedOperation,
) -> Result<usize, ProcessingError> {
    let current = descriptor_bytes(before)?;
    let next = descriptor_bytes(after)?;
    let points = axis.map(|axis| before.axes()[axis].points());
    let gather = match resolved {
        ResolvedOperation::FourierTransform(_)
        | ResolvedOperation::FrequencyDomainPhaseRampV1 { .. }
        | ResolvedOperation::TimeDomainShiftFoldV1 { .. }
        | ResolvedOperation::PhaseCorrection(_)
        | ResolvedOperation::AutomaticPhase { .. } => points
            .expect("single-axis kernel has a target")
            .checked_mul(std::mem::size_of::<Complex64>())
            .ok_or(ProcessingError::SizeOverflow)?,
        ResolvedOperation::ComponentTransform { transform, .. } => transform
            .input_lanes()
            .checked_mul(std::mem::size_of::<Complex64>())
            .ok_or(ProcessingError::SizeOverflow)?,
        ResolvedOperation::Projection {
            projection: Projection::Magnitude,
            ..
        } => before
            .axes()
            .iter()
            .try_fold(std::mem::size_of::<f64>(), |bytes, axis| {
                bytes.checked_mul(axis.component_count())
            })
            .ok_or(ProcessingError::SizeOverflow)?,
        _ => 0,
    };
    let scratch = match resolved {
        ResolvedOperation::Spectrum(_) => {
            spectrum::working_bytes(points.expect("spectrum operation has an axis"))?
        }
        ResolvedOperation::FourierTransform(_) => {
            crate::processing::kernels::fft::working_bytes(points.expect("FFT has an axis"), 1)?
        }
        ResolvedOperation::TimeDomainShiftFoldV1 { .. } => {
            crate::processing::kernels::fft::working_bytes(
                points.expect("shift/fold has an axis"),
                2,
            )?
        }
        ResolvedOperation::BaselineCorrection(_) => points
            .expect("single-axis kernel has a target")
            .checked_mul(18)
            .and_then(|values| values.checked_mul(std::mem::size_of::<f64>()))
            .ok_or(ProcessingError::SizeOverflow)?,
        _ => 0,
    };
    let tail = match resolved {
        ResolvedOperation::TimeDomainShiftFoldV1 { fold, .. } => fold
            .checked_mul(std::mem::size_of::<Complex64>())
            .ok_or(ProcessingError::SizeOverflow)?,
        _ => 0,
    };
    [current, next, gather, scratch, tail]
        .into_iter()
        .try_fold(0usize, |total, bytes| total.checked_add(bytes))
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn validate_processing_axes(
    dataset: &ProcessedDataset,
) -> Result<(), ProcessingError> {
    let axes = dataset.descriptor().axes();
    let rank = axes.len();
    if !(1..=2).contains(&rank) {
        return Err(ProcessingError::UnsupportedRank { rank });
    }
    if let Some(axis) = axes[..rank - 1]
        .iter()
        .position(|axis| axis.role() == AxisRole::DirectAcquisition)
    {
        return Err(ProcessingError::MissingCapability {
            capability: "direct acquisition axis in fastest storage position",
            axis: Some(axis),
        });
    }
    Ok(())
}
