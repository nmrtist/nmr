/// Resource limits applied while opening and reading a dataset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadLimits {
    max_source_bytes: usize,
    max_working_bytes: usize,
    max_region_bytes: usize,
    max_materialized_bytes: usize,
    max_metadata_bytes: usize,
    max_component_lanes: usize,
    max_transform_coefficients: usize,
    max_modulation_period: usize,
    max_transform_work: usize,
    retained_working_bytes: usize,
}

impl ReadLimits {
    /// Returns the default limits.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum combined size of all numeric sample files.
    pub fn max_source_bytes(mut self, value: usize) -> Self {
        self.max_source_bytes = value;
        self
    }

    /// Sets a bound on simultaneous temporary decoding and mapping buffers.
    /// Includes owned numeric input bytes, intermediate samples and data indexes.
    /// Lazy Reader accesses include the final region/materialized sample buffer,
    /// adapter decoding buffers and any cropped trace that coexist. Trace,
    /// region, layout and observation container payloads are also included.
    /// Separate region/materialized limits also apply. Other complete-read entry points
    /// retain their documented decoding estimates. Bruker raw additionally
    /// checks combined parameter-table expansion before parsing, including
    /// owned path text, and retains the parsed-table charge during decoding
    /// and Reader access. Lazy Bruker NUS also charges its schedule and row map.
    /// Lazy Bruker source records include their owned roles and locator paths.
    /// Its boxed adapter is included as well.
    /// Lazy Bruker descriptors reserve axes, text, evidence and component transforms
    /// before construction, conservatively retaining their construction bound.
    /// Borrowed caller input,
    /// other retained metadata structures, allocator overhead and fixed-size stack
    /// state are excluded; this is not yet the shared full-memory contract and
    /// is not a bound on process RSS or on concurrent calls combined.
    pub fn max_working_bytes(mut self, value: usize) -> Self {
        self.max_working_bytes = value;
        self
    }

    /// Sets the maximum allocation produced by one region read.
    pub fn max_region_bytes(mut self, value: usize) -> Self {
        self.max_region_bytes = value;
        self
    }

    /// Sets the maximum allocation produced by materializing a complete dataset.
    pub fn max_materialized_bytes(mut self, value: usize) -> Self {
        self.max_materialized_bytes = value;
        self
    }

    /// Sets the maximum combined metadata input size.
    pub fn max_metadata_bytes(mut self, value: usize) -> Self {
        self.max_metadata_bytes = value;
        self
    }

    /// Sets the maximum number of raw lanes on any one axis.
    pub fn max_component_lanes(mut self, value: usize) -> Self {
        self.max_component_lanes = value;
        self
    }

    /// Sets the maximum coefficient count of any resolved component transform.
    pub fn max_transform_coefficients(mut self, value: usize) -> Self {
        self.max_transform_coefficients = value;
        self
    }

    /// Sets the maximum periodic modulation length in phases.
    pub fn max_modulation_period(mut self, value: usize) -> Self {
        self.max_modulation_period = value;
        self
    }

    /// Sets the maximum charged component-transform multiply-accumulate work.
    pub fn max_transform_work(mut self, value: usize) -> Self {
        self.max_transform_work = value;
        self
    }

    /// Returns the combined numeric-sample input limit.
    pub fn source_bytes(&self) -> usize {
        self.max_source_bytes
    }

    /// Returns the working-memory limit.
    pub fn working_bytes(&self) -> usize {
        self.max_working_bytes
    }

    /// Returns the per-region output limit.
    pub fn region_bytes(&self) -> usize {
        self.max_region_bytes
    }

    /// Returns the complete-dataset output limit.
    pub fn materialized_bytes(&self) -> usize {
        self.max_materialized_bytes
    }

    /// Returns the metadata limit.
    pub fn metadata_bytes(&self) -> usize {
        self.max_metadata_bytes
    }

    /// Returns the per-axis raw lane limit.
    pub fn component_lanes(&self) -> usize {
        self.max_component_lanes
    }
    /// Returns the component-transform coefficient limit.
    pub fn transform_coefficients(&self) -> usize {
        self.max_transform_coefficients
    }
    /// Returns the periodic modulation phase limit.
    pub fn modulation_period(&self) -> usize {
        self.max_modulation_period
    }
    /// Returns the component-transform work limit.
    pub fn transform_work(&self) -> usize {
        self.max_transform_work
    }

    pub(crate) fn with_retained_working(mut self, bytes: usize) -> Result<Self, crate::ReadError> {
        self.retained_working_bytes = self
            .retained_working_bytes
            .checked_add(bytes)
            .ok_or(crate::ReadError::SizeOverflow)?;
        self.check_working(crate::raw::ReadResource::WorkingBytes, 0)?;
        Ok(self)
    }

    pub(crate) fn check_working(
        &self,
        resource: crate::raw::ReadResource,
        temporary_bytes: usize,
    ) -> Result<(), crate::ReadError> {
        let required = self
            .retained_working_bytes
            .checked_add(temporary_bytes)
            .ok_or(crate::ReadError::SizeOverflow)?;
        if required > self.max_working_bytes {
            return Err(crate::ReadError::limit(
                resource,
                self.max_working_bytes,
                required,
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_descriptor(
        &self,
        descriptor: &crate::raw::RawDescriptor,
    ) -> Result<(), crate::ReadError> {
        use crate::raw::{IndirectComponents, ReadResource};
        let logical_points = descriptor
            .layout()
            .logical_shape()
            .iter()
            .try_fold(1usize, |total, &points| total.checked_mul(points))
            .ok_or(crate::ReadError::SizeOverflow)?;
        for axis in descriptor.axes() {
            let lanes = axis.component_lanes();
            if lanes > self.max_component_lanes {
                return Err(crate::ReadError::limit(
                    ReadResource::ComponentLanes,
                    self.max_component_lanes,
                    lanes,
                ));
            }
            if let crate::raw::RawAxisKind::Indirect(IndirectComponents::Encoded(resolved)) =
                axis.kind()
            {
                let transform = resolved.transform();
                if transform.coefficients().len() > self.max_transform_coefficients {
                    return Err(crate::ReadError::limit(
                        ReadResource::TransformCoefficients,
                        self.max_transform_coefficients,
                        transform.coefficients().len(),
                    ));
                }
                if transform.modulation().period() > self.max_modulation_period {
                    return Err(crate::ReadError::limit(
                        ReadResource::ModulationPeriod,
                        self.max_modulation_period,
                        transform.modulation().period(),
                    ));
                }
                let work = logical_points
                    .checked_mul(transform.input_lanes())
                    .and_then(|value| value.checked_mul(2))
                    .ok_or(crate::ReadError::SizeOverflow)?;
                if work > self.max_transform_work {
                    return Err(crate::ReadError::limit(
                        ReadResource::TransformWork,
                        self.max_transform_work,
                        work,
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 1024 * 1024 * 1024,
            max_working_bytes: 512 * 1024 * 1024,
            max_region_bytes: 512 * 1024 * 1024,
            max_materialized_bytes: 512 * 1024 * 1024,
            max_metadata_bytes: 64 * 1024 * 1024,
            max_component_lanes: 64,
            max_transform_coefficients: 128,
            max_modulation_period: 1024,
            max_transform_work: 1_000_000_000,
            retained_working_bytes: 0,
        }
    }
}
