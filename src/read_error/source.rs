use crate::Format;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

/// A resolver candidate retained in ambiguity and mismatch errors.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReadCandidate {
    format: Format,
    resolved_path: PathBuf,
}

impl ReadCandidate {
    pub(crate) fn new(format: Format, resolved_path: PathBuf) -> Self {
        Self {
            format,
            resolved_path,
        }
    }

    /// Returns the candidate format.
    pub fn format(&self) -> Format {
        self.format
    }

    /// Returns the candidate's resolved primary path.
    pub fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }
}

/// The concrete input associated with a reader failure.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputSource {
    /// A filesystem input.
    Path(PathBuf),
    /// An in-memory input identified by its actual dataset role.
    #[non_exhaustive]
    Memory {
        /// The input's vendor-defined role, such as `fid` or `procpar`.
        role: String,
    },
}

impl InputSource {
    /// Creates an in-memory source identified by its vendor-defined role.
    pub fn memory(role: impl Into<String>) -> Self {
        Self::Memory { role: role.into() }
    }

    /// Returns the filesystem path, or `None` for an in-memory input.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path),
            Self::Memory { .. } => None,
        }
    }

    /// Returns the in-memory role, or `None` for a filesystem input.
    pub fn role(&self) -> Option<&str> {
        match self {
            Self::Path(_) => None,
            Self::Memory { role } => Some(role),
        }
    }
}

impl From<&Path> for InputSource {
    fn from(path: &Path) -> Self {
        Self::Path(path.to_path_buf())
    }
}

impl From<PathBuf> for InputSource {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl fmt::Display for InputSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(path) => write!(formatter, "{}", path.display()),
            Self::Memory { role } => write!(formatter, "in-memory {role}"),
        }
    }
}
