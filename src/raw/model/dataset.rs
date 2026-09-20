use super::*;

/// Fully decoded raw dataset.
///
/// Cloning performs a deep copy of all owned sample storage.
#[derive(Clone, Debug, PartialEq)]
pub struct RawDataset {
    descriptor: RawDescriptor,
    data: RawData,
    provenance: RawProvenance,
    sampling: Option<SamplingSchedule>,
}
impl RawDataset {
    pub(crate) fn with_reader_sources(mut self, mut sources: Vec<SourceFile>) -> Self {
        for (ordinal, source) in sources.iter_mut().enumerate() {
            source.assign_id(ordinal);
        }
        self.provenance.sources = sources;
        self
    }
    pub(crate) fn from_reader(
        descriptor: RawDescriptor,
        data: RawData,
        provenance: RawProvenance,
        sampling: Option<SamplingSchedule>,
    ) -> Result<Self, ValidationError> {
        let value = Self {
            descriptor,
            data,
            provenance,
            sampling,
        };
        value.validate()?;
        Ok(value)
    }
    /// Returns the normalized descriptor.
    pub fn descriptor(&self) -> &RawDescriptor {
        &self.descriptor
    }
    /// Returns canonical dense or sparse samples.
    pub fn data(&self) -> &RawData {
        &self.data
    }
    /// Returns source-layer provenance and opaque format metadata.
    pub fn provenance(&self) -> &RawProvenance {
        &self.provenance
    }
    /// Returns a schedule when acquisition order was coordinate-driven.
    pub fn sampling_schedule(&self) -> Option<&SamplingSchedule> {
        self.sampling.as_ref()
    }
    /// Computes frozen canonical descriptor, sample, and dataset identities.
    pub fn canonical_digests(&self) -> crate::provenance::CanonicalDatasetDigests {
        crate::canonical_digest::dataset_digests(self)
    }
    /// Reads one logical coordinate, rejecting duplicate sparse observations.
    pub fn read_trace(&self, coordinate: &[usize]) -> Result<crate::raw::Trace, ReadError> {
        self.data.read_trace(coordinate)
    }
    /// Reads one acquisition-order observation exactly.
    pub fn read_observation(
        &self,
        ordinal: ObservationOrdinal,
    ) -> Result<crate::raw::Trace, ReadError> {
        if self.data.is_sparse() {
            return self.data.read_observation(ordinal);
        }
        let coordinate = if let Some(schedule) = &self.sampling {
            schedule
                .coordinates()
                .get(ordinal.get())
                .ok_or(AccessError::ObservationUnavailable { ordinal })?
                .as_slice()
                .to_vec()
        } else {
            unflatten(
                &self.data.shape()[..self.data.shape().len() - 1],
                ordinal.get(),
            )?
        };
        let trace = self.data.read_trace(&coordinate)?;
        crate::raw::Trace::new(
            &coordinate,
            Some(ordinal),
            trace.direct_points(),
            trace.component_lanes(),
            trace.samples().to_vec(),
        )
    }
    /// Rechecks every normalized acquisition invariant.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.descriptor.validate()?;
        self.data.validate()?;
        if self.descriptor.layout() != &self.data.layout {
            return Err(ValidationError::DescriptorDataMismatch);
        }
        if self.descriptor.axes.last().is_some_and(|axis| {
            matches!(
                axis.kind(),
                RawAxisKind::Direct(crate::acquisition::DirectSamples::Real)
            )
        }) {
            let has_imaginary = match &self.data.representation {
                SampleRepresentation::Dense(samples) => {
                    samples.iter().any(|sample| sample.im != 0.0)
                }
                SampleRepresentation::Sparse(traces) => traces
                    .iter()
                    .flat_map(|trace| &trace.samples)
                    .any(|sample| sample.im != 0.0),
            };
            if has_imaginary {
                return Err(ValidationError::ImaginarySamplesOnRealAxis);
            }
        }
        if let Some(schedule) = &self.sampling {
            if schedule.declaration().is_some_and(|d| {
                d.indirect_lanes()
                    != &self.descriptor.component_lanes()[..self.descriptor.axes().len() - 1]
            }) {
                return Err(ValidationError::ScheduleDataMismatch);
            }
            if schedule.grid != self.data.shape()[..self.data.shape().len() - 1] {
                return Err(ValidationError::ScheduleGridMismatch);
            }
            match &self.data.representation {
                SampleRepresentation::Sparse(traces) => {
                    if traces
                        .iter()
                        .map(|trace| &trace.coordinate)
                        .ne(schedule.coordinates.iter())
                        || traces
                            .iter()
                            .enumerate()
                            .any(|(ordinal, trace)| trace.ordinal.get() != ordinal)
                    {
                        return Err(ValidationError::ScheduleDataMismatch);
                    }
                }
                SampleRepresentation::Dense(_) => {
                    if !schedule.is_complete_unique()? {
                        return Err(ValidationError::ScheduleDataMismatch);
                    }
                }
            }
        } else if self.data.is_sparse() {
            return Err(ValidationError::MissingScheduleForSparseData);
        }
        Ok(())
    }
}

/// Public construction boundary that binds a descriptor and samples to one layout.
pub struct RawDatasetBuilder {
    descriptor: RawDescriptor,
    sources: Vec<SourceFile>,
}

impl RawDatasetBuilder {
    /// Creates a checked canonical descriptor for user-authored raw data.
    pub fn new(axes: Vec<RawAxis>, acquisition: RawMetadata) -> Result<Self, ValidationError> {
        Ok(Self {
            descriptor: RawDescriptor::new(axes, acquisition)?,
            sources: Vec::new(),
        })
    }

    /// Attaches caller-selected source provenance without changing semantics.
    pub fn with_sources(mut self, sources: Vec<SourceFile>) -> Self {
        self.sources = sources;
        self
    }

    /// Retains the absolute full-grid origin for a cropped logical region.
    ///
    /// Shape and lane counts remain derived from the builder's axes.
    pub fn absolute_grid_origin(mut self, origin: Vec<usize>) -> Result<Self, ValidationError> {
        self.descriptor.layout = RawLayout::from_region(
            self.descriptor.logical_shape(),
            self.descriptor.component_lanes(),
            origin,
        )?;
        Ok(self)
    }

    /// Builds a dense dataset using the descriptor's immutable layout.
    pub fn dense(self, samples: Vec<Complex64>) -> Result<RawDataset, ValidationError> {
        let data = RawData::dense(self.descriptor.layout.clone(), samples)?;
        RawDataset::from_reader(
            self.descriptor,
            data,
            RawProvenance::user_constructed(self.sources),
            None,
        )
    }

    /// Builds sparse observations and validates their acquisition-order schedule.
    pub fn sparse(
        self,
        traces: Vec<SparseTrace>,
        schedule: SamplingSchedule,
    ) -> Result<RawDataset, ValidationError> {
        let data = RawData::sparse(self.descriptor.layout.clone(), traces)?;
        RawDataset::from_reader(
            self.descriptor,
            data,
            RawProvenance::user_constructed(self.sources),
            Some(schedule),
        )
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawDataset {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &RawDescriptor,
        &RawData,
        &RawProvenance,
        &Option<SamplingSchedule>,
    ) {
        (
            &self.descriptor,
            &self.data,
            &self.provenance,
            &self.sampling,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            RawDescriptor,
            RawData,
            RawProvenance,
            Option<SamplingSchedule>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (descriptor, data, provenance, sampling) = parts;
        let value = Self {
            descriptor,
            data,
            provenance,
            sampling,
        };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
