//! Preserve reading classifications for shared sampling-declaration validation.

use super::{InputSource, ReadError, UnsupportedFeatureCode};
use crate::raw::model::sampling_declaration::SamplingDeclarationError;

impl ReadError {
    pub(crate) fn sampling_declaration(message: &str) -> Self {
        Self::unsupported_code(
            InputSource::memory("sampling_declaration"),
            UnsupportedFeatureCode::SAMPLING_LAYOUT,
            message,
        )
    }
}

impl From<SamplingDeclarationError> for ReadError {
    fn from(error: SamplingDeclarationError) -> Self {
        match error {
            SamplingDeclarationError::Mismatch => Self::sampling_declaration(
                "sampling declaration conflicts with vendor grid, lane count or observation count",
            ),
            SamplingDeclarationError::IndexOutOfBounds => Self::sampling_declaration(
                "sampling declaration index is outside the declared grid",
            ),
            SamplingDeclarationError::ScheduleConflict => Self::sampling_declaration(
                "sampling declaration disagrees with vendor observation order or coordinates",
            ),
            SamplingDeclarationError::Model(error) => error.into(),
            SamplingDeclarationError::Execution(error) => error.into(),
        }
    }
}
