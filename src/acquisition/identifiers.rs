use std::sync::Arc;

use super::EvidenceValidationError;

/// A zero-based axis position in slowest-to-fastest descriptor order.
///
/// This value is not bound to a dataset and does not follow an axis through
/// reordering. Source lineage uses [`crate::provenance::InputAxisRef`] instead.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct AxisIndex(usize);

impl AxisIndex {
    /// Creates an axis identifier from its zero-based descriptor position.
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the zero-based descriptor position.
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Stable identifier for a versioned format-resolution rule.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct RuleId(&'static str);

impl RuleId {
    pub(crate) const fn registered(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the registered identifier.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Stable identifier supplied with a complete caller assertion.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct AssertionId(Arc<str>);

impl AssertionId {
    /// Creates a non-empty caller assertion identifier.
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, EvidenceValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EvidenceValidationError::EmptyIdentifier);
        }
        Ok(Self(value))
    }

    /// Returns the identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identifier of a versioned normalization derivation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct DerivationStepId(&'static str);

impl DerivationStepId {
    pub(crate) const fn registered(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the registered identifier.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl AssertionId {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Arc<str>,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Arc<str>,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        if value.0.trim().is_empty() {
            return Err(crate::internal::ModelError::Structure);
        }

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl AxisIndex {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&usize,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(parts: (usize,)) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl DerivationStepId {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&&'static str,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (&'static str,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RuleId {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&&'static str,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (&'static str,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}
