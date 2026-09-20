use super::*;

pub(crate) fn checked_product(values: &[usize]) -> Option<usize> {
    values.iter().try_fold(1usize, |a, &b| a.checked_mul(b))
}

pub(crate) fn validate_trace_coordinate(
    shape: &[usize],
    coordinate: &[usize],
) -> Result<(), AccessError> {
    let expected = shape.len().saturating_sub(1);
    if coordinate.len() != expected {
        return Err(AccessError::TraceRankMismatch {
            expected,
            actual: coordinate.len(),
        });
    }
    for (axis, (&index, &points)) in coordinate.iter().zip(shape).enumerate() {
        if index >= points {
            return Err(AccessError::TraceOutOfBounds {
                axis,
                index,
                points,
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_region(
    logical_shape: &[usize],
    start: &[usize],
    shape: &[usize],
) -> Result<(), AccessError> {
    if start.len() != logical_shape.len() || shape.len() != logical_shape.len() {
        return Err(AccessError::RegionRankMismatch {
            expected: logical_shape.len(),
            start_rank: start.len(),
            shape_rank: shape.len(),
        });
    }
    for (axis, &length) in shape.iter().enumerate() {
        if length == 0 {
            return Err(AccessError::EmptyRegion { axis });
        }
    }
    for (axis, ((&start, &length), &points)) in
        start.iter().zip(shape).zip(logical_shape).enumerate()
    {
        if start.checked_add(length).is_none_or(|end| end > points) {
            return Err(AccessError::RegionOutOfBounds {
                axis,
                start,
                length,
                points,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_absolute_region(
    layout: &RawLayout,
    start: &[usize],
    shape: &[usize],
) -> Result<Vec<usize>, ReadError> {
    let logical_shape = layout.logical_shape();
    if start.len() != logical_shape.len() || shape.len() != logical_shape.len() {
        return Err(AccessError::RegionRankMismatch {
            expected: logical_shape.len(),
            start_rank: start.len(),
            shape_rank: shape.len(),
        }
        .into());
    }
    let allocation_bytes = start
        .len()
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut relative = Vec::new();
    relative
        .try_reserve_exact(start.len())
        .map_err(|_| ReadError::allocation(allocation_bytes))?;
    for (axis, ((&start, &length), (&points, &origin))) in start
        .iter()
        .zip(shape)
        .zip(logical_shape.iter().zip(layout.absolute_origin()))
        .enumerate()
    {
        if length == 0 {
            return Err(AccessError::EmptyRegion { axis }.into());
        }
        let axis_end = origin.checked_add(points).ok_or(ReadError::SizeOverflow)?;
        if start < origin
            || start
                .checked_add(length)
                .is_none_or(|region_end| region_end > axis_end)
        {
            return Err(AccessError::RegionOutOfBounds {
                axis,
                start,
                length,
                points: axis_end,
            }
            .into());
        }
        relative.push(start - origin);
    }
    Ok(relative)
}

pub(super) fn unflatten(shape: &[usize], mut index: usize) -> Result<Vec<usize>, ReadError> {
    if index >= checked_product(shape).ok_or(ReadError::SizeOverflow)? {
        return Err(ReadError::corrupt(
            InputSource::memory("normalized raw data"),
            "normalized storage index is out of bounds",
        ));
    }
    let mut coordinate = vec![0; shape.len()];
    for axis in (0..shape.len()).rev() {
        coordinate[axis] = index % shape[axis];
        index /= shape[axis];
    }
    Ok(coordinate)
}

// Inputs have already passed layout/access validation at the caller.
pub(super) fn trace_storage_index(
    shape: &[usize],
    lanes: &[usize],
    logical: &[usize],
    mut component: usize,
    direct_point: usize,
) -> Result<usize, ReadError> {
    let mut stride = 1usize;
    let mut index = 0usize;
    for axis in (0..shape.len()).rev() {
        let lane = component % lanes[axis];
        component /= lanes[axis];
        let point = if axis == shape.len() - 1 {
            direct_point
        } else {
            logical[axis]
        };
        let expanded = point
            .checked_mul(lanes[axis])
            .and_then(|value| value.checked_add(lane))
            .ok_or(ReadError::SizeOverflow)?;
        index = expanded
            .checked_mul(stride)
            .and_then(|value| index.checked_add(value))
            .ok_or(ReadError::SizeOverflow)?;
        stride = stride
            .checked_mul(shape[axis])
            .and_then(|value| value.checked_mul(lanes[axis]))
            .ok_or(ReadError::SizeOverflow)?;
    }
    Ok(index)
}

pub(super) fn region_storage_index(
    source_shape: &[usize],
    lanes: &[usize],
    output_shape: &[usize],
    start: &[usize],
    mut output_index: usize,
) -> Result<usize, ReadError> {
    let mut stride = 1usize;
    let mut index = 0usize;
    for axis in (0..source_shape.len()).rev() {
        let extent = output_shape[axis]
            .checked_mul(lanes[axis])
            .ok_or(ReadError::SizeOverflow)?;
        let expanded = output_index % extent;
        output_index /= extent;
        let coordinate = start[axis]
            .checked_mul(lanes[axis])
            .and_then(|value| value.checked_add(expanded))
            .ok_or(ReadError::SizeOverflow)?;
        index = coordinate
            .checked_mul(stride)
            .and_then(|value| index.checked_add(value))
            .ok_or(ReadError::SizeOverflow)?;
        stride = stride
            .checked_mul(source_shape[axis])
            .and_then(|value| value.checked_mul(lanes[axis]))
            .ok_or(ReadError::SizeOverflow)?;
    }
    Ok(index)
}

pub(crate) fn scatter_trace(
    destination: &mut [Complex64],
    storage_shape: &[usize],
    logical_shape: &[usize],
    component_lanes: &[usize],
    logical_coordinate: &[usize],
    trace: &[Complex64],
) -> Result<(), ReadError> {
    let invalid = || {
        ReadError::corrupt(
            InputSource::memory("normalized raw trace"),
            "trace shape or coordinate does not match the destination",
        )
    };
    let rank = logical_shape.len();
    if rank == 0
        || storage_shape.len() != rank
        || component_lanes.len() != rank
        || logical_coordinate.len() != rank - 1
    {
        return Err(invalid());
    }
    for axis in 0..rank {
        if logical_shape[axis] == 0
            || component_lanes[axis] == 0
            || logical_shape[axis].checked_mul(component_lanes[axis]) != Some(storage_shape[axis])
            || axis < rank - 1 && logical_coordinate[axis] >= logical_shape[axis]
        {
            return Err(invalid());
        }
    }
    let component_count = checked_product(component_lanes).ok_or(ReadError::SizeOverflow)?;
    let direct_points = logical_shape[rank - 1];
    if checked_product(storage_shape).ok_or(ReadError::SizeOverflow)? != destination.len()
        || component_count
            .checked_mul(direct_points)
            .ok_or(ReadError::SizeOverflow)?
            != trace.len()
    {
        return Err(invalid());
    }
    for component in 0..component_count {
        for direct_point in 0..direct_points {
            let target = trace_storage_index(
                logical_shape,
                component_lanes,
                logical_coordinate,
                component,
                direct_point,
            )?;
            destination[target] = trace[component * direct_points + direct_point];
        }
    }
    Ok(())
}

pub(super) fn reserve_complex(length: usize) -> Result<Vec<Complex64>, ReadError> {
    let requested_bytes = length
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(length)
        .map_err(|_| ReadError::allocation(requested_bytes))?;
    Ok(samples)
}

pub(super) fn try_clone_complex(source: &[Complex64]) -> Result<Vec<Complex64>, ReadError> {
    let mut samples = reserve_complex(source.len())?;
    samples.extend_from_slice(source);
    Ok(samples)
}

#[cfg(test)]
mod scatter_tests {
    use super::*;

    #[test]
    fn descriptor_validation_checks_layout_without_rebuilding_it() {
        use crate::acquisition::{DirectSamples, IndirectComponents};
        let axis = |kind| {
            RawAxis::new(
                kind,
                AxisDomain::Time,
                Some(AxisUnit::Second),
                2,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0,
                },
            )
            .unwrap()
        };
        let descriptor = RawDescriptor::new(
            vec![
                axis(RawAxisKind::Indirect(IndirectComponents::Scalar)),
                axis(RawAxisKind::Direct(DirectSamples::Complex)),
            ],
            RawMetadata::default(),
        )
        .unwrap();
        descriptor.validate().unwrap();
        let mut mismatch = descriptor.clone();
        mismatch.layout.logical_shape = vec![2].into();
        assert!(matches!(
            mismatch.validate(),
            Err(ValidationError::DescriptorDataMismatch)
        ));
        let mut mismatch = descriptor.clone();
        mismatch.layout.lane_counts[0] = 2;
        assert!(matches!(
            mismatch.validate(),
            Err(ValidationError::DescriptorDataMismatch)
        ));
        let mut overflow = descriptor.clone();
        overflow.axes[0].points = 1usize << (usize::BITS / 2);
        overflow.axes[1].points = 1usize << (usize::BITS / 2);
        assert!(matches!(
            overflow.validate(),
            Err(ValidationError::SizeOverflow)
        ));
        let mut wrong_order = descriptor;
        wrong_order.axes.swap(0, 1);
        assert!(matches!(
            wrong_order.validate(),
            Err(ValidationError::DirectAxisNotFastest)
        ));
    }

    #[test]
    fn sparse_region_reserves_only_matching_trace_slots() {
        let data = RawData::sparse(
            RawLayout::from_parts(vec![128, 1], vec![1, 1]).unwrap(),
            (0..128)
                .map(|point| {
                    SparseTrace::new(
                        ObservationOrdinal::new(point),
                        SamplingCoordinate::new(vec![point]),
                        vec![Complex64::new(point as f64, 0.0)],
                    )
                })
                .collect(),
        )
        .unwrap();
        let region = data
            .read_region(&crate::raw::Region::new([100, 0], [1, 1]).unwrap(), 16)
            .unwrap();
        let SampleRepresentation::Sparse(traces) = &region.data().representation else {
            panic!("sparse result");
        };
        assert_eq!(traces.capacity(), 1);
        assert_eq!(traces[0].samples(), &[Complex64::new(100.0, 0.0)]);
    }

    #[test]
    fn dense_access_preserves_all_lanes_and_nested_absolute_regions() {
        let data = RawData::dense(
            RawLayout::from_parts(vec![2, 3, 2], vec![2, 2, 2]).unwrap(),
            (0..96)
                .map(|value| Complex64::new(value as f64, 0.0))
                .collect(),
        )
        .unwrap();
        let trace = data.read_trace(&[1, 2]).unwrap();
        let mut expected = Vec::new();
        for slow in 0..2 {
            for middle in 0..2 {
                for fast in 0..2 {
                    for direct in 0..2 {
                        expected.push(Complex64::new(
                            (64 + 24 * slow + 4 * middle + 2 * direct + fast) as f64,
                            0.0,
                        ));
                    }
                }
            }
        }
        assert_eq!(trace.samples(), expected);
        let region = data
            .read_region(
                &crate::raw::Region::new([1, 1, 0], [1, 2, 2]).unwrap(),
                usize::MAX,
            )
            .unwrap();
        let mut expected = Vec::new();
        for slow in 0..2 {
            for middle in 0..4 {
                for fast in 0..4 {
                    expected.push(Complex64::new(
                        ((2 + slow) * 24 + (2 + middle) * 4 + fast) as f64,
                        0.0,
                    ));
                }
            }
        }
        assert_eq!(region.data().dense_samples().unwrap(), expected);
        let inner = crate::raw::Region::new([1, 2, 1], [1, 1, 1]).unwrap();
        let nested = region.data().read_region(&inner, usize::MAX).unwrap();
        let direct = data.read_region(&inner, usize::MAX).unwrap();
        assert_eq!(nested.data(), direct.data());
        assert_eq!(
            nested.data().read_trace(&[1, 2]).unwrap().samples(),
            &[66.0, 67.0, 70.0, 71.0, 90.0, 91.0, 94.0, 95.0]
                .map(|value| Complex64::new(value, 0.0))
        );
    }

    #[test]
    fn sparse_validation_accepts_unordered_unique_ordinals_but_rejects_duplicates() {
        for ordinals in [vec![0, 1, 2], vec![2, 0, 1], vec![usize::MAX, 1, 0]] {
            let data = RawData::sparse(
                RawLayout::from_parts(vec![3, 1], vec![1, 1]).unwrap(),
                ordinals
                    .iter()
                    .enumerate()
                    .map(|(coordinate, &ordinal)| {
                        SparseTrace::new(
                            ObservationOrdinal::new(ordinal),
                            SamplingCoordinate::new(vec![coordinate]),
                            vec![Complex64::new(coordinate as f64, 0.0)],
                        )
                    })
                    .collect(),
            )
            .unwrap();
            data.validate().unwrap();
            assert_eq!(
                data.sparse_traces()
                    .unwrap()
                    .iter()
                    .map(|trace| trace.ordinal().get())
                    .collect::<Vec<_>>(),
                ordinals,
            );
        }
        let duplicate = RawData::sparse(
            RawLayout::from_parts(vec![3, 1], vec![1, 1]).unwrap(),
            [2, 0, 2]
                .into_iter()
                .enumerate()
                .map(|(coordinate, ordinal)| {
                    SparseTrace::new(
                        ObservationOrdinal::new(ordinal),
                        SamplingCoordinate::new(vec![coordinate]),
                        vec![Complex64::new(0.0, 0.0)],
                    )
                })
                .collect(),
        );
        assert_eq!(
            duplicate.unwrap_err(),
            ValidationError::DuplicateObservationOrdinal
        );
    }

    #[test]
    fn schedule_caches_uniqueness_without_reordering_observations() {
        for (grid, coordinates, repeated, complete) in [
            (
                vec![2, 2],
                vec![vec![1, 1], vec![0, 1], vec![1, 0], vec![0, 0]],
                false,
                true,
            ),
            (
                vec![2, 2],
                vec![vec![1, 1], vec![0, 1], vec![1, 1], vec![0, 0]],
                true,
                false,
            ),
            (vec![2, 2], vec![vec![1, 1], vec![0, 1]], false, false),
            (vec![], vec![vec![]], false, true),
            (vec![], vec![vec![], vec![]], true, false),
        ] {
            let coordinates: Vec<_> = coordinates
                .into_iter()
                .map(SamplingCoordinate::new)
                .collect();
            let schedule = SamplingSchedule::new(grid, coordinates.clone()).unwrap();
            assert_eq!(schedule.coordinates(), coordinates);
            for _ in 0..3 {
                assert_eq!(schedule.has_repeated_coordinates(), repeated);
                assert_eq!(schedule.is_complete_unique().unwrap(), complete);
            }
            assert_eq!(schedule.clone(), schedule);
        }
        let error: ReadError = ValidationError::Allocation {
            requested_bytes: 128,
        }
        .into();
        assert_eq!(error.kind(), crate::ReadErrorKind::Allocation);
        assert!(matches!(
            error.reason(),
            ReadErrorReason::Allocation {
                requested_bytes: 128
            }
        ));
    }

    #[test]
    fn scatter_matches_expanded_three_axis_component_order() {
        let sentinel = Complex64::new(-1.0, 0.0);
        let mut actual = vec![sentinel; 96];
        let trace: Vec<_> = (0..16)
            .map(|index| Complex64::new(index as f64, 0.0))
            .collect();
        scatter_trace(
            &mut actual,
            &[4, 6, 4],
            &[2, 3, 2],
            &[2, 2, 2],
            &[1, 2],
            &trace,
        )
        .unwrap();
        let mut expected = vec![sentinel; 96];
        for slow in 0..2 {
            for middle in 0..2 {
                for fast in 0..2 {
                    for direct in 0..2 {
                        expected[64 + 24 * slow + 4 * middle + 2 * direct + fast] =
                            Complex64::new((8 * slow + 4 * middle + 2 * fast + direct) as f64, 0.0);
                    }
                }
            }
        }
        assert_eq!(actual, expected);
        assert!(
            scatter_trace(
                &mut actual,
                &[4, 6, 4],
                &[2, 3, 2],
                &[2, 2, 2],
                &[2, 2],
                &trace
            )
            .is_err()
        );
        assert!(
            scatter_trace(
                &mut actual,
                &[4, 6, 4],
                &[2, 3, 2],
                &[2, 2, 2],
                &[1, 2],
                &trace[..15]
            )
            .is_err()
        );
        assert!(scatter_trace(&mut [], &[], &[], &[], &[], &[]).is_err());
        assert!(
            scatter_trace(
                &mut [],
                &[usize::MAX, 2],
                &[usize::MAX, 2],
                &[1, 1],
                &[0],
                &[]
            )
            .is_err()
        );
    }
}
