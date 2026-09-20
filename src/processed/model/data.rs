use super::*;

/// Finite dense scalar tensor for processed data.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessedData {
    shape: Vec<usize>,
    component_counts: Vec<usize>,
    samples: Vec<f64>,
}

impl ProcessedData {
    /// Creates dense storage with shape and components derived from a checked descriptor.
    pub fn from_descriptor(
        descriptor: &ProcessedDescriptor,
        samples: Vec<f64>,
    ) -> Result<Self, ProcessedValidationError> {
        descriptor.validate()?;
        Self::new(
            descriptor.logical_shape(),
            descriptor.component_counts(),
            samples,
        )
    }
    /// Creates checked canonical row-major storage.
    pub fn new(
        shape: Vec<usize>,
        component_counts: Vec<usize>,
        samples: Vec<f64>,
    ) -> Result<Self, ProcessedValidationError> {
        let value = Self {
            shape,
            component_counts,
            samples,
        };
        value.validate()?;
        Ok(value)
    }

    /// Returns the logical shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns the component count on each axis.
    pub fn component_counts(&self) -> &[usize] {
        &self.component_counts
    }

    /// Returns `s[j] = n[j] * c[j]` for every axis.
    pub fn storage_shape(&self) -> Result<Vec<usize>, ProcessedValidationError> {
        storage_shape(&self.shape, &self.component_counts)
    }

    /// Returns canonical finite scalar storage.
    pub fn samples(&self) -> &[f64] {
        &self.samples
    }

    /// Returns one scalar at a logical coordinate and component coordinate.
    pub fn get(
        &self,
        logical: &[usize],
        components: &[usize],
    ) -> Result<f64, ProcessedAccessError> {
        let offset = checked_offset(&self.shape, &self.component_counts, logical, components)?;
        Ok(self.samples[offset])
    }

    /// Iterates one fixed component plane in logical row-major order.
    pub fn component_plane(
        &self,
        components: &[usize],
    ) -> Result<ComponentPlane<'_>, ProcessedAccessError> {
        validate_components(&self.component_counts, components)?;
        let logical_len = checked_product_access(&self.shape)?;
        if self.component_counts.iter().all(|&count| count == 1) {
            return Ok(ComponentPlane {
                samples: &self.samples,
                contiguous: Some(self.samples.iter()),
                axes: Vec::new(),
                offset: 0,
                remaining: logical_len,
            });
        }
        let mut stride = 1;
        let mut offset = 0;
        let mut axes = Vec::with_capacity(self.shape.len());
        for axis in (0..self.shape.len()).rev() {
            offset += components[axis] * stride;
            let step = stride * self.component_counts[axis];
            axes.push(PlaneAxis {
                position: 0,
                points: self.shape[axis],
                step,
            });
            stride = step * self.shape[axis];
        }
        Ok(ComponentPlane {
            samples: &self.samples,
            contiguous: None,
            axes,
            offset,
            remaining: logical_len,
        })
    }

    /// Borrows a scalar tensor, rejecting component-bearing storage.
    pub fn scalar_plane(&self) -> Result<ScalarPlane<'_>, ProcessedAccessError> {
        if self.component_counts.iter().any(|&n| n != 1) {
            return Err(ProcessedAccessError::NotScalar);
        }
        Ok(ScalarPlane {
            shape: &self.shape,
            samples: &self.samples,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), ProcessedValidationError> {
        validate_rank(self.shape.len())?;
        if self.shape.contains(&0) {
            return Err(ProcessedValidationError::ZeroExtent);
        }
        if self.component_counts.len() != self.shape.len() {
            return Err(ProcessedValidationError::ComponentRankMismatch);
        }
        if self.component_counts.contains(&0) {
            return Err(ProcessedValidationError::ZeroComponentCount);
        }
        let expected = self.shape.iter().zip(&self.component_counts).try_fold(
            1usize,
            |total, (&points, &components)| {
                total
                    .checked_mul(points)
                    .and_then(|value| value.checked_mul(components))
                    .ok_or(ProcessedValidationError::SizeOverflow)
            },
        )?;
        if self.samples.len() != expected {
            return Err(ProcessedValidationError::SampleLengthMismatch);
        }
        if self.samples.iter().any(|value| !value.is_finite()) {
            return Err(ProcessedValidationError::NonFiniteSample);
        }
        Ok(())
    }
}

/// Iterator over a fixed component coordinate.
pub struct ComponentPlane<'a> {
    samples: &'a [f64],
    contiguous: Option<std::slice::Iter<'a, f64>>,
    axes: Vec<PlaneAxis>,
    offset: usize,
    remaining: usize,
}

pub(super) struct PlaneAxis {
    position: usize,
    points: usize,
    step: usize,
}

impl<'a> Iterator for ComponentPlane<'a> {
    type Item = &'a f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        if let Some(iter) = &mut self.contiguous {
            return iter.next();
        }
        let result = &self.samples[self.offset];
        if self.remaining != 0 {
            for axis in &mut self.axes {
                axis.position += 1;
                if axis.position < axis.points {
                    self.offset += axis.step;
                    break;
                }
                self.offset -= (axis.points - 1) * axis.step;
                axis.position = 0;
            }
        }
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ComponentPlane<'_> {}
impl std::iter::FusedIterator for ComponentPlane<'_> {}

/// Contiguous scalar tensor in logical row-major order.
#[derive(Clone, Copy, Debug)]
pub struct ScalarPlane<'a> {
    shape: &'a [usize],
    samples: &'a [f64],
}
impl<'a> ScalarPlane<'a> {
    /// Logical tensor shape.
    pub fn shape(self) -> &'a [usize] {
        self.shape
    }
    /// Contiguous row-major scalar values.
    pub fn samples(self) -> &'a [f64] {
        self.samples
    }
    /// Allocation-free scalar traversal.
    pub fn iter(self) -> std::slice::Iter<'a, f64> {
        self.samples.iter()
    }
}

/// Cartesian values along one logical axis; no complex slice layout is implied.
pub struct ComplexTrace<'a> {
    samples: &'a [f64],
    offset: usize,
    stride: usize,
    imaginary_stride: usize,
    remaining: usize,
}
impl<'a> ComplexTrace<'a> {
    pub(crate) fn new(
        data: &'a ProcessedData,
        axis: usize,
        logical: &[usize],
        component_axis: usize,
        components: &[usize],
    ) -> Result<Self, ProcessedAccessError> {
        let rank = data.shape.len();
        if axis >= rank || component_axis >= rank {
            return Err(ProcessedAccessError::AxisOutOfBounds {
                axis: axis.max(component_axis),
                rank,
            });
        }
        let mut fixed = logical.to_vec();
        if fixed.len() != rank {
            return Err(ProcessedAccessError::RankMismatch {
                expected: rank,
                logical: fixed.len(),
                components: components.len(),
            });
        }
        fixed[axis] = 0;
        let mut lanes = components.to_vec();
        if let Some(lane) = lanes.get_mut(component_axis) {
            *lane = 0;
        }
        validate_components(&data.component_counts, &lanes)?;
        let offset = checked_offset(&data.shape, &data.component_counts, &fixed, &lanes)?;
        let stride = data.component_counts[axis]
            * data.shape[axis + 1..]
                .iter()
                .zip(&data.component_counts[axis + 1..])
                .map(|(n, c)| n * c)
                .product::<usize>();
        let imaginary_stride = data.shape[component_axis + 1..]
            .iter()
            .zip(&data.component_counts[component_axis + 1..])
            .map(|(n, c)| n * c)
            .product();
        Ok(Self {
            samples: &data.samples,
            offset,
            stride,
            imaginary_stride,
            remaining: data.shape[axis],
        })
    }
}
impl Iterator for ComplexTrace<'_> {
    type Item = crate::Complex64;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let value = crate::Complex64::new(
            self.samples[self.offset],
            self.samples[self.offset + self.imaginary_stride],
        );
        self.remaining -= 1;
        if self.remaining != 0 {
            self.offset += self.stride;
        }
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for ComplexTrace<'_> {}
impl std::iter::FusedIterator for ComplexTrace<'_> {}

pub(super) fn checked_offset(
    shape: &[usize],
    component_counts: &[usize],
    logical: &[usize],
    components: &[usize],
) -> Result<usize, ProcessedAccessError> {
    if logical.len() != shape.len() || components.len() != shape.len() {
        return Err(ProcessedAccessError::RankMismatch {
            expected: shape.len(),
            logical: logical.len(),
            components: components.len(),
        });
    }
    validate_components(component_counts, components)?;
    shape
        .iter()
        .zip(component_counts)
        .zip(logical)
        .zip(components)
        .enumerate()
        .try_fold(
            0usize,
            |offset, (axis, (((&points, &count), &point), &component))| {
                if point >= points {
                    return Err(ProcessedAccessError::LogicalOutOfBounds {
                        axis,
                        index: point,
                        points,
                    });
                }
                let storage_extent = points
                    .checked_mul(count)
                    .ok_or(ProcessedAccessError::SizeOverflow)?;
                let expanded = point
                    .checked_mul(count)
                    .and_then(|value| value.checked_add(component))
                    .ok_or(ProcessedAccessError::SizeOverflow)?;
                offset
                    .checked_mul(storage_extent)
                    .and_then(|value| value.checked_add(expanded))
                    .ok_or(ProcessedAccessError::SizeOverflow)
            },
        )
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedData {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Vec<usize>, &Vec<usize>, &Vec<f64>) {
        (&self.shape, &self.component_counts, &self.samples)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Vec<usize>, Vec<usize>, Vec<f64>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (shape, component_counts, samples) = parts;
        let value = Self {
            shape,
            component_counts,
            samples,
        };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
