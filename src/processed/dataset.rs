//! Complete processed provenance and dataset values above processing history.

use crate::axis::AxisRole;
use crate::processed::SourceMetadata;
use crate::processed::model::{
    ProcessedData, ProcessedDescriptor, ProcessedOrigin, ProcessedValidationError,
};
use crate::processing::contracts::history::ProcessingHistory;
use crate::provenance::SourceFile;

/// Immutable source and origin statement for processed data.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessedProvenance {
    origin: ProcessedOrigin,
    sources: Vec<SourceFile>,
    history: Option<ProcessingHistory>,
    source_metadata: SourceMetadata,
    read_record: Option<crate::provenance::ProcessedReadRecord>,
}

impl ProcessedProvenance {
    pub(crate) fn library(
        boundary: crate::derivation::LibraryDerivation,
        sources: Vec<SourceFile>,
    ) -> Self {
        Self {
            origin: ProcessedOrigin::Library(Box::new(boundary)),
            sources,
            history: None,
            source_metadata: SourceMetadata::default(),
            read_record: None,
        }
    }

    pub(crate) fn history_mut(&mut self) -> Option<&mut ProcessingHistory> {
        self.history.as_mut()
    }
    pub(crate) fn accept_external_ancestors(&mut self) {
        if let ProcessedOrigin::Library(boundary) = &mut self.origin {
            boundary.accept_archived_history();
        }
        if let ProcessedOrigin::External(boundary) = &mut self.origin {
            boundary.accept_archived_history();
        }
    }
    /// Creates a provenance statement.
    pub fn new(
        origin: ProcessedOrigin,
        sources: Vec<SourceFile>,
    ) -> Result<Self, ProcessedValidationError> {
        if matches!(
            origin,
            ProcessedOrigin::DerivedRaw(_)
                | ProcessedOrigin::External(_)
                | ProcessedOrigin::Library(_)
        ) {
            return Err(ProcessedValidationError::LibraryDerivedProvenance);
        }
        Ok(Self {
            origin,
            sources,
            history: None,
            source_metadata: SourceMetadata::default(),
            read_record: None,
        })
    }

    /// Returns the declared origin.
    pub fn origin(&self) -> &ProcessedOrigin {
        &self.origin
    }

    /// Returns source files in caller-defined order.
    pub fn sources(&self) -> &[SourceFile] {
        &self.sources
    }

    /// Returns immutable processing history when this crate has applied operations.
    pub fn history(&self) -> Option<&ProcessingHistory> {
        self.history.as_ref()
    }

    /// Returns opaque source metadata retained by an imported-format reader.
    pub fn source_metadata(&self) -> &SourceMetadata {
        &self.source_metadata
    }

    /// Returns verified applied decoding for migrated processed readers.
    pub fn read_record(&self) -> Option<&crate::provenance::ProcessedReadRecord> {
        self.read_record.as_ref()
    }

    pub(crate) fn with_read_record(
        mut self,
        record: crate::provenance::ProcessedReadRecord,
    ) -> Self {
        self.read_record = Some(record);
        self
    }

    pub(crate) fn derived(
        origin: ProcessedOrigin,
        sources: Vec<SourceFile>,
        mut history: ProcessingHistory,
    ) -> Self {
        history.inherit_segments(None);
        Self {
            origin,
            sources,
            history: Some(history),
            source_metadata: SourceMetadata::default(),
            read_record: None,
        }
    }

    pub(crate) fn derived_from(source: &Self, mut history: ProcessingHistory) -> Self {
        history.inherit_segments(source.history.as_ref());
        Self {
            origin: source.origin.clone(),
            sources: source.sources.clone(),
            history: Some(history),
            source_metadata: source.source_metadata.clone(),
            read_record: source.read_record.clone(),
        }
    }
}

/// A checked processed data product.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessedDataset {
    state: crate::processing::contracts::state::PlanState,
    digests: crate::provenance::CanonicalDatasetDigests,
    descriptor: ProcessedDescriptor,
    data: ProcessedData,
    provenance: ProcessedProvenance,
}

impl ProcessedDataset {
    pub(crate) fn new_library(
        control: &mut crate::ExecutionContext<'_>,
        descriptor: ProcessedDescriptor,
        data: ProcessedData,
        provenance: ProcessedProvenance,
    ) -> Result<Self, crate::processing::ProcessingError> {
        let ProcessedOrigin::Library(boundary) = provenance.origin() else {
            return Err(crate::processing::ProcessingError::Mapping(
                "expected library boundary",
            ));
        };
        let state = boundary.state.clone();
        let digests = crate::canonical_digest::processed_digests(&descriptor, &data, &state);
        let value = Self {
            state,
            digests,
            descriptor,
            data,
            provenance,
        };
        value.validate()?;
        control.check_cancelled()?;
        Ok(value)
    }

    pub(crate) fn accept_archived_history(&mut self) {
        if let Some(history) = &mut self.provenance.history {
            history.accept_archived_history();
        }
        if let ProcessedOrigin::Library(boundary) = &mut self.provenance.origin {
            boundary.accept_archived_history();
        }
        if let ProcessedOrigin::External(boundary) = &mut self.provenance.origin {
            boundary.accept_archived_history();
        }
    }
    pub(crate) fn validate_recorded(
        &self,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        if let Some(history) = self.provenance.history() {
            history.validate_recorded()?;
            history.validate_recorded_evidence(
                self.provenance.origin(),
                self.provenance.sources(),
                control,
            )?;
        }
        if let ProcessedOrigin::Library(boundary) = self.provenance.origin() {
            boundary.validate_recorded(control)?;
        }
        if let ProcessedOrigin::External(boundary) = self.provenance.origin() {
            boundary.validate_recorded(control)?;
        }
        match self.provenance.origin() {
            ProcessedOrigin::DerivedRaw(origin) => origin.snapshot().validate_recorded(control)?,
            ProcessedOrigin::DeclaredRaw { snapshot, .. } => snapshot.validate_recorded(control)?,
            _ => {}
        }
        self.validate()
            .map_err(|e| ModelError::Validation(e.to_string()))?;
        if let Some(record) = self.provenance.read_record() {
            if let Some(history) = self.provenance.history() {
                if !matches!(history.input(), crate::processing::contracts::history::HistoryInput::Processed { read_record: Some(input), .. } if input == record)
                {
                    return Err(ModelError::Structure);
                }
            } else {
                record.validate_recorded(
                    &self.descriptor,
                    self.digests,
                    self.provenance.sources(),
                )?;
            }
        }
        let state = processing_state(&self.descriptor, &self.provenance)
            .map_err(|e| ModelError::Validation(e.to_string()))?;
        if state != self.state {
            return Err(ModelError::Structure);
        }
        let actual = crate::canonical_digest::processed_digests_controlled(
            &self.descriptor,
            &self.data,
            &state,
            Some(control.cancellation()),
        )?;
        if actual != self.digests {
            return Err(ModelError::DigestMismatch);
        }
        if let Some(history) = self.provenance.history() {
            if history
                .segments()
                .last()
                .is_none_or(|s| s.output_digests() != actual)
            {
                return Err(ModelError::DigestMismatch);
            }
        } else if let ProcessedOrigin::Library(boundary) = self.provenance.origin() {
            if boundary.canonical_digests() != actual || boundary.descriptor() != &self.descriptor {
                return Err(ModelError::DigestMismatch);
            }
        } else if let ProcessedOrigin::External(boundary) = self.provenance.origin() {
            if boundary.canonical_digests() != actual || boundary.descriptor() != &self.descriptor {
                return Err(ModelError::DigestMismatch);
            }
        }
        Ok(())
    }
    pub(crate) fn attach_external_boundary(&mut self, boundary: crate::external::ExternalBoundary) {
        self.provenance.origin = ProcessedOrigin::External(Box::new(boundary));
    }
    /// Constructs checked dense samples using descriptor-derived shape and components.
    pub fn from_dense_samples(
        descriptor: ProcessedDescriptor,
        samples: Vec<f64>,
        provenance: ProcessedProvenance,
    ) -> Result<Self, ProcessedValidationError> {
        let data = ProcessedData::from_descriptor(&descriptor, samples)?;
        Self::new(descriptor, data, provenance)
    }

    /// Constructs a scalar frequency spectrum with caller-supplied calibration and provenance.
    pub fn scalar_spectrum(
        axis: crate::processed::ProcessedAxis,
        samples: Vec<f64>,
        provenance: ProcessedProvenance,
    ) -> Result<Self, ProcessedValidationError> {
        if axis.domain() != crate::axis::AxisDomain::Frequency
            || !matches!(
                axis.component_basis(),
                crate::processed::ComponentBasis::Scalar
            )
        {
            return Err(ProcessedValidationError::DescriptorDataMismatch);
        }
        Self::from_dense_samples(ProcessedDescriptor::new(vec![axis])?, samples, provenance)
    }

    /// Constructs a one-axis Cartesian trace with caller-supplied coordinates and provenance.
    pub fn from_complex_trace(
        axis: crate::processed::ProcessedAxis,
        samples: Vec<crate::Complex64>,
        provenance: ProcessedProvenance,
    ) -> Result<Self, ProcessedValidationError> {
        if !matches!(
            axis.component_basis(),
            crate::processed::ComponentBasis::Cartesian
        ) || samples.len() != axis.points()
        {
            return Err(ProcessedValidationError::DescriptorDataMismatch);
        }
        Self::from_dense_samples(
            ProcessedDescriptor::new(vec![axis])?,
            samples.into_iter().flat_map(|z| [z.re, z.im]).collect(),
            provenance,
        )
    }

    /// Iterates Cartesian values along `axis`. Coordinate arrays have full rank;
    /// the traversed logical coordinate and selected Cartesian lane are ignored.
    /// Other logical coordinates and component lanes remain fixed.
    pub fn complex_trace(
        &self,
        axis: crate::acquisition::AxisIndex,
        logical: &[usize],
        component_axis: crate::acquisition::AxisIndex,
        components: &[usize],
    ) -> Result<crate::processed::ComplexTrace<'_>, crate::processed::ProcessedAccessError> {
        if !self
            .descriptor
            .axes()
            .get(component_axis.index())
            .is_some_and(|a| {
                matches!(
                    a.component_basis(),
                    crate::processed::ComponentBasis::Cartesian
                )
            })
        {
            return Err(crate::processed::ProcessedAccessError::NotCartesian);
        }
        crate::processed::ComplexTrace::new(
            &self.data,
            axis.index(),
            logical,
            component_axis.index(),
            components,
        )
    }
    /// Creates and aggregate-validates a processed dataset.
    pub fn new(
        descriptor: ProcessedDescriptor,
        data: ProcessedData,
        provenance: ProcessedProvenance,
    ) -> Result<Self, ProcessedValidationError> {
        if matches!(
            provenance.origin,
            ProcessedOrigin::DerivedRaw(_)
                | ProcessedOrigin::External(_)
                | ProcessedOrigin::Library(_)
        ) || provenance.history.is_some()
            || provenance.read_record.is_some()
        {
            return Err(ProcessedValidationError::LibraryDerivedProvenance);
        }
        let state = processing_state(&descriptor, &provenance)?;
        let value = Self {
            digests: crate::canonical_digest::processed_digests(&descriptor, &data, &state),
            state,
            descriptor,
            data,
            provenance,
        };
        value.validate()?;
        Ok(value)
    }

    #[cfg(test)]
    pub(crate) fn new_derived(
        descriptor: ProcessedDescriptor,
        data: ProcessedData,
        provenance: ProcessedProvenance,
    ) -> Result<Self, ProcessedValidationError> {
        Self::new_derived_with_context(
            &mut crate::ExecutionContext::default(),
            descriptor,
            data,
            provenance,
        )
        .map_err(|error| match error {
            crate::processing::ProcessingError::Validation(error) => error,
            _ => unreachable!("no execution limits in model construction"),
        })
    }

    pub(crate) fn new_derived_with_context(
        control: &mut crate::ExecutionContext<'_>,
        descriptor: ProcessedDescriptor,
        data: ProcessedData,
        provenance: ProcessedProvenance,
    ) -> Result<Self, crate::processing::ProcessingError> {
        control.check_cancelled()?;
        let state = processing_state(&descriptor, &provenance)?;
        let mut value = Self {
            digests: crate::canonical_digest::processed_digests_controlled(
                &descriptor,
                &data,
                &state,
                Some(control.cancellation()),
            )?,
            state,
            descriptor,
            data,
            provenance,
        };
        // processing_state already validated the complete history and produced
        // this exact immutable state. Recheck structure without replaying again.
        value.validate_structure()?;
        if let Some(history) = &mut value.provenance.history {
            history.finish_segment(value.digests);
        }
        control.check_cancelled()?;
        Ok(value)
    }

    pub(crate) fn new_imported(
        descriptor: ProcessedDescriptor,
        data: ProcessedData,
        provenance: ProcessedProvenance,
        source_metadata: SourceMetadata,
    ) -> Result<Self, ProcessedValidationError> {
        let mut provenance = provenance;
        provenance.source_metadata = source_metadata;
        let state = processing_state(&descriptor, &provenance)?;
        let value = Self {
            digests: crate::canonical_digest::processed_digests(&descriptor, &data, &state),
            state,
            descriptor,
            data,
            provenance,
        };
        value.validate()?;
        Ok(value)
    }

    /// Returns canonical descriptor/state, samples, and combined identities.
    ///
    /// Canonical identities exclude display labels and source paths.
    pub fn canonical_digests(&self) -> crate::provenance::CanonicalDatasetDigests {
        self.digests
    }

    pub(crate) fn processing_state(&self) -> &crate::processing::contracts::state::PlanState {
        &self.state
    }

    /// Returns the processed descriptor.
    pub fn descriptor(&self) -> &ProcessedDescriptor {
        &self.descriptor
    }

    /// Returns current calibration and filter evidence, indexed by the current
    /// descriptor (including after slicing, projection and snapshot restoration).
    /// Returns None for an out-of-range axis. This does not replay history.
    pub fn axis_evidence(
        &self,
        axis: usize,
    ) -> Option<crate::processed::ProcessedAxisEvidence<'_>> {
        self.state
            .axes
            .get(axis)
            .map(crate::processed::ProcessedAxisEvidence)
    }

    /// Returns dense component-tensor storage.
    pub fn data(&self) -> &ProcessedData {
        &self.data
    }

    /// Returns immutable provenance.
    pub fn provenance(&self) -> &ProcessedProvenance {
        &self.provenance
    }

    fn validate_structure(&self) -> Result<(), ProcessedValidationError> {
        self.descriptor.validate()?;
        if self
            .descriptor
            .axes()
            .iter()
            .map(|axis| axis.points())
            .ne(self.data.shape().iter().copied())
            || self
                .descriptor
                .axes()
                .iter()
                .map(|axis| axis.component_count())
                .ne(self.data.component_counts().iter().copied())
        {
            return Err(ProcessedValidationError::DescriptorDataMismatch);
        }
        self.data.validate()?;
        if let ProcessedOrigin::DeclaredRaw {
            snapshot,
            axis_lineage,
        } = &self.provenance.origin
        {
            let raw_axes = snapshot.descriptor().axes();
            if axis_lineage.len() != self.descriptor.axes().len() {
                return Err(ProcessedValidationError::OriginRankMismatch);
            }
            for (processed_index, (&reference, processed_axis)) in
                axis_lineage.iter().zip(self.descriptor.axes()).enumerate()
            {
                let raw_index = reference.axis();
                if reference.input().index() != 0
                    || raw_index >= raw_axes.len()
                    || axis_lineage[..processed_index].contains(&reference)
                {
                    return Err(ProcessedValidationError::InvalidAxisLineage {
                        axis: processed_index,
                    });
                }
                let raw_axis = &raw_axes[raw_index];
                if !roles_compatible(raw_axis.role(), processed_axis.role())
                    || raw_axis
                        .nucleus()
                        .zip(processed_axis.nucleus())
                        .is_some_and(|(raw, processed)| raw != processed)
                {
                    return Err(ProcessedValidationError::OriginAxisMismatch {
                        axis: processed_index,
                    });
                }
            }
        }
        Ok(())
    }

    /// Rechecks descriptor, storage, declared lineage, and processing history.
    pub fn validate(&self) -> Result<(), ProcessedValidationError> {
        self.validate_structure()?;
        if matches!(self.provenance.origin, ProcessedOrigin::DerivedRaw(_))
            || self.provenance.history.is_some()
        {
            let expected = crate::processing::contracts::history::validate_derived_history(
                self.provenance
                    .history
                    .as_ref()
                    .ok_or(ProcessedValidationError::InvalidProcessingHistory)?,
                &self.descriptor,
                &self.provenance.origin,
                &self.provenance.sources,
            )?;
            if expected != self.state {
                return Err(ProcessedValidationError::InvalidProcessingHistory);
            }
        }
        Ok(())
    }
}

pub(crate) fn processing_state(
    descriptor: &ProcessedDescriptor,
    provenance: &ProcessedProvenance,
) -> Result<crate::processing::contracts::state::PlanState, ProcessedValidationError> {
    let state = match &provenance.history {
        Some(history) => crate::processing::contracts::history::validate_derived_history(
            history,
            descriptor,
            &provenance.origin,
            &provenance.sources,
        )?,
        None => match provenance.origin() {
            ProcessedOrigin::Library(boundary) => boundary.state.clone(),
            _ => crate::processing::contracts::state::PlanState::from_descriptor(descriptor),
        },
    };
    Ok(state)
}

fn roles_compatible(raw: AxisRole, processed: AxisRole) -> bool {
    raw == processed || raw.is_signal() && processed == AxisRole::Signal
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedDataset {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &crate::processing::contracts::state::PlanState,
        &crate::provenance::CanonicalDatasetDigests,
        &ProcessedDescriptor,
        &ProcessedData,
        &ProcessedProvenance,
    ) {
        (
            &self.state,
            &self.digests,
            &self.descriptor,
            &self.data,
            &self.provenance,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            crate::processing::contracts::state::PlanState,
            crate::provenance::CanonicalDatasetDigests,
            ProcessedDescriptor,
            ProcessedData,
            ProcessedProvenance,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (state, digests, descriptor, data, provenance) = parts;
        let value = Self {
            state,
            digests,
            descriptor,
            data,
            provenance,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedProvenance {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &ProcessedOrigin,
        &Vec<SourceFile>,
        &Option<ProcessingHistory>,
        &SourceMetadata,
        &Option<crate::provenance::ProcessedReadRecord>,
    ) {
        (
            &self.origin,
            &self.sources,
            &self.history,
            &self.source_metadata,
            &self.read_record,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            ProcessedOrigin,
            Vec<SourceFile>,
            Option<ProcessingHistory>,
            SourceMetadata,
            Option<crate::provenance::ProcessedReadRecord>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (origin, sources, history, source_metadata, read_record) = parts;
        let value = Self {
            origin,
            sources,
            history,
            source_metadata,
            read_record,
        };

        Ok(value)
    }
}
