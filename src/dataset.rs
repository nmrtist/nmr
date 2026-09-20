//! Materialized datasets with private storage and retained reading context.

use crate::processed::{ProcessedData, ProcessedDataset, ProcessedDescriptor, ProcessedProvenance};
use crate::provenance::CanonicalDatasetDigests;
use crate::raw::{RawDataset, RawDescriptor, RawProvenance};
use crate::{DatasetIdentity, DatasetKind, Format, ReadWarning};
use std::path::{Path, PathBuf};

/// Borrowed scientific description without conflating raw and processed semantics.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum DescriptorRef<'a> {
    /// Acquisition description.
    Raw(&'a RawDescriptor),
    /// Processed tensor description.
    Processed(&'a ProcessedDescriptor),
}

/// Borrowed provenance for the dataset's actual representation.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum ProvenanceRef<'a> {
    /// Acquisition provenance.
    Raw(&'a RawProvenance),
    /// Processed provenance and execution history.
    Processed(&'a ProcessedProvenance),
}

/// The original reader selection, retained even after subsequent processing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadSelection {
    format: Format,
    path: PathBuf,
}

impl ReadSelection {
    /// Returns the format selected for the original read, not the current data kind.
    pub fn format(&self) -> Format {
        self.format
    }
    /// Returns the caller's original path exactly as supplied.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Portable labels, warnings and optional original reading selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DatasetMetadata {
    identity: DatasetIdentity,
    selection: Option<ReadSelection>,
    warnings: Vec<ReadWarning>,
    accepted_archive: bool,
}

impl DatasetMetadata {
    /// Whether this aggregate was restored after explicit acceptance of recorded history.
    pub fn accepted_archive(&self) -> bool {
        self.accepted_archive
    }
    /// Returns portable identity evidence; absent labels remain unknown.
    pub fn identity(&self) -> &DatasetIdentity {
        &self.identity
    }
    /// Returns the original reading selection, if this aggregate came from `read`.
    pub fn selection(&self) -> Option<&ReadSelection> {
        self.selection.as_ref()
    }
    /// Returns non-fatal warnings retained from import.
    pub fn warnings(&self) -> &[ReadWarning] {
        &self.warnings
    }
}

#[derive(Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum DatasetData {
    Raw(RawDataset),
    Processed(ProcessedDataset),
}

/// An immutable materialized dataset with private representation and reading context.
///
/// Borrow the explicit raw/processed views when needed. This aggregate does not
/// promise a contiguous slice or an implicit deep clone for every representation.
#[derive(Debug, PartialEq)]
pub struct Dataset {
    data: DatasetData,
    metadata: DatasetMetadata,
    digests: CanonicalDatasetDigests,
}

impl Dataset {
    /// Wraps a checked raw dataset without copying samples or inventing a read selection.
    pub fn from_raw(value: RawDataset) -> Self {
        let digests = value.canonical_digests();
        Self {
            data: DatasetData::Raw(value),
            metadata: DatasetMetadata::default(),
            digests,
        }
    }
    /// Wraps raw data while computing its identity with cooperative cancellation.
    pub fn from_raw_with_context(
        value: RawDataset,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<Self, crate::execution::ExecutionError> {
        control.begin(crate::execution::ExecutionStage::Digest, None, None)?;
        let digests = crate::canonical_digest::dataset_digests_controlled(
            &value,
            Some(control.cancellation()),
        )?;
        Ok(Self {
            data: DatasetData::Raw(value),
            metadata: DatasetMetadata::default(),
            digests,
        })
    }

    /// Wraps a checked processed dataset without copying samples or granting new provenance.
    pub fn from_processed(value: ProcessedDataset) -> Self {
        let digests = value.canonical_digests();
        Self {
            data: DatasetData::Processed(value),
            metadata: DatasetMetadata::default(),
            digests,
        }
    }
    pub(crate) fn attach_read_context(
        mut self,
        format: Format,
        identity: DatasetIdentity,
        path: PathBuf,
        warnings: Vec<ReadWarning>,
    ) -> Result<Self, crate::ReadError> {
        if self.kind() != format.kind() {
            return Err(crate::ReadError::new(
                None,
                crate::ReadErrorReason::Invalid {
                    detail: "format and dataset kinds differ".into(),
                },
            ));
        }
        self.metadata = DatasetMetadata {
            identity,
            selection: Some(ReadSelection { format, path }),
            warnings,
            accepted_archive: false,
        };
        Ok(self)
    }
    pub(crate) fn derived_processed_with_secondary(
        &self,
        value: ProcessedDataset,
        other: &Self,
    ) -> Self {
        let mut output = self.derived_processed(value);
        output.metadata.warnings.extend_from_slice(other.warnings());
        output
    }
    pub(crate) fn derived_processed(&self, value: ProcessedDataset) -> Self {
        let mut output = Self::from_processed(value);
        output.metadata = self.metadata.clone();
        output
    }
    /// Returns the actual current representation kind.
    pub fn kind(&self) -> DatasetKind {
        match self.data {
            DatasetData::Raw(_) => DatasetKind::Raw,
            DatasetData::Processed(_) => DatasetKind::Processed,
        }
    }
    /// Returns the appropriate borrowed scientific description.
    pub fn descriptor(&self) -> DescriptorRef<'_> {
        match &self.data {
            DatasetData::Raw(value) => DescriptorRef::Raw(value.descriptor()),
            DatasetData::Processed(value) => DescriptorRef::Processed(value.descriptor()),
        }
    }
    /// Returns retained portable reading context.
    pub fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }
    /// Returns the appropriate borrowed provenance.
    pub fn provenance(&self) -> ProvenanceRef<'_> {
        match &self.data {
            DatasetData::Raw(value) => ProvenanceRef::Raw(value.provenance()),
            DatasetData::Processed(value) => ProvenanceRef::Processed(value.provenance()),
        }
    }
    /// Returns cached immutable canonical identities without scanning samples.
    pub fn canonical_digests(&self) -> CanonicalDatasetDigests {
        self.digests
    }
    /// Borrows the raw dataset when present.
    pub fn as_raw(&self) -> Option<&RawDataset> {
        match &self.data {
            DatasetData::Raw(value) => Some(value),
            _ => None,
        }
    }
    /// Borrows the processed dataset when present.
    pub fn as_processed(&self) -> Option<&ProcessedDataset> {
        match &self.data {
            DatasetData::Processed(value) => Some(value),
            _ => None,
        }
    }
    /// Borrows dense processed storage when present.
    pub fn as_dense_processed(&self) -> Option<&ProcessedData> {
        self.as_processed().map(ProcessedDataset::data)
    }
    /// Narrows the representation without copying samples or losing context.
    #[allow(clippy::result_large_err)]
    pub fn into_raw(self) -> Result<RawDatasetWithContext, Self> {
        if self.as_raw().is_some() {
            Ok(RawDatasetWithContext(self))
        } else {
            Err(self)
        }
    }
    /// Narrows the representation without copying samples or losing context.
    #[allow(clippy::result_large_err)]
    pub fn into_processed(self) -> Result<ProcessedDatasetWithContext, Self> {
        if self.as_processed().is_some() {
            Ok(ProcessedDatasetWithContext(self))
        } else {
            Err(self)
        }
    }
    /// Extracts raw data, explicitly discarding aggregate metadata on success.
    #[allow(clippy::result_large_err)]
    pub fn into_raw_data(self) -> Result<RawDataset, Self> {
        match self.data {
            DatasetData::Raw(value) => Ok(value),
            _ => Err(self),
        }
    }
    /// Extracts processed data, explicitly discarding aggregate metadata on success.
    #[allow(clippy::result_large_err)]
    pub fn into_processed_data(self) -> Result<ProcessedDataset, Self> {
        match self.data {
            DatasetData::Processed(value) => Ok(value),
            _ => Err(self),
        }
    }
    /// Returns the original selected format, if a reader selection exists.
    pub fn source_format(&self) -> Option<Format> {
        self.metadata.selection.as_ref().map(ReadSelection::format)
    }
    /// Returns portable identity evidence.
    pub fn identity(&self) -> &DatasetIdentity {
        self.metadata.identity()
    }
    /// Returns the original selected path, if a reader selection exists.
    pub fn selected_path(&self) -> Option<&Path> {
        self.metadata.selection.as_ref().map(ReadSelection::path)
    }
    /// Returns import warnings.
    pub fn warnings(&self) -> &[ReadWarning] {
        self.metadata.warnings()
    }
    /// Returns the current representation's ordered source records.
    pub fn sources(&self) -> &[crate::provenance::SourceFile] {
        match &self.data {
            DatasetData::Raw(value) => value.provenance().sources(),
            DatasetData::Processed(value) => value.provenance().sources(),
        }
    }
}

/// Owned raw representation retaining the complete aggregate context.
///
/// Choose [`Self::raw`] for raw-only access or [`Self::dataset`] to retain context
/// through processing. Implicit copying or coercion to a bare value is forbidden.
///
/// ```compile_fail,E0599
/// fn copy(value: nmr::RawDatasetWithContext) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0308
/// fn raw_only(_: &nmr::raw::RawDataset) {}
/// fn implicit(value: nmr::RawDatasetWithContext) { raw_only(&value); }
/// ```
#[derive(Debug, PartialEq)]
pub struct RawDatasetWithContext(Dataset);

impl RawDatasetWithContext {
    /// Borrows the checked raw data. No mutable replacement is exposed.
    pub fn raw(&self) -> &RawDataset {
        self.0.as_raw().expect("narrowed raw")
    }
    /// Borrows the aggregate, including metadata and cached identity.
    pub fn dataset(&self) -> &Dataset {
        &self.0
    }
    /// Restores the aggregate without allocation or scanning samples.
    pub fn into_dataset(self) -> Dataset {
        self.0
    }
}

/// Owned processed representation retaining the complete aggregate context.
///
/// Choose [`Self::processed`] for bare scientific access or [`Self::dataset`] to
/// retain context through processing. Neither implicit deep cloning nor coercion
/// can discard the aggregate's reading and archive-acceptance metadata.
///
/// ```compile_fail,E0599
/// fn copy(value: nmr::ProcessedDatasetWithContext) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0308
/// fn processed_only(_: &nmr::processed::ProcessedDataset) {}
/// fn implicit(value: nmr::ProcessedDatasetWithContext) { processed_only(&value); }
/// ```
#[derive(Debug, PartialEq)]
pub struct ProcessedDatasetWithContext(Dataset);

impl ProcessedDatasetWithContext {
    /// Borrows checked processed data. No mutable replacement is exposed.
    pub fn processed(&self) -> &ProcessedDataset {
        self.0.as_processed().expect("narrowed processed")
    }
    /// Borrows the aggregate, including metadata and cached identity.
    pub fn dataset(&self) -> &Dataset {
        &self.0
    }
    /// Restores the aggregate without allocation or scanning samples.
    pub fn into_dataset(self) -> Dataset {
        self.0
    }
}

impl From<RawDataset> for Dataset {
    fn from(value: RawDataset) -> Self {
        Self::from_raw(value)
    }
}
impl From<ProcessedDataset> for Dataset {
    fn from(value: ProcessedDataset) -> Self {
        Self::from_processed(value)
    }
}

impl Dataset {
    pub(crate) fn accept_archived_history(&mut self) {
        self.metadata.accepted_archive = true;
        if let DatasetData::Processed(value) = &mut self.data {
            value.accept_archived_history();
        }
    }
    pub(crate) fn validate_recorded(
        &self,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError;
        control.check_cancelled()?;
        let actual = match &self.data {
            DatasetData::Raw(value) => {
                value
                    .validate()
                    .map_err(|e| ModelError::Validation(e.to_string()))?;
                crate::canonical_digest::dataset_digests_controlled(
                    value,
                    Some(control.cancellation()),
                )?
            }
            DatasetData::Processed(value) => {
                value.validate_recorded(control)?;
                value.canonical_digests()
            }
        };
        control.check_cancelled()?;
        if actual != self.digests {
            return Err(ModelError::DigestMismatch);
        }
        Ok(())
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl Dataset {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&DatasetData, &DatasetMetadata, &CanonicalDatasetDigests) {
        (&self.data, &self.metadata, &self.digests)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (DatasetData, DatasetMetadata, CanonicalDatasetDigests),
    ) -> Result<Self, crate::internal::ModelError> {
        let (data, metadata, digests) = parts;
        let value = Self {
            data,
            metadata,
            digests,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl DatasetMetadata {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &DatasetIdentity,
        &Option<ReadSelection>,
        &Vec<ReadWarning>,
        &bool,
    ) {
        (
            &self.identity,
            &self.selection,
            &self.warnings,
            &self.accepted_archive,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            DatasetIdentity,
            Option<ReadSelection>,
            Vec<ReadWarning>,
            bool,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (identity, selection, warnings, accepted_archive) = parts;
        let value = Self {
            identity,
            selection,
            warnings,
            accepted_archive,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ReadSelection {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Format, &PathBuf) {
        (&self.format, &self.path)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Format, PathBuf),
    ) -> Result<Self, crate::internal::ModelError> {
        let (format, path) = parts;
        let value = Self { format, path };

        Ok(value)
    }
}
