//! Read failures with format, source and resource classification.

mod source;
pub use source::{InputSource, ReadCandidate};
mod parameter;
pub use parameter::{ParameterError, ParameterErrorKind};
mod classification;
pub use classification::{ReadErrorKind, ReadErrorReason, ReadResource, UnsupportedFeatureCode};
mod error;
pub use error::ReadError;

#[cfg(test)]
mod tests;

mod sampling;
