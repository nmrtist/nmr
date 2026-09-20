use crate::Complex64;
use crate::ReadError;
use crate::raw::InputSource;

/// Decode declared Y P/N acquisition after the section sign convention.
/// Sections form A=s0-i*s1=P and B=-s2+i*s3=-N, not C and S.
/// P=(C+i*S), N=(C-i*S), with i the direct complex unit. Thus
/// C=(A-B)/2 and S=(A+B)/(2i). This complex-linear conversion commutes
/// with F2 FFT, so it is also valid on the acquired time-domain traces.
pub(super) fn pn_to_cartesian(samples: &mut [Complex64], direct_points: usize) {
    for row in samples.chunks_exact_mut(2 * direct_points) {
        let (a, b) = row.split_at_mut(direct_points);
        for (a, b) in a.iter_mut().zip(b) {
            let c = *a * 0.5 - *b * 0.5;
            let sum = *a * 0.5 + *b * 0.5;
            *a = c;
            *b = Complex64::new(sum.im, -sum.re);
        }
    }
}

pub(crate) fn reorder_section(
    section: &[f64],
    shape: &[usize],
    submatrix_edge: usize,
    source: &InputSource,
) -> Result<Vec<f64>, ReadError> {
    let total = crate::checked_product(shape).ok_or(ReadError::SizeOverflow)?;
    if section.len() != total {
        return Err(ReadError::truncated(source.clone(), total, section.len()));
    }
    if shape.len() <= 1 {
        return Ok(section.to_vec());
    }
    if submatrix_edge == 0 || shape.iter().any(|&size| size % submatrix_edge != 0) {
        return Err(ReadError::unsupported_code(
            source.clone(),
            crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
            "JEOL data shape is not divisible by its submatrix edge",
        ));
    }
    let tile_shape: Vec<usize> = shape.iter().map(|&size| size / submatrix_edge).collect();
    let tile_count = crate::checked_product(&tile_shape).ok_or(ReadError::SizeOverflow)?;
    let tile_size = submatrix_edge
        .checked_pow(shape.len() as u32)
        .ok_or(ReadError::SizeOverflow)?;
    if tile_count.checked_mul(tile_size) != Some(total) {
        return Err(ReadError::SizeOverflow);
    }
    let strides = row_major_strides(shape)?;
    let mut reordered = vec![0.0; total];
    for tile_index in 0..tile_count {
        let mut remaining = tile_index;
        let mut tile_coordinate = vec![0usize; shape.len()];
        for axis in (0..shape.len()).rev() {
            tile_coordinate[axis] = remaining % tile_shape[axis];
            remaining /= tile_shape[axis];
        }
        for local_index in 0..tile_size {
            let mut local_remaining = local_index;
            let mut destination = 0usize;
            for axis in (0..shape.len()).rev() {
                let local_coordinate = local_remaining % submatrix_edge;
                local_remaining /= submatrix_edge;
                let coordinate = tile_coordinate[axis]
                    .checked_mul(submatrix_edge)
                    .and_then(|v| v.checked_add(local_coordinate))
                    .ok_or(ReadError::SizeOverflow)?;
                destination = destination
                    .checked_add(
                        coordinate
                            .checked_mul(strides[axis])
                            .ok_or(ReadError::SizeOverflow)?,
                    )
                    .ok_or(ReadError::SizeOverflow)?;
            }
            reordered[destination] = section[tile_index * tile_size + local_index];
        }
    }
    Ok(reordered)
}

pub(crate) fn row_major_strides(shape: &[usize]) -> Result<Vec<usize>, ReadError> {
    let mut strides = vec![1usize; shape.len()];
    for axis in (0..shape.len().saturating_sub(1)).rev() {
        strides[axis] = strides[axis + 1]
            .checked_mul(shape[axis + 1])
            .ok_or(ReadError::SizeOverflow)?;
    }
    Ok(strides)
}

pub(crate) fn combine_sections(
    sections: &[Vec<f64>],
    shape: &[usize],
    axis_types: &[u8],
    source: &InputSource,
) -> Result<(Vec<usize>, Vec<Complex64>), ReadError> {
    let total = crate::checked_product(shape).ok_or(ReadError::SizeOverflow)?;
    match sections.len() {
        1 => Ok((
            shape.to_vec(),
            sections[0]
                .iter()
                .copied()
                .map(|value| Complex64::new(value, 0.0))
                .collect(),
        )),
        2 => {
            let samples = sections[0]
                .iter()
                .zip(&sections[1])
                .map(|(&real, &imag)| Complex64::new(real, -imag))
                .collect();
            Ok((shape.to_vec(), samples))
        }
        4 if shape.len() == 2 && axis_types == [3, 3] => {
            let expanded_shape = vec![
                shape[0].checked_mul(2).ok_or(ReadError::SizeOverflow)?,
                shape[1],
            ];
            let expanded_total = total.checked_mul(2).ok_or(ReadError::SizeOverflow)?;
            let mut samples = vec![Complex64::new(0.0, 0.0); expanded_total];
            for row in 0..shape[0] {
                for column in 0..shape[1] {
                    let source = row * shape[1] + column;
                    let real = Complex64::new(sections[0][source], -sections[1][source]);
                    let imag = Complex64::new(-sections[2][source], sections[3][source]);
                    samples[(2 * row) * shape[1] + column] = real;
                    samples[(2 * row + 1) * shape[1] + column] = imag;
                }
            }
            Ok((expanded_shape, samples))
        }
        _ => Err(ReadError::unsupported_code(
            source.clone(),
            crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
            "unsupported JEOL section/quadrature combination",
        )),
    }
}

pub(crate) fn crop_region(
    shape: &[usize],
    samples: &[Complex64],
    starts: &[usize],
    ends: &[usize],
    source: &InputSource,
) -> Result<(Vec<usize>, Vec<Complex64>), ReadError> {
    if shape.len() != starts.len() || shape.len() != ends.len() {
        return Err(ReadError::corrupt(
            source.clone(),
            "JEOL crop rank mismatch",
        ));
    }
    if starts
        .iter()
        .zip(ends)
        .zip(shape)
        .any(|((&start, &end), &size)| start > end || end >= size)
    {
        return Err(ReadError::corrupt(
            source.clone(),
            "JEOL valid window is out of bounds",
        ));
    }
    let cropped_shape: Vec<usize> = starts
        .iter()
        .zip(ends)
        .map(|(&start, &end)| end - start + 1)
        .collect();
    let cropped_count = crate::checked_product(&cropped_shape).ok_or(ReadError::SizeOverflow)?;
    if samples.len() != crate::checked_product(shape).ok_or(ReadError::SizeOverflow)? {
        return Err(ReadError::corrupt(
            source.clone(),
            "JEOL sample shape mismatch",
        ));
    }
    let strides = row_major_strides(shape)?;
    let cropped_bytes = cropped_count
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut cropped = Vec::new();
    cropped
        .try_reserve_exact(cropped_count)
        .map_err(|_| ReadError::allocation(cropped_bytes))?;
    let mut copier = RegionCopier {
        shape,
        strides: &strides,
        starts,
        ends,
        samples,
        input_source: source,
        output: &mut cropped,
    };
    copier.copy(0, 0)?;
    Ok((cropped_shape, cropped))
}

pub(crate) struct RegionCopier<'a> {
    pub(super) shape: &'a [usize],
    pub(super) strides: &'a [usize],
    pub(super) starts: &'a [usize],
    pub(super) ends: &'a [usize],
    pub(super) samples: &'a [Complex64],
    pub(super) input_source: &'a InputSource,
    pub(super) output: &'a mut Vec<Complex64>,
}

impl RegionCopier<'_> {
    pub(super) fn copy(&mut self, axis: usize, source_base: usize) -> Result<(), ReadError> {
        if axis + 1 == self.shape.len() {
            let start = source_base
                .checked_add(self.starts[axis])
                .ok_or(ReadError::SizeOverflow)?;
            let end = source_base
                .checked_add(self.ends[axis])
                .and_then(|value| value.checked_add(1))
                .ok_or(ReadError::SizeOverflow)?;
            self.output
                .extend_from_slice(self.samples.get(start..end).ok_or_else(|| {
                    ReadError::corrupt(self.input_source.clone(), "JEOL crop exceeds sample buffer")
                })?);
            return Ok(());
        }
        for coordinate in self.starts[axis]..=self.ends[axis] {
            let offset = coordinate
                .checked_mul(self.strides[axis])
                .ok_or(ReadError::SizeOverflow)?;
            self.copy(
                axis + 1,
                source_base
                    .checked_add(offset)
                    .ok_or(ReadError::SizeOverflow)?,
            )?;
        }
        Ok(())
    }
}
