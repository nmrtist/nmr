use super::options::TraceOrder;
use crate::Complex64;
use crate::ReadError;
use crate::raw::InputSource;

pub(crate) fn flatten_index(shape: &[usize], coordinate: &[usize]) -> Result<usize, ReadError> {
    if shape.len() != coordinate.len() {
        return Err(ReadError::corrupt(
            InputSource::memory("normalized Varian trace mapping"),
            "coordinate rank mismatch",
        ));
    }
    shape
        .iter()
        .zip(coordinate)
        .try_fold(0usize, |index, (&extent, &value)| {
            if value >= extent {
                None
            } else {
                index.checked_mul(extent)?.checked_add(value)
            }
        })
        .ok_or(ReadError::SizeOverflow)
}

pub(crate) fn unflatten_index(shape: &[usize], mut index: usize) -> Result<Vec<usize>, ReadError> {
    let total = crate::checked_product(shape).ok_or(ReadError::SizeOverflow)?;
    if index >= total {
        return Err(ReadError::corrupt(
            InputSource::memory("normalized Varian trace mapping"),
            "flat index exceeds shape",
        ));
    }
    let mut coordinate = vec![0; shape.len()];
    for axis in (0..shape.len()).rev() {
        coordinate[axis] = index % shape[axis];
        index /= shape[axis];
    }
    Ok(coordinate)
}

pub(crate) fn disk_component_index(
    lanes: &[usize],
    canonical_component: usize,
    order: TraceOrder,
) -> Result<usize, ReadError> {
    if matches!(order, TraceOrder::Flat | TraceOrder::Regular) {
        return Ok(canonical_component);
    }
    let components = unflatten_index(lanes, canonical_component)?;
    let reversed_lanes: Vec<usize> = lanes.iter().rev().copied().collect();
    let reversed_components: Vec<usize> = components.iter().rev().copied().collect();
    flatten_index(&reversed_lanes, &reversed_components)
}

pub(crate) fn canonical_to_disk_mapping(
    logical_shape: &[usize],
    lanes: &[usize],
    order: TraceOrder,
    asserted_physical_to_canonical: Option<&[usize]>,
) -> Result<Vec<usize>, ReadError> {
    let expanded_shape = logical_shape
        .iter()
        .zip(lanes)
        .map(|(&logical, &lane_count)| {
            logical
                .checked_mul(lane_count)
                .ok_or(ReadError::SizeOverflow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expanded_traces = crate::checked_product(&expanded_shape).ok_or(ReadError::SizeOverflow)?;
    if let Some(physical_to_canonical) = asserted_physical_to_canonical {
        if physical_to_canonical.len() != expanded_traces {
            return Err(ReadError::assertion_conflict(
                "asserted trace permutation length disagrees with the resolved layout",
            ));
        }
        let mut canonical_to_physical = vec![usize::MAX; expanded_traces];
        for (physical, &canonical) in physical_to_canonical.iter().enumerate() {
            if canonical >= expanded_traces || canonical_to_physical[canonical] != usize::MAX {
                return Err(ReadError::assertion_conflict(
                    "asserted trace permutation is not a complete bijection",
                ));
            }
            canonical_to_physical[canonical] = physical;
        }
        return Ok(canonical_to_physical);
    }

    let component_count = crate::checked_product(lanes).ok_or(ReadError::SizeOverflow)?;
    (0..expanded_traces)
        .map(|canonical_trace| {
            let expanded = unflatten_index(&expanded_shape, canonical_trace)?;
            let logical = expanded
                .iter()
                .zip(lanes)
                .map(|(&value, &lane_count)| value / lane_count)
                .collect::<Vec<_>>();
            let components = expanded
                .iter()
                .zip(lanes)
                .map(|(&value, &lane_count)| value % lane_count)
                .collect::<Vec<_>>();
            let logical_index = flatten_index(logical_shape, &logical)?;
            let component_index = flatten_index(lanes, &components)?;
            let disk_component = disk_component_index(lanes, component_index, order)?;
            logical_index
                .checked_mul(component_count)
                .and_then(|value| value.checked_add(disk_component))
                .ok_or(ReadError::SizeOverflow)
        })
        .collect()
}

pub(crate) fn canonical_trace_index(
    logical_shape: &[usize],
    lanes: &[usize],
    mut logical_trace: usize,
    mut component: usize,
) -> Result<usize, ReadError> {
    if logical_shape.len() != lanes.len()
        || logical_trace >= crate::checked_product(logical_shape).ok_or(ReadError::SizeOverflow)?
        || component >= crate::checked_product(lanes).ok_or(ReadError::SizeOverflow)?
    {
        return Err(ReadError::corrupt(
            InputSource::memory("normalized Varian trace mapping"),
            "trace or component index exceeds its shape",
        ));
    }
    let mut index = 0usize;
    let mut stride = 1usize;
    for (&points, &lane_count) in logical_shape.iter().zip(lanes).rev() {
        let point = logical_trace % points;
        logical_trace /= points;
        let lane = component % lane_count;
        component /= lane_count;
        let coordinate = point
            .checked_mul(lane_count)
            .and_then(|value| value.checked_add(lane))
            .ok_or(ReadError::SizeOverflow)?;
        index = coordinate
            .checked_mul(stride)
            .and_then(|value| index.checked_add(value))
            .ok_or(ReadError::SizeOverflow)?;
        stride = points
            .checked_mul(lane_count)
            .and_then(|extent| stride.checked_mul(extent))
            .ok_or(ReadError::SizeOverflow)?;
    }
    Ok(index)
}

pub(crate) fn canonical_trace_group(
    samples: &[Complex64],
    logical_trace: usize,
    logical_shape: &[usize],
    direct_points: usize,
    lanes: &[usize],
    trace_mapping: &[usize],
) -> Result<Vec<Complex64>, ReadError> {
    let component_count = crate::checked_product(lanes).ok_or(ReadError::SizeOverflow)?;
    let mut trace = Vec::with_capacity(
        component_count
            .checked_mul(direct_points)
            .ok_or(ReadError::SizeOverflow)?,
    );
    for component in 0..component_count {
        let canonical_trace =
            canonical_trace_index(logical_shape, lanes, logical_trace, component)?;
        let disk_trace = *trace_mapping.get(canonical_trace).ok_or_else(|| {
            ReadError::corrupt(
                InputSource::memory("normalized Varian trace mapping"),
                "canonical trace mapping is incomplete",
            )
        })?;
        let start = disk_trace
            .checked_mul(direct_points)
            .ok_or(ReadError::SizeOverflow)?;
        let end = start
            .checked_add(direct_points)
            .ok_or(ReadError::SizeOverflow)?;
        trace.extend_from_slice(samples.get(start..end).ok_or_else(|| {
            ReadError::corrupt(
                InputSource::memory("decoded Varian traces"),
                "trace mapping exceeds fid data",
            )
        })?);
    }
    Ok(trace)
}

pub(crate) fn scatter_logical_trace(
    destination: &mut [Complex64],
    storage_shape: &[usize],
    logical_coordinate: &[usize],
    lanes: &[usize],
    direct_points: usize,
    trace: &[Complex64],
) -> Result<(), ReadError> {
    let component_count = crate::checked_product(lanes).ok_or(ReadError::SizeOverflow)?;
    for component in 0..component_count {
        let component_coordinate = unflatten_index(lanes, component)?;
        let expanded: Vec<usize> = logical_coordinate
            .iter()
            .zip(lanes)
            .zip(&component_coordinate)
            .map(|((&logical, &lane_count), &lane)| logical * lane_count + lane)
            .collect();
        let storage_trace = flatten_index(&storage_shape[..storage_shape.len() - 1], &expanded)?;
        let target = storage_trace
            .checked_mul(direct_points)
            .ok_or(ReadError::SizeOverflow)?;
        let source = component
            .checked_mul(direct_points)
            .ok_or(ReadError::SizeOverflow)?;
        destination[target..target + direct_points]
            .copy_from_slice(&trace[source..source + direct_points]);
    }
    Ok(())
}

pub(crate) fn reorder_dense(
    samples: &[Complex64],
    logical_shape: &[usize],
    lanes: &[usize],
    direct_points: usize,
    trace_mapping: &[usize],
) -> Result<Vec<Complex64>, ReadError> {
    let expanded_shape: Vec<usize> = logical_shape
        .iter()
        .zip(lanes)
        .map(|(&logical, &lane_count)| {
            logical
                .checked_mul(lane_count)
                .ok_or(ReadError::SizeOverflow)
        })
        .collect::<Result<_, _>>()?;
    let expanded_traces = crate::checked_product(&expanded_shape).ok_or(ReadError::SizeOverflow)?;
    if trace_mapping.len() != expanded_traces {
        return Err(ReadError::corrupt(
            InputSource::memory("normalized Varian trace mapping"),
            "trace mapping length disagrees with canonical storage",
        ));
    }
    let mut canonical = vec![Complex64::new(0.0, 0.0); samples.len()];
    for (canonical_trace, &disk_trace) in trace_mapping.iter().enumerate() {
        let source = disk_trace
            .checked_mul(direct_points)
            .ok_or(ReadError::SizeOverflow)?;
        let target = canonical_trace
            .checked_mul(direct_points)
            .ok_or(ReadError::SizeOverflow)?;
        canonical[target..target + direct_points]
            .copy_from_slice(&samples[source..source + direct_points]);
    }
    Ok(canonical)
}
