use crate::SourceFile;
use crate::provenance::CanonicalDatasetDigests;
use crate::raw::{RawDataset, RawDescriptor, SamplingSchedule};

/// A sample-free snapshot produced only from a validated raw dataset.
#[derive(Clone, Debug, PartialEq)]
pub struct RawDatasetSnapshot {
    descriptor: RawDescriptor,
    sources: Vec<SourceFile>,
    sample_normalization: crate::provenance::SampleNormalization,
    sampling: Option<SamplingSchedule>,
    absolute_origin: Box<[usize]>,
    canonical_digests: CanonicalDatasetDigests,
}

impl RawDatasetSnapshot {
    pub(crate) fn validate_recorded(
        &self,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        let token = Some(control.cancellation());
        let descriptor =
            crate::canonical_digest::descriptor_digest_controlled(&self.descriptor, token)?;
        if self.absolute_origin.as_ref() != self.descriptor.layout().absolute_origin()
            || crate::canonical_digest::raw_binding(
                descriptor,
                self.canonical_digests.samples(),
                self.sampling.as_ref(),
                token,
            )? != self.canonical_digests
        {
            return Err(ModelError::DigestMismatch);
        }
        if let Some(schedule) = &self.sampling {
            if schedule.declaration().is_some_and(|d| {
                d.indirect_lanes()
                    != &self.descriptor.component_lanes()[..self.descriptor.axes().len() - 1]
            }) {
                return Err(ModelError::Structure);
            }
            if schedule
                .grid()
                .iter()
                .copied()
                .ne(self.descriptor.axes()[..self.descriptor.axes().len() - 1]
                    .iter()
                    .map(|a| a.points()))
            {
                return Err(ModelError::Structure);
            }
        }
        Ok(())
    }

    /// Returns the validated raw descriptor captured by the snapshot.
    pub fn descriptor(&self) -> &RawDescriptor {
        &self.descriptor
    }

    /// Returns source provenance captured from the raw dataset.
    pub fn sources(&self) -> &[SourceFile] {
        &self.sources
    }

    /// Returns source-block scaling and stored-imaginary sign normalization.
    pub fn sample_normalization(&self) -> &crate::provenance::SampleNormalization {
        &self.sample_normalization
    }

    /// Returns the raw logical sampling schedule, when present.
    pub fn sampling_schedule(&self) -> Option<&SamplingSchedule> {
        self.sampling.as_ref()
    }

    /// Returns the absolute full-grid origin of the captured sample layout.
    pub fn absolute_origin(&self) -> &[usize] {
        &self.absolute_origin
    }

    /// Returns the strong canonical identities bound to processing history.
    pub fn canonical_digests(&self) -> CanonicalDatasetDigests {
        self.canonical_digests
    }
}

impl RawDataset {
    /// Captures validated raw axis and source evidence without copying samples.
    pub fn snapshot(&self) -> RawDatasetSnapshot {
        debug_assert!(self.validate().is_ok());
        self.snapshot_with_digests(self.canonical_digests())
    }

    pub(crate) fn snapshot_with_digests(
        &self,
        canonical_digests: CanonicalDatasetDigests,
    ) -> RawDatasetSnapshot {
        RawDatasetSnapshot {
            descriptor: self.descriptor().clone(),
            sources: self.provenance().sources().to_vec(),
            sample_normalization: self.provenance().sample_normalization().clone(),
            sampling: self.sampling_schedule().cloned(),
            absolute_origin: self.data().layout().absolute_origin().into(),
            canonical_digests,
        }
    }
}

/// Declared origin of a processed data product.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessedOrigin {
    /// Library-executed derivation with complete ordered input evidence.
    Library(Box<crate::derivation::LibraryDerivation>),
    /// Host-executed algorithm with a checked fresh processing boundary.
    External(Box<crate::external::ExternalBoundary>),
    /// Imported as an already processed product.
    Imported,
    /// The caller declares descent from a validated raw dataset.
    DeclaredRaw {
        /// Sample-free raw snapshot.
        snapshot: Box<RawDatasetSnapshot>,
        /// Input-slot/axis reference for each processed axis. This declaration
        /// has one snapshot in slot zero; references must be unique and in range.
        axis_lineage: Vec<crate::provenance::InputAxisRef>,
    },
    /// Produced by this crate from a validated raw dataset.
    DerivedRaw(DerivedRawOrigin),
    /// No trustworthy origin statement is available.
    Unknown,
}

/// Read-only evidence that this crate expanded a particular raw dataset.
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedRawOrigin {
    snapshot: Box<RawDatasetSnapshot>,
    axis_lineage: Vec<crate::provenance::InputAxisRef>,
}

impl DerivedRawOrigin {
    /// Returns the sample-free raw snapshot.
    pub fn snapshot(&self) -> &RawDatasetSnapshot {
        &self.snapshot
    }

    /// Returns each processed axis's reference to this origin's raw input slot zero.
    pub fn axis_lineage(&self) -> &[crate::provenance::InputAxisRef] {
        &self.axis_lineage
    }

    pub(crate) fn new(snapshot: RawDatasetSnapshot, rank: usize) -> Self {
        Self {
            snapshot: Box::new(snapshot),
            axis_lineage: (0..rank)
                .map(crate::provenance::InputAxisRef::single)
                .collect(),
        }
    }
}
// Crate-private model decomposition; no wire tags or encoding policy.
impl DerivedRawOrigin {
    // Preserve the exact owned-slot type at the read-only model boundary.
    #[allow(clippy::type_complexity, clippy::borrowed_box)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Box<RawDatasetSnapshot>,
        &Vec<crate::provenance::InputAxisRef>,
    ) {
        (&self.snapshot, &self.axis_lineage)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Box<RawDatasetSnapshot>,
            Vec<crate::provenance::InputAxisRef>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (snapshot, axis_lineage) = parts;
        let value = Self {
            snapshot,
            axis_lineage,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawDatasetSnapshot {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &RawDescriptor,
        &Vec<SourceFile>,
        &crate::provenance::SampleNormalization,
        &Option<SamplingSchedule>,
        &Box<[usize]>,
        &CanonicalDatasetDigests,
    ) {
        (
            &self.descriptor,
            &self.sources,
            &self.sample_normalization,
            &self.sampling,
            &self.absolute_origin,
            &self.canonical_digests,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            RawDescriptor,
            Vec<SourceFile>,
            crate::provenance::SampleNormalization,
            Option<SamplingSchedule>,
            Box<[usize]>,
            CanonicalDatasetDigests,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            descriptor,
            sources,
            sample_normalization,
            sampling,
            absolute_origin,
            canonical_digests,
        ) = parts;
        let value = Self {
            descriptor,
            sources,
            sample_normalization,
            sampling,
            absolute_origin,
            canonical_digests,
        };

        Ok(value)
    }
}
