//! Checked boundaries for algorithms executed by the host application.

pub use crate::execution_report::ExternalAlgorithmDeclaration;
use crate::{
    Dataset,
    dataset::DatasetMetadata,
    processed::{ProcessedDataset, ProcessedDescriptor, ProcessedOrigin, ProcessedProvenance},
    provenance::{CanonicalDatasetDigests, InputAxisRef},
};

/// Source relationship for an output axis. It does not validate a coordinate transform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalAxisSource {
    /// Axis in the single parent, identified by input slot zero and descriptor position.
    Parent(InputAxisRef),
    /// A new axis with no parent-axis correspondence.
    New,
}

/// Immutable parent evidence, intentionally excluding parent samples.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessedEvidence {
    descriptor: ProcessedDescriptor,
    digests: CanonicalDatasetDigests,
    provenance: ProcessedProvenance,
    metadata: DatasetMetadata,
}
impl ProcessedEvidence {
    pub(crate) fn capture(input: &Dataset) -> Self {
        let parent = input.as_processed().expect("checked processed input");
        Self {
            descriptor: parent.descriptor().clone(),
            digests: input.canonical_digests(),
            provenance: parent.provenance().clone(),
            metadata: input.metadata().clone(),
        }
    }
    pub(crate) fn accept_archived_history(&mut self) {
        if let Some(h) = self.provenance.history_mut() {
            h.accept_archived_history();
        }
        self.provenance.accept_external_ancestors();
    }
    pub(crate) fn validate_recorded(
        &self,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        let provenance = &self.provenance;
        if let Some(history) = provenance.history() {
            history.validate_recorded()?;
            history.validate_recorded_evidence(
                provenance.origin(),
                provenance.sources(),
                control,
            )?;
            let state = crate::processing::contracts::history::validate_derived_history(
                history,
                &self.descriptor,
                provenance.origin(),
                provenance.sources(),
            )
            .map_err(|e| ModelError::Validation(e.to_string()))?;
            crate::canonical_digest::check_processed_evidence(
                &self.descriptor,
                &state,
                self.digests,
                Some(control.cancellation()),
            )?;
            if history
                .segments()
                .last()
                .is_none_or(|s| s.output_digests() != self.digests)
            {
                return Err(ModelError::DigestMismatch);
            }
        }
        if provenance.history().is_none() {
            if matches!(provenance.origin(), ProcessedOrigin::DerivedRaw(_)) {
                return Err(ModelError::Structure);
            }
            crate::canonical_digest::check_processed_evidence(
                &self.descriptor,
                &match provenance.origin() {
                    ProcessedOrigin::Library(boundary) => boundary.state.clone(),
                    _ => crate::processing::contracts::state::PlanState::from_descriptor(
                        &self.descriptor,
                    ),
                },
                self.digests,
                Some(control.cancellation()),
            )?;
        }
        match provenance.origin() {
            ProcessedOrigin::DerivedRaw(origin) => origin.snapshot().validate_recorded(control)?,
            ProcessedOrigin::DeclaredRaw { snapshot, .. } => snapshot.validate_recorded(control)?,
            _ => {}
        }
        if let ProcessedOrigin::External(boundary) = provenance.origin() {
            boundary.validate_recorded(control)?;
        }
        if let ProcessedOrigin::Library(boundary) = self.provenance.origin() {
            boundary.validate_recorded(control)?;
        }
        Ok(())
    }
    /// Parent's scientific description.
    pub fn descriptor(&self) -> &ProcessedDescriptor {
        &self.descriptor
    }
    /// Parent's recorded scientific identity; no ancestor samples are retained.
    pub fn canonical_digests(&self) -> CanonicalDatasetDigests {
        self.digests
    }
    /// Parent's complete source evidence and processing history.
    pub fn provenance(&self) -> &ProcessedProvenance {
        &self.provenance
    }
    /// Parent's original aggregate context.
    pub fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }
}

/// A host algorithm boundary and the new checked library-processing starting point.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalBoundary {
    parent: ProcessedEvidence,
    declaration: ExternalAlgorithmDeclaration,
    axes: Vec<ExternalAxisSource>,
    descriptor: ProcessedDescriptor,
    digests: CanonicalDatasetDigests,
}
impl ExternalBoundary {
    pub(crate) fn accept_archived_history(&mut self) {
        if let Some(history) = self.parent.provenance.history_mut() {
            history.accept_archived_history();
        }
        self.parent.provenance.accept_external_ancestors();
    }
    pub(crate) fn validate_recorded(
        &self,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        self.parent
            .descriptor
            .validate()
            .map_err(|e| ModelError::Validation(e.to_string()))?;
        self.descriptor
            .validate()
            .map_err(|e| ModelError::Validation(e.to_string()))?;
        if self.axes.len() != self.descriptor.axes().len() {
            return Err(ModelError::Structure);
        }
        for (i, source) in self.axes.iter().enumerate() {
            if let ExternalAxisSource::Parent(reference) = source {
                if reference.input().index() != 0
                    || reference.axis() >= self.parent.descriptor.axes().len()
                    || self.axes[..i].contains(source)
                {
                    return Err(ModelError::Structure);
                }
            }
        }
        self.parent.validate_recorded(control)?;
        Ok(())
    }
    /// Evidence for the parent, without its samples.
    pub fn parent(&self) -> &ProcessedEvidence {
        &self.parent
    }
    /// Host-authored algorithm statement, never a library `Applied` record.
    pub fn declaration(&self) -> &ExternalAlgorithmDeclaration {
        &self.declaration
    }
    /// Output-axis source relationships.
    pub fn axes(&self) -> &[ExternalAxisSource] {
        &self.axes
    }
    /// Starting descriptor of this new library-processing segment.
    pub fn descriptor(&self) -> &ProcessedDescriptor {
        &self.descriptor
    }
    /// Scientific identity the host must supply to replay this segment.
    pub fn canonical_digests(&self) -> CanonicalDatasetDigests {
        self.digests
    }
}

/// Failure to establish a checked host-algorithm boundary.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ExternalDerivationError {
    /// The parent must already be processed.
    #[error("external processed derivation requires a processed parent")]
    RawParent,
    /// Exactly one source declaration is required per output axis.
    #[error("external axis map rank differs from output rank")]
    AxisMapRank,
    /// A parent reference is out of range, duplicated, or uses a nonzero input slot.
    #[error("invalid external parent-axis reference for output axis {axis}")]
    AxisMap {
        /// Zero-based output axis.
        axis: usize,
    },
    /// Output descriptor or owned samples violate the scientific model.
    #[error(transparent)]
    Validation(#[from] crate::processed::ProcessedValidationError),
}

impl Dataset {
    /// Establishes a new processing origin for a host-produced result.
    /// State is rebuilt from the output descriptor: inherited FFT bins, phase
    /// facts, delay corrections and acquisition maps are discarded. This method
    /// neither executes nor proves the external algorithm or its coordinates.
    pub fn derive_external_processed(
        &self,
        descriptor: ProcessedDescriptor,
        samples: Vec<f64>,
        axes: Vec<ExternalAxisSource>,
        declaration: ExternalAlgorithmDeclaration,
    ) -> Result<Self, ExternalDerivationError> {
        let parent = self
            .as_processed()
            .ok_or(ExternalDerivationError::RawParent)?;
        if axes.len() != descriptor.axes().len() {
            return Err(ExternalDerivationError::AxisMapRank);
        }
        for (axis, source) in axes.iter().enumerate() {
            if let ExternalAxisSource::Parent(reference) = source {
                if reference.input().index() != 0
                    || reference.axis() >= parent.descriptor().axes().len()
                    || axes[..axis].contains(source)
                {
                    return Err(ExternalDerivationError::AxisMap { axis });
                }
            }
        }
        let mut output = ProcessedDataset::from_dense_samples(
            descriptor.clone(),
            samples,
            ProcessedProvenance::new(ProcessedOrigin::Unknown, self.sources().to_vec())?,
        )?;
        let boundary = ExternalBoundary {
            parent: ProcessedEvidence {
                descriptor: parent.descriptor().clone(),
                digests: self.canonical_digests(),
                provenance: parent.provenance().clone(),
                metadata: self.metadata().clone(),
            },
            declaration,
            axes,
            descriptor,
            digests: output.canonical_digests(),
        };
        output.attach_external_boundary(boundary);
        Ok(self.derived_processed(output))
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ExternalBoundary {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &ProcessedEvidence,
        &ExternalAlgorithmDeclaration,
        &Vec<ExternalAxisSource>,
        &ProcessedDescriptor,
        &CanonicalDatasetDigests,
    ) {
        (
            &self.parent,
            &self.declaration,
            &self.axes,
            &self.descriptor,
            &self.digests,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            ProcessedEvidence,
            ExternalAlgorithmDeclaration,
            Vec<ExternalAxisSource>,
            ProcessedDescriptor,
            CanonicalDatasetDigests,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (parent, declaration, axes, descriptor, digests) = parts;
        let value = Self {
            parent,
            declaration,
            axes,
            descriptor,
            digests,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessedEvidence {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &ProcessedDescriptor,
        &CanonicalDatasetDigests,
        &ProcessedProvenance,
        &DatasetMetadata,
    ) {
        (
            &self.descriptor,
            &self.digests,
            &self.provenance,
            &self.metadata,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            ProcessedDescriptor,
            CanonicalDatasetDigests,
            ProcessedProvenance,
            DatasetMetadata,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (descriptor, digests, provenance, metadata) = parts;
        let value = Self {
            descriptor,
            digests,
            provenance,
            metadata,
        };

        Ok(value)
    }
}
