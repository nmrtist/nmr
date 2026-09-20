//! Source identity, file inspection and complete-source hashing.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use thiserror::Error;

/// Validation failures for source provenance.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProvenanceError {
    /// A source entry has no semantic role.
    #[error("source role is empty")]
    EmptySourceRole,
    /// A source identity or digest could not be read.
    #[error("could not inspect source provenance: {0}")]
    SourceIo(String),
    /// A source changed after its identity was captured.
    #[error("source changed after opening")]
    SourceChanged,
}

/// Broad classification of an input recorded in dataset provenance.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    /// Numeric sample data.
    Data,
    /// Vendor parameters or descriptive headers.
    Parameters,
    /// A non-uniform sampling schedule.
    SamplingSchedule,
    /// Another explicitly identified source role.
    Other,
}

/// A source file and its semantic role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    kind: SourceKind,
    role: String,
    path: PathBuf,
    identity: Option<SourceIdentity>,
    digest: SourceDigest,
    id: Option<SourceId>,
}

/// Dataset-local source identity derived from typed role and stable ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceId {
    kind: SourceKind,
    ordinal: usize,
}

/// Stable file facts captured when a source is opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceIdentity {
    length: u64,
    modified_nanos: Option<u128>,
}

/// SHA-256 state for a source file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDigest {
    /// The source bytes have not all been consumed yet.
    NotComputed,
    /// SHA-256 over the complete source byte stream.
    Sha256([u8; 32]),
}

impl SourceFile {
    pub(crate) fn same_content(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.role == other.role
            && self.id == other.id
            && self.digest == other.digest
    }
    pub(crate) fn from_consumed_bytes(
        kind: SourceKind,
        role: &str,
        path: &Path,
        bytes: &[u8],
    ) -> Self {
        Self {
            kind,
            role: role.to_owned(),
            path: path.to_owned(),
            identity: None,
            digest: SourceDigest::Sha256(Sha256::digest(bytes).into()),
            id: None,
        }
    }

    /// Creates an entry with a non-empty role.
    pub fn new(
        kind: SourceKind,
        role: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Result<Self, ProvenanceError> {
        let role = role.into();
        if role.trim().is_empty() {
            return Err(ProvenanceError::EmptySourceRole);
        }
        let path = path.into();
        let identity = capture_identity(&path)?;
        let digest = if identity.is_some()
            && matches!(kind, SourceKind::Parameters | SourceKind::SamplingSchedule)
        {
            SourceDigest::Sha256(hash_file(&path, identity)?)
        } else {
            SourceDigest::NotComputed
        };
        Ok(Self {
            kind,
            role,
            path,
            identity,
            digest,
            id: None,
        })
    }

    /// Returns the broad source classification.
    pub fn kind(&self) -> SourceKind {
        self.kind
    }

    /// Returns the semantic role, such as `fid` or `procpar`.
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Returns the source path used by the reader.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Optional filesystem locator. In-memory Parts sources have no locator.
    pub fn locator(&self) -> Option<&Path> {
        (!self.path.as_os_str().is_empty()).then_some(self.path.as_path())
    }

    /// Returns file length and modification time captured at open.
    pub fn identity(&self) -> Option<SourceIdentity> {
        self.identity
    }

    /// Returns the current complete-source digest state.
    pub fn digest(&self) -> SourceDigest {
        self.digest
    }

    /// Returns the dataset-local typed source ID after attachment to provenance.
    pub fn id(&self) -> Option<SourceId> {
        self.id
    }

    pub(crate) fn assign_id(&mut self, ordinal: usize) {
        self.id = Some(SourceId {
            kind: self.kind,
            ordinal,
        });
    }

    pub(crate) fn set_consumed_digest(&mut self, digest: SourceDigest) {
        self.digest = digest;
    }

    pub(crate) fn verify_identity(&self) -> Result<(), ProvenanceError> {
        if self.identity.is_some() && capture_identity(&self.path)? != self.identity {
            return Err(ProvenanceError::SourceChanged);
        }
        Ok(())
    }

    pub(crate) fn verify_digest(&self) -> Result<(), ProvenanceError> {
        if self.locator().is_none() {
            return Ok(());
        }
        let SourceDigest::Sha256(expected) = self.digest else {
            return Err(ProvenanceError::SourceChanged);
        };
        let identity = match self.identity {
            Some(identity) => Some(identity),
            None => capture_identity(&self.path)?,
        };
        if hash_file(&self.path, identity)? != expected {
            return Err(ProvenanceError::SourceChanged);
        }
        Ok(())
    }
}

impl SourceId {
    /// Returns the broad typed source role.
    pub const fn kind(self) -> SourceKind {
        self.kind
    }
    /// Returns the stable ordinal within the dataset's ordered source list.
    pub const fn ordinal(self) -> usize {
        self.ordinal
    }
}

impl SourceIdentity {
    /// Returns source length in bytes.
    pub const fn length(self) -> u64 {
        self.length
    }
    /// Returns modification time as nanoseconds since the Unix epoch when available.
    pub const fn modified_nanos(self) -> Option<u128> {
        self.modified_nanos
    }
}

fn capture_identity(path: &Path) -> Result<Option<SourceIdentity>, ProvenanceError> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata =
        fs::metadata(path).map_err(|error| ProvenanceError::SourceIo(error.to_string()))?;
    let modified_nanos = metadata.modified().ok().and_then(|value| {
        value
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_nanos())
    });
    Ok(Some(SourceIdentity {
        length: metadata.len(),
        modified_nanos,
    }))
}

fn hash_file(path: &Path, expected: Option<SourceIdentity>) -> Result<[u8; 32], ProvenanceError> {
    if capture_identity(path)? != expected {
        return Err(ProvenanceError::SourceChanged);
    }
    let mut file =
        fs::File::open(path).map_err(|error| ProvenanceError::SourceIo(error.to_string()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| ProvenanceError::SourceIo(error.to_string()))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    if capture_identity(path)? != expected {
        return Err(ProvenanceError::SourceChanged);
    }
    Ok(hash.finalize().into())
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SourceFile {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &SourceKind,
        &String,
        &PathBuf,
        &Option<SourceIdentity>,
        &SourceDigest,
        &Option<SourceId>,
    ) {
        (
            &self.kind,
            &self.role,
            &self.path,
            &self.identity,
            &self.digest,
            &self.id,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            SourceKind,
            String,
            PathBuf,
            Option<SourceIdentity>,
            SourceDigest,
            Option<SourceId>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (kind, role, path, identity, digest, id) = parts;
        let value = Self {
            kind,
            role,
            path,
            identity,
            digest,
            id,
        };

        if value.role.trim().is_empty() {
            return Err(crate::internal::ModelError::Structure);
        }

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SourceId {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&SourceKind, &usize) {
        (&self.kind, &self.ordinal)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (SourceKind, usize),
    ) -> Result<Self, crate::internal::ModelError> {
        let (kind, ordinal) = parts;
        let value = Self { kind, ordinal };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SourceIdentity {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&u64, &Option<u128>) {
        (&self.length, &self.modified_nanos)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (u64, Option<u128>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (length, modified_nanos) = parts;
        let value = Self {
            length,
            modified_nanos,
        };

        Ok(value)
    }
}
