use super::header::BodyEndian;
use super::header::Precision;

use crate::ReadError;
use crate::raw::InputSource;

/// Checked experimental mapping from canonical JEOL coordinates to section byte offsets.
pub(super) struct LayoutPlan {
    input_source: InputSource,
    data_start: usize,
    precision: Precision,
    endian: BodyEndian,
    sections: usize,
    physical_count: usize,
    logical_shape: Vec<usize>,
    output_shape: Vec<usize>,
    crop_start: Vec<usize>,
    component_lanes: Vec<usize>,
    acquired_indirect_coordinates: Option<Vec<usize>>,
    submatrix_edge: usize,
}

impl LayoutPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        input_source: InputSource,
        data_start: usize,
        precision: Precision,
        endian: BodyEndian,
        sections: usize,
        physical_count: usize,
        logical_shape: Vec<usize>,
        output_shape: Vec<usize>,
        crop_start: Vec<usize>,
        component_lanes: Vec<usize>,
        acquired_indirect_coordinates: Option<Vec<usize>>,
        submatrix_edge: usize,
    ) -> Result<Self, ReadError> {
        let rank = logical_shape.len();
        if rank == 0
            || output_shape.len() != rank
            || crop_start.len() != rank
            || component_lanes.len() != rank
        {
            return Err(ReadError::corrupt(
                input_source,
                "inconsistent JEOL layout-plan rank",
            ));
        }
        let component_count =
            crate::checked_product(&component_lanes).ok_or(ReadError::SizeOverflow)?;
        if !matches!((sections, component_count), (1, 1) | (2, 1) | (4, 2)) {
            return Err(ReadError::unsupported_code(
                input_source,
                crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
                "unsupported JEOL section/quadrature combination",
            ));
        }
        Ok(Self {
            input_source,
            data_start,
            precision,
            endian,
            sections,
            physical_count,
            logical_shape,
            output_shape,
            crop_start,
            component_lanes,
            acquired_indirect_coordinates,
            submatrix_edge,
        })
    }

    pub(super) fn precision(&self) -> Precision {
        self.precision
    }

    pub(super) fn endian(&self) -> BodyEndian {
        self.endian
    }

    pub(super) fn input_source(&self) -> &InputSource {
        &self.input_source
    }

    pub(super) fn sections(&self) -> usize {
        self.sections
    }

    pub(super) fn component_count(&self) -> Result<usize, ReadError> {
        crate::checked_product(&self.component_lanes).ok_or(ReadError::SizeOverflow)
    }

    pub(super) fn direct_points(&self) -> usize {
        self.output_shape[self.output_shape.len() - 1]
    }

    pub(super) fn trace_rank(&self) -> usize {
        self.logical_shape.len() - 1
    }

    pub(super) fn sample_offset(
        &self,
        section: usize,
        coordinate: &[usize],
        direct: usize,
    ) -> Result<usize, ReadError> {
        self.sample_offset_inner(section, coordinate, direct, None)
    }

    pub(super) fn sample_offset_observation(
        &self,
        section: usize,
        coordinate: &[usize],
        direct: usize,
        observation: usize,
    ) -> Result<usize, ReadError> {
        self.sample_offset_inner(section, coordinate, direct, Some(observation))
    }

    fn sample_offset_inner(
        &self,
        section: usize,
        coordinate: &[usize],
        direct: usize,
        observation: Option<usize>,
    ) -> Result<usize, ReadError> {
        if section >= self.sections || coordinate.len() != self.trace_rank() {
            return Err(ReadError::corrupt(
                self.input_source.clone(),
                "JEOL sample coordinate rank or section is out of bounds",
            ));
        }
        let direct_axis = self.logical_shape.len() - 1;
        let physical = |axis: usize| -> Result<usize, ReadError> {
            let shifted = if axis == direct_axis {
                direct
                    .checked_add(self.crop_start[axis])
                    .ok_or(ReadError::SizeOverflow)?
            } else if axis == 0 {
                let value = coordinate[axis];
                match (observation, &self.acquired_indirect_coordinates) {
                    (Some(row), Some(_)) => row,
                    (None, Some(acquired)) => acquired
                        .iter()
                        .position(|&candidate| candidate == value)
                        .ok_or_else(|| crate::AccessError::UnsampledCoordinate {
                            coordinate: coordinate.to_vec(),
                        })?,
                    (_, None) => value
                        .checked_add(self.crop_start[axis])
                        .ok_or(ReadError::SizeOverflow)?,
                }
            } else {
                coordinate[axis]
                    .checked_add(self.crop_start[axis])
                    .ok_or(ReadError::SizeOverflow)?
            };
            if shifted >= self.logical_shape[axis] {
                return Err(ReadError::corrupt(
                    self.input_source.clone(),
                    if axis == direct_axis {
                        "JEOL direct coordinate is out of bounds"
                    } else {
                        "JEOL physical coordinate is out of bounds"
                    },
                ));
            }
            Ok(shifted)
        };
        let raw_index = tiled_source_index(
            &self.logical_shape,
            physical,
            self.submatrix_edge,
            &self.input_source,
        )?;
        let value_index = section
            .checked_mul(self.physical_count)
            .and_then(|value| value.checked_add(raw_index))
            .ok_or(ReadError::SizeOverflow)?;
        self.data_start
            .checked_add(
                value_index
                    .checked_mul(self.precision.size())
                    .ok_or(ReadError::SizeOverflow)?,
            )
            .ok_or(ReadError::SizeOverflow)
    }
}

fn tiled_source_index(
    shape: &[usize],
    mut coordinate: impl FnMut(usize) -> Result<usize, ReadError>,
    submatrix_edge: usize,
    source: &InputSource,
) -> Result<usize, ReadError> {
    if shape.is_empty() {
        return Err(ReadError::corrupt(
            source.clone(),
            "JEOL physical coordinate has no axes",
        ));
    }
    let mut tile_index = 0usize;
    let mut local_index = 0usize;
    let mut tile_size = 1usize;
    for (axis, &extent) in shape.iter().enumerate() {
        let value = coordinate(axis)?;
        if value >= extent {
            return Err(ReadError::corrupt(
                source.clone(),
                "JEOL physical coordinate is out of bounds",
            ));
        }
        if shape.len() == 1 {
            return Ok(value);
        }
        let tile_extent = extent
            .checked_div(submatrix_edge)
            .ok_or(ReadError::SizeOverflow)?;
        let tile_coordinate = value / submatrix_edge;
        if tile_coordinate >= tile_extent {
            return Err(ReadError::SizeOverflow);
        }
        tile_index = tile_index
            .checked_mul(tile_extent)
            .and_then(|index| index.checked_add(tile_coordinate))
            .ok_or(ReadError::SizeOverflow)?;
        local_index = local_index
            .checked_mul(submatrix_edge)
            .and_then(|index| index.checked_add(value % submatrix_edge))
            .ok_or(ReadError::SizeOverflow)?;
        tile_size = tile_size
            .checked_mul(submatrix_edge)
            .ok_or(ReadError::SizeOverflow)?;
    }
    tile_index
        .checked_mul(tile_size)
        .and_then(|index| index.checked_add(local_index))
        .ok_or(ReadError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn independent_index(slow: usize, middle: usize, fast: usize) -> usize {
        let tile = ((slow / 2) * 3 + middle / 2) * 4 + fast / 2;
        let local = ((slow % 2) * 2 + middle % 2) * 2 + fast % 2;
        tile * 8 + local
    }

    #[test]
    fn tiled_offsets_match_independent_three_axis_formula() {
        let source = InputSource::memory("synthetic tiled layout");
        for slow in 0..4 {
            for middle in 0..6 {
                for fast in 0..8 {
                    let values = [slow, middle, fast];
                    let mut calls = 0;
                    let actual = tiled_source_index(
                        &[4, 6, 8],
                        |axis| {
                            calls += 1;
                            Ok(values[axis])
                        },
                        2,
                        &source,
                    )
                    .unwrap();
                    assert_eq!(actual, independent_index(slow, middle, fast));
                    assert_eq!(calls, 3);
                }
            }
        }
        assert_eq!(tiled_source_index(&[8], |_| Ok(7), 0, &source).unwrap(), 7);
        assert!(tiled_source_index(&[4, 6], |_| Ok(0), 0, &source).is_err());
        assert!(tiled_source_index(&[4, 6], |_| Ok(9), 2, &source).is_err());
        assert!(matches!(
            tiled_source_index(&[usize::MAX; 2], |_| Ok(usize::MAX - 1), 1, &source),
            Err(error) if matches!(error.reason(), crate::ReadErrorReason::SizeOverflow)
        ));
    }

    #[test]
    fn tiled_offsets_preserve_crop_sections_and_duplicate_observations() {
        let layout = LayoutPlan::new(
            InputSource::memory("synthetic crop"),
            32,
            Precision::F64,
            BodyEndian::Little,
            2,
            192,
            vec![4, 6, 8],
            vec![2, 3, 5],
            vec![1, 2, 1],
            vec![1; 3],
            None,
            2,
        )
        .unwrap();
        for section in 0..2 {
            for slow in 0..2 {
                for middle in 0..3 {
                    for direct in 0..5 {
                        assert_eq!(
                            layout
                                .sample_offset(section, &[slow, middle], direct)
                                .unwrap(),
                            32 + (section * 192
                                + independent_index(slow + 1, middle + 2, direct + 1))
                                * 8
                        );
                    }
                }
            }
        }
        let schedule = LayoutPlan::new(
            InputSource::memory("synthetic repeated observations"),
            32,
            Precision::F64,
            BodyEndian::Little,
            2,
            192,
            vec![4, 6, 8],
            vec![4, 6, 8],
            vec![0; 3],
            vec![1; 3],
            Some(vec![3, 1, 1]),
            2,
        )
        .unwrap();
        assert_eq!(
            schedule.sample_offset(1, &[1, 2], 3).unwrap(),
            32 + (192 + independent_index(1, 2, 3)) * 8
        );
        assert_eq!(
            schedule
                .sample_offset_observation(1, &[1, 2], 3, 2)
                .unwrap(),
            32 + (192 + independent_index(2, 2, 3)) * 8
        );
        assert!(schedule.sample_offset(0, &[0, 0], 0).is_err());
        assert!(
            schedule
                .sample_offset_observation(0, &[1, 0], 0, 4)
                .is_err()
        );
    }
}
