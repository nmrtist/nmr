//! Applied decoding and component mappings retained by processed datasets.

use super::{CanonicalDatasetDigests, SourceFile, SourceId};

/// Applied decoding, separate from subsequent numerical processing.
/// Only a reader can attach this immutable record to a dataset.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessedReadRecord {
    transform: ProcessedReadTransform,
    components: Vec<SourceId>,
    component_indices: Vec<Vec<usize>>,
    output: crate::provenance::CanonicalDatasetDigests,
}

/// Format-specific numeric decoding parameters actually used by a reader.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessedReadTransform {
    /// Experimental two-dimensional JEOL section decoding, with disk-axis ranges.
    JeolDelta2D {
        /// True for binary64 source values.
        float64: bool,
        /// Numeric source byte order.
        big_endian: bool,
        /// Original point counts in x,y disk order.
        disk_points: [usize; 2],
        /// First retained x,y points.
        crop_start: [usize; 2],
        /// Retained x,y point counts.
        crop_points: [usize; 2],
        /// JEOL section-reordering tile edge.
        submatrix_edge: usize,
        /// Applied direct imaginary multiplier.
        imaginary_multiplier: i8,
    },
    /// Binary Bruker scalar decoding and intensity multiplication by 2^NC_PROC.
    Bruker {
        /// Applied power-of-two intensity exponent.
        nc_proc: i32,
        /// True for IEEE binary64, false for signed int32.
        float64: bool,
        /// True for big-endian scalars.
        big_endian: bool,
    },
    /// JCAMP ASDF coordinate-checkpoint and intensity scaling.
    JcampDx {
        /// Factor applied to encoded X checkpoints, not FIRSTX/LASTX.
        x_factor: f64,
        /// Factor applied to decoded Y values.
        y_factor: f64,
    },
    /// Supported one-dimensional JEOL section decoding and valid-window extraction.
    /// Records which library rule was applied; this is not vendor certification.
    JeolDelta {
        /// True for binary64, false for binary32 source values.
        float64: bool,
        /// Byte order of numeric sample sections.
        big_endian: bool,
        /// Logical point count in each original section.
        disk_points: usize,
        /// First retained point in the original section.
        crop_start: usize,
        /// Number of retained points.
        crop_points: usize,
        /// Multiplier applied to the stored imaginary section.
        imaginary_multiplier: i8,
        /// Edge used by the existing section-reordering rule.
        submatrix_edge: usize,
        /// Decimal unit multiplier applied to header or explicit-list coordinates.
        coordinate_scale: f64,
        /// Whether coordinates came from an explicit source axis list.
        explicit_coordinates: bool,
    },
}

impl ProcessedReadRecord {
    pub(crate) fn validate_recorded(
        &self,
        descriptor: &crate::processed::ProcessedDescriptor,
        digests: CanonicalDatasetDigests,
        sources: &[SourceFile],
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        let counts = descriptor.component_counts();
        if self.output != digests
            || self.components.len() != self.component_indices.len()
            || self.components.len() != counts.iter().product::<usize>()
        {
            return Err(ModelError::Structure);
        }
        for (i, (id, coordinate)) in self
            .components
            .iter()
            .zip(&self.component_indices)
            .enumerate()
        {
            if !sources.iter().any(|source| source.id() == Some(*id))
                || coordinate.len() != counts.len()
                || coordinate.iter().zip(&counts).any(|(v, n)| v >= n)
                || self.component_indices[..i].contains(coordinate)
            {
                return Err(ModelError::Structure);
            }
        }
        let valid = match self.transform {
            ProcessedReadTransform::JeolDelta2D {
                disk_points,
                crop_start,
                crop_points,
                submatrix_edge,
                imaginary_multiplier,
                ..
            } => {
                counts.len() == 2
                    && submatrix_edge > 0
                    && matches!(imaginary_multiplier, -1 | 1)
                    && (0..2).all(|a| {
                        crop_points[a] > 0
                            && crop_points[a] == descriptor.axes()[1 - a].points()
                            && crop_start[a]
                                .checked_add(crop_points[a])
                                .is_some_and(|end| end <= disk_points[a])
                    })
            }
            ProcessedReadTransform::Bruker { nc_proc, .. } => {
                let scale = 2f64.powi(nc_proc);
                scale.is_finite() && scale != 0.0
            }
            ProcessedReadTransform::JcampDx { x_factor, y_factor } => {
                x_factor.is_finite() && y_factor.is_finite() && x_factor != 0.0 && y_factor != 0.0
            }
            ProcessedReadTransform::JeolDelta {
                disk_points,
                crop_start,
                crop_points,
                imaginary_multiplier,
                submatrix_edge,
                coordinate_scale,
                ..
            } => {
                crop_points > 0
                    && crop_start
                        .checked_add(crop_points)
                        .is_some_and(|end| end <= disk_points)
                    && matches!(imaginary_multiplier, -1 | 1)
                    && submatrix_edge > 0
                    && coordinate_scale.is_finite()
                    && coordinate_scale > 0.0
            }
        };
        if !valid {
            return Err(ModelError::Structure);
        }
        Ok(())
    }
    /// Version of the decoding and component-interleaving rule.
    pub fn algorithm_version(&self) -> &'static str {
        match self.transform {
            ProcessedReadTransform::JeolDelta2D { .. } => "jeol.processed-2d.v1",
            ProcessedReadTransform::Bruker { .. } if self.component_indices[0].len() == 1 => {
                "bruker.processed-1d.v1"
            }
            ProcessedReadTransform::Bruker { .. } => "bruker.processed-2d.v1",
            ProcessedReadTransform::JcampDx { .. } => "jcamp.xydata-asdf.v1",
            ProcessedReadTransform::JeolDelta { .. } => "jeol.processed-1d.v1",
        }
    }
    /// Typed format-specific decoding parameters.
    pub fn transform(&self) -> &ProcessedReadTransform {
        &self.transform
    }
    /// Source planes in reader order; see component_indices for tensor mapping.
    pub fn components(&self) -> &[SourceId] {
        &self.components
    }
    /// Per-plane tensor component coordinates, in the same order as components.
    pub fn component_indices(&self) -> &[Vec<usize>] {
        &self.component_indices
    }
    /// Canonical output identity produced by this reader execution.
    pub fn output_digests(&self) -> crate::provenance::CanonicalDatasetDigests {
        self.output
    }
    pub(crate) fn bruker(
        nc_proc: i32,
        float64: bool,
        big_endian: bool,
        components: Vec<SourceId>,
        component_indices: Vec<Vec<usize>>,
        output: crate::provenance::CanonicalDatasetDigests,
    ) -> Self {
        Self {
            transform: ProcessedReadTransform::Bruker {
                nc_proc,
                float64,
                big_endian,
            },
            components,
            component_indices,
            output,
        }
    }
    pub(crate) fn jcamp(
        source: SourceId,
        x_factor: f64,
        y_factor: f64,
        output: crate::provenance::CanonicalDatasetDigests,
    ) -> Self {
        Self {
            transform: ProcessedReadTransform::JcampDx { x_factor, y_factor },
            components: vec![source],
            component_indices: vec![vec![0]],
            output,
        }
    }
    pub(crate) fn jeol(
        source: SourceId,
        transform: ProcessedReadTransform,
        component_count: usize,
        output: crate::provenance::CanonicalDatasetDigests,
    ) -> Self {
        Self {
            transform,
            components: vec![source; component_count],
            component_indices: (0..component_count)
                .map(|component| vec![component])
                .collect(),
            output,
        }
    }
    pub(crate) fn jeol_2d(
        source: SourceId,
        transform: ProcessedReadTransform,
        counts: &[usize],
        output: CanonicalDatasetDigests,
    ) -> Self {
        Self {
            transform,
            components: vec![source; counts.iter().product()],
            component_indices: (0..counts[0])
                .flat_map(|a| (0..counts[1]).map(move |b| vec![a, b]))
                .collect(),
            output,
        }
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedReadRecord {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &ProcessedReadTransform,
        &Vec<SourceId>,
        &Vec<Vec<usize>>,
        &crate::provenance::CanonicalDatasetDigests,
    ) {
        (
            &self.transform,
            &self.components,
            &self.component_indices,
            &self.output,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            ProcessedReadTransform,
            Vec<SourceId>,
            Vec<Vec<usize>>,
            crate::provenance::CanonicalDatasetDigests,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (transform, components, component_indices, output) = parts;
        let value = Self {
            transform,
            components,
            component_indices,
            output,
        };

        if value.components.is_empty()
            || value.components.len() != value.component_indices.len()
            || value.component_indices.iter().any(Vec::is_empty)
        {
            return Err(crate::internal::ModelError::Structure);
        }

        Ok(value)
    }
}
