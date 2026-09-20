use super::*;

/// Canonical dense or sparse samples.
///
/// Cloning performs a deep copy of the owned `Vec<Complex64>` sample storage.
#[derive(Clone, Debug, PartialEq)]
pub struct RawData {
    pub(super) layout: RawLayout,
    pub(super) representation: SampleRepresentation,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SampleRepresentation {
    Dense(Vec<Complex64>),
    Sparse(Vec<SparseTrace>),
}
impl RawData {
    /// Returns the immutable logical/lane layout for these samples.
    pub fn layout(&self) -> &RawLayout {
        &self.layout
    }
    /// Creates checked dense row-major data from an immutable resolved layout.
    pub(crate) fn dense(
        layout: RawLayout,
        samples: Vec<Complex64>,
    ) -> Result<Self, ValidationError> {
        let value = Self {
            layout,
            representation: SampleRepresentation::Dense(samples),
        };
        value.validate()?;
        Ok(value)
    }
    /// Creates checked sparse traces without filling missing coordinates.
    pub(crate) fn sparse(
        layout: RawLayout,
        traces: Vec<SparseTrace>,
    ) -> Result<Self, ValidationError> {
        let value = Self {
            layout,
            representation: SampleRepresentation::Sparse(traces),
        };
        value.validate()?;
        Ok(value)
    }
    /// Logical shape, slowest to fastest axis.
    pub fn shape(&self) -> &[usize] {
        self.layout.logical_shape()
    }
    /// Returns component-lane counts corresponding to [`Self::shape`].
    pub fn component_lanes(&self) -> &[usize] {
        self.layout.lane_counts()
    }
    /// Returns logical point counts multiplied by component lanes per axis.
    pub fn storage_shape(&self) -> Result<Vec<usize>, ValidationError> {
        self.layout.storage_shape()
    }
    /// Returns dense canonical samples, or `None` for sparse data.
    pub fn dense_samples(&self) -> Option<&[Complex64]> {
        match &self.representation {
            SampleRepresentation::Dense(samples) => Some(samples),
            _ => None,
        }
    }
    /// Returns sparse traces, or `None` for dense data.
    pub fn sparse_traces(&self) -> Option<&[SparseTrace]> {
        match &self.representation {
            SampleRepresentation::Sparse(traces) => Some(traces),
            _ => None,
        }
    }
    /// Returns whether missing logical traces remain unmaterialized.
    pub fn is_sparse(&self) -> bool {
        matches!(self.representation, SampleRepresentation::Sparse(_))
    }
    /// Reads one logical indirect coordinate.
    ///
    /// The result is row-major over component lanes, with direct logical
    /// points fastest. No absent sparse trace is silently filled. If a sparse
    /// schedule contains repeated observations, single-coordinate access is
    /// ambiguous and fails rather than selecting an observation arbitrarily.
    pub fn read_trace(&self, coordinate: &[usize]) -> Result<crate::raw::Trace, ReadError> {
        let relative = self.relative_trace_coordinate(coordinate)?;
        let samples = match &self.representation {
            SampleRepresentation::Sparse(traces) => {
                let mut matches = traces
                    .iter()
                    .filter(|trace| trace.coordinate.0 == coordinate);
                let trace = matches.next().ok_or_else(|| {
                    ReadError::from(AccessError::UnsampledCoordinate {
                        coordinate: coordinate.to_vec(),
                    })
                })?;
                if matches.next().is_some() {
                    return Err(AccessError::AmbiguousObservation {
                        coordinate: coordinate.to_vec(),
                    }
                    .into());
                }
                try_clone_complex(&trace.samples)
            }
            SampleRepresentation::Dense(samples) => {
                let component_count =
                    checked_product(self.component_lanes()).ok_or(ReadError::SizeOverflow)?;
                let direct_points = *self.shape().last().expect("validated non-empty shape");
                let trace_len = component_count
                    .checked_mul(direct_points)
                    .ok_or(ReadError::SizeOverflow)?;
                let mut trace = reserve_complex(trace_len)?;
                for component in 0..component_count {
                    for direct_point in 0..direct_points {
                        let index = trace_storage_index(
                            self.shape(),
                            self.component_lanes(),
                            &relative,
                            component,
                            direct_point,
                        )?;
                        trace.push(samples[index]);
                    }
                }
                Ok(trace)
            }
        }?;
        crate::raw::Trace::new(
            coordinate,
            None,
            *self.shape().last().expect("validated non-empty shape"),
            self.component_lanes(),
            samples,
        )
    }

    /// Reads one sparse observation by its unique acquisition-order ordinal.
    pub fn read_observation(
        &self,
        ordinal: ObservationOrdinal,
    ) -> Result<crate::raw::Trace, ReadError> {
        let SampleRepresentation::Sparse(traces) = &self.representation else {
            return Err(AccessError::ObservationUnavailable { ordinal }.into());
        };
        let trace = traces
            .iter()
            .find(|trace| trace.ordinal == ordinal)
            .ok_or(AccessError::ObservationUnavailable { ordinal })?;
        crate::raw::Trace::new(
            trace.coordinate.as_slice(),
            Some(ordinal),
            *self.shape().last().expect("validated non-empty shape"),
            self.component_lanes(),
            try_clone_complex(&trace.samples)?,
        )
    }

    fn relative_trace_coordinate(&self, coordinate: &[usize]) -> Result<Vec<usize>, ReadError> {
        let expected = self.shape().len() - 1;
        if coordinate.len() != expected {
            return Err(AccessError::TraceRankMismatch {
                expected,
                actual: coordinate.len(),
            }
            .into());
        }
        coordinate
            .iter()
            .enumerate()
            .map(|(axis, &value)| {
                let origin = self.layout.absolute_origin()[axis];
                let end = origin
                    .checked_add(self.shape()[axis])
                    .ok_or(ReadError::SizeOverflow)?;
                if value < origin || value >= end {
                    return Err(AccessError::TraceOutOfBounds {
                        axis,
                        index: value,
                        points: end,
                    }
                    .into());
                }
                Ok(value - origin)
            })
            .collect()
    }

    /// Extract a logical region while retaining every component lane.
    pub fn read_region(
        &self,
        region: &crate::raw::Region,
        max_bytes: usize,
    ) -> Result<crate::raw::RegionData, ReadError> {
        self.read_region_with_context(region, max_bytes, &mut crate::ExecutionContext::default())
    }

    /// Copies a logical region with cancellation between bounded sample blocks.
    pub fn read_region_with_context(
        &self,
        region: &crate::raw::Region,
        max_bytes: usize,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<crate::raw::RegionData, ReadError> {
        control.check_cancelled()?;
        let start = region.start();
        let shape = region.shape();
        let relative_start = validate_absolute_region(self.layout(), start, shape)?;
        let total = shape
            .iter()
            .zip(self.component_lanes())
            .try_fold(1usize, |total, (&points, &lanes)| {
                total.checked_mul(points)?.checked_mul(lanes)
            })
            .ok_or(ReadError::SizeOverflow)?;
        let data = match &self.representation {
            SampleRepresentation::Dense(samples) => {
                let bytes = total
                    .checked_mul(std::mem::size_of::<Complex64>())
                    .ok_or(ReadError::SizeOverflow)?;
                if bytes > max_bytes {
                    return Err(ReadError::limit(
                        ReadResource::RegionBytes,
                        max_bytes,
                        bytes,
                    ));
                }
                let mut output = reserve_complex(total)?;
                for output_index in 0..total {
                    if output_index % 4096 == 0 {
                        control.check_cancelled()?;
                    }
                    let source_index = region_storage_index(
                        self.shape(),
                        self.component_lanes(),
                        shape,
                        &relative_start,
                        output_index,
                    )?;
                    output.push(samples[source_index]);
                }
                Self::dense(
                    RawLayout::from_region(
                        shape.to_vec(),
                        self.component_lanes().to_vec(),
                        start.to_vec(),
                    )?,
                    output,
                )
                .map_err(ReadError::from)
            }
            SampleRepresentation::Sparse(traces) => {
                let direct_axis = self.shape().len() - 1;
                let direct_start = relative_start[direct_axis];
                let direct_length = shape[direct_axis];
                let direct_lanes = self.component_lanes()[direct_axis];
                let component_count =
                    checked_product(self.component_lanes()).ok_or(ReadError::SizeOverflow)?;
                let matching_count = traces
                    .iter()
                    .filter(|trace| {
                        trace.coordinate.0.iter().enumerate().all(|(axis, &value)| {
                            value >= start[axis] && value < start[axis] + shape[axis]
                        })
                    })
                    .count();
                let sparse_samples = matching_count
                    .checked_mul(component_count)
                    .and_then(|value| value.checked_mul(direct_length))
                    .ok_or(ReadError::SizeOverflow)?;
                let sparse_bytes = sparse_samples
                    .checked_mul(std::mem::size_of::<Complex64>())
                    .ok_or(ReadError::SizeOverflow)?;
                if sparse_bytes > max_bytes {
                    return Err(ReadError::limit(
                        ReadResource::RegionBytes,
                        max_bytes,
                        sparse_bytes,
                    ));
                }
                let mut output = Vec::new();
                let trace_storage_bytes = matching_count
                    .checked_mul(std::mem::size_of::<SparseTrace>())
                    .ok_or(ReadError::SizeOverflow)?;
                output
                    .try_reserve_exact(matching_count)
                    .map_err(|_| ReadError::allocation(trace_storage_bytes))?;
                for trace in traces {
                    control.check_cancelled()?;
                    if trace.coordinate.0.iter().enumerate().all(|(axis, &value)| {
                        value >= start[axis] && value < start[axis] + shape[axis]
                    }) {
                        let sample_count = component_count
                            .checked_mul(direct_length)
                            .ok_or(ReadError::SizeOverflow)?;
                        let mut samples = reserve_complex(sample_count)?;
                        let source_direct = self.shape()[direct_axis]
                            .checked_mul(direct_lanes)
                            .ok_or(ReadError::SizeOverflow)?;
                        let output_direct = direct_length
                            .checked_mul(direct_lanes)
                            .ok_or(ReadError::SizeOverflow)?;
                        for component in 0..component_count / direct_lanes {
                            let source = component
                                .checked_mul(source_direct)
                                .and_then(|value| value.checked_add(direct_start * direct_lanes))
                                .ok_or(ReadError::SizeOverflow)?;
                            for chunk in trace.samples[source..source + output_direct].chunks(4096)
                            {
                                control.check_cancelled()?;
                                samples.extend_from_slice(chunk);
                            }
                        }
                        output.push(SparseTrace::new(
                            trace.ordinal,
                            trace.coordinate.clone(),
                            samples,
                        ));
                    }
                }
                if output.is_empty() {
                    return Err(AccessError::UnsampledRegion {
                        start: start.to_vec(),
                        shape: shape.to_vec(),
                    }
                    .into());
                }
                Self::sparse(
                    RawLayout::from_region(
                        shape.to_vec(),
                        self.component_lanes().to_vec(),
                        start.to_vec(),
                    )?,
                    output,
                )
                .map_err(ReadError::from)
            }
        }?;
        control.check_cancelled()?;
        Ok(crate::raw::RegionData::new(region.clone(), data))
    }
    /// Materializes sparse data with an explicit fill value.
    ///
    /// Repeated observations cannot be represented by one dense cell and
    /// return an ambiguous-access error instead of applying an implicit merge.
    pub fn materialize_dense(&self, fill: Complex64, max_bytes: usize) -> Result<Self, ReadError> {
        let storage_shape = self.storage_shape()?;
        let total = checked_product(&storage_shape).ok_or(ReadError::SizeOverflow)?;
        let requested_bytes = total
            .checked_mul(std::mem::size_of::<Complex64>())
            .ok_or(ReadError::SizeOverflow)?;
        if requested_bytes > max_bytes {
            return Err(ReadError::limit(
                ReadResource::MaterializedBytes,
                max_bytes,
                requested_bytes,
            ));
        }
        match &self.representation {
            SampleRepresentation::Dense(source) => {
                let mut samples = Vec::new();
                samples
                    .try_reserve_exact(total)
                    .map_err(|_| ReadError::allocation(requested_bytes))?;
                samples.extend_from_slice(source);
                Self::dense(self.layout.clone(), samples).map_err(Into::into)
            }
            SampleRepresentation::Sparse(traces) => {
                let mut unique = BTreeSet::new();
                if let Some(repeated) = traces
                    .iter()
                    .find(|trace| !unique.insert(&trace.coordinate.0))
                {
                    return Err(AccessError::AmbiguousObservation {
                        coordinate: repeated.coordinate.0.clone(),
                    }
                    .into());
                }
                let mut samples = Vec::new();
                samples
                    .try_reserve_exact(total)
                    .map_err(|_| ReadError::allocation(requested_bytes))?;
                samples.resize(total, fill);
                for trace in traces {
                    scatter_trace(
                        &mut samples,
                        &storage_shape,
                        self.shape(),
                        self.component_lanes(),
                        &trace.coordinate.0,
                        &trace.samples,
                    )?;
                }
                Ok(Self::dense(self.layout.clone(), samples)?)
            }
        }
    }
    /// Validates shape, lane, sample-count, finiteness, and sparse bounds.
    pub fn validate(&self) -> Result<(), ValidationError> {
        RawLayout::validate_parts(
            self.shape(),
            self.component_lanes(),
            self.layout.absolute_origin(),
        )?;
        let total = self
            .shape()
            .iter()
            .zip(self.component_lanes())
            .try_fold(1usize, |total, (&points, &lanes)| {
                total.checked_mul(points)?.checked_mul(lanes)
            })
            .ok_or(ValidationError::SizeOverflow)?;
        match &self.representation {
            SampleRepresentation::Dense(samples) if samples.len() != total => {
                Err(ValidationError::DenseLengthMismatch)
            }
            SampleRepresentation::Dense(samples)
                if samples
                    .iter()
                    .any(|sample| !sample.re.is_finite() || !sample.im.is_finite()) =>
            {
                Err(ValidationError::NonFiniteSample)
            }
            SampleRepresentation::Dense(_) => Ok(()),
            SampleRepresentation::Sparse(traces) => {
                if traces.is_empty() {
                    return Err(ValidationError::EmptySparseData);
                }
                let trace_len = self.shape()[self.shape().len() - 1]
                    .checked_mul(
                        checked_product(self.component_lanes())
                            .ok_or(ValidationError::SizeOverflow)?,
                    )
                    .ok_or(ValidationError::SizeOverflow)?;
                // Readers and region crops already preserve increasing ordinals.
                // Only externally constructed unordered observations need an index.
                if !traces
                    .windows(2)
                    .all(|pair| pair[0].ordinal < pair[1].ordinal)
                {
                    let requested_bytes = traces
                        .len()
                        .checked_mul(std::mem::size_of::<ObservationOrdinal>())
                        .filter(|&bytes| bytes <= isize::MAX as usize)
                        .ok_or(ValidationError::SizeOverflow)?;
                    let mut ordinals = Vec::new();
                    ordinals
                        .try_reserve_exact(traces.len())
                        .map_err(|_| ValidationError::Allocation { requested_bytes })?;
                    ordinals.extend(traces.iter().map(|trace| trace.ordinal));
                    ordinals.sort_unstable();
                    if ordinals.windows(2).any(|pair| pair[0] == pair[1]) {
                        return Err(ValidationError::DuplicateObservationOrdinal);
                    }
                }
                for trace in traces {
                    if trace.coordinate.0.len() + 1 != self.shape().len() {
                        return Err(ValidationError::SamplingRankMismatch);
                    }
                    if trace.coordinate.0.iter().enumerate().any(|(axis, &value)| {
                        let origin = self.layout.absolute_origin()[axis];
                        value < origin || value >= origin + self.shape()[axis]
                    }) {
                        return Err(ValidationError::SamplingOutOfBounds);
                    }
                    if trace.samples.len() != trace_len {
                        return Err(ValidationError::SparseTraceLengthMismatch);
                    }
                    if trace
                        .samples
                        .iter()
                        .any(|sample| !sample.re.is_finite() || !sample.im.is_finite())
                    {
                        return Err(ValidationError::NonFiniteSample);
                    }
                }
                Ok(())
            }
        }
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawData {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&RawLayout, &SampleRepresentation) {
        (&self.layout, &self.representation)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (RawLayout, SampleRepresentation),
    ) -> Result<Self, crate::internal::ModelError> {
        let (layout, representation) = parts;
        let value = Self {
            layout,
            representation,
        };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
