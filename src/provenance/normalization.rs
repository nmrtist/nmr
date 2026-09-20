//! Recorded sample normalization and frozen rule validation.

use std::sync::Arc;

/// Canonical record of numeric normalization performed while decoding samples.
///
/// Scale factors are ordered by source block. The imaginary multiplier is the
/// factor applied to a stored imaginary value to produce public `R + iI`.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleNormalization {
    algorithm_version: &'static str,
    source_block_scale_factors: Arc<[f64]>,
    stored_imaginary_multiplier: i8,
}

impl SampleNormalization {
    pub(crate) fn reader(
        source_block_scale_factors: Vec<f64>,
        stored_imaginary_multiplier: i8,
    ) -> Self {
        debug_assert!(!source_block_scale_factors.is_empty());
        debug_assert!(
            source_block_scale_factors
                .iter()
                .all(|factor| factor.is_finite() && *factor != 0.0)
        );
        debug_assert!(matches!(stored_imaginary_multiplier, -1 | 1));
        Self {
            algorithm_version: "source-sample-normalization.v1",
            source_block_scale_factors: source_block_scale_factors.into(),
            stored_imaginary_multiplier,
        }
    }

    pub(crate) fn identity() -> Self {
        Self::reader(vec![1.0], 1)
    }

    /// Returns the frozen decoding-normalization algorithm identifier.
    pub const fn algorithm_version(&self) -> &'static str {
        self.algorithm_version
    }

    /// Returns finite nonzero scale factors in source-block order.
    pub fn source_block_scale_factors(&self) -> &[f64] {
        &self.source_block_scale_factors
    }

    /// Returns the sign applied to stored imaginary values.
    pub const fn stored_imaginary_multiplier(&self) -> i8 {
        self.stored_imaginary_multiplier
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SampleNormalization {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&&'static str, &Arc<[f64]>, &i8) {
        (
            &self.algorithm_version,
            &self.source_block_scale_factors,
            &self.stored_imaginary_multiplier,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (&'static str, Arc<[f64]>, i8),
    ) -> Result<Self, crate::internal::ModelError> {
        let (algorithm_version, source_block_scale_factors, stored_imaginary_multiplier) = parts;
        let value = Self {
            algorithm_version,
            source_block_scale_factors,
            stored_imaginary_multiplier,
        };

        if value.algorithm_version != "source-sample-normalization.v1" {
            return Err(crate::internal::ModelError::UnsupportedHistoryVersion(
                value.algorithm_version.into(),
            ));
        }
        if value.source_block_scale_factors.is_empty()
            || value
                .source_block_scale_factors
                .iter()
                .any(|v| !v.is_finite() || *v == 0.0)
            || !matches!(value.stored_imaginary_multiplier, -1 | 1)
        {
            return Err(crate::internal::ModelError::Structure);
        }

        Ok(value)
    }
}
