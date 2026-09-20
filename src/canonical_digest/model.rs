//! Frozen versioned byte protocol for canonical acquisition identities.

/// A SHA-256 identity over a frozen canonical byte protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalDigest([u8; 32]);

impl CanonicalDigest {
    /// Returns the 32 digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub(crate) const fn zero() -> Self {
        Self([0; 32])
    }
}

/// Strong canonical identities for descriptor, samples, and their binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalDatasetDigests {
    descriptor: CanonicalDigest,
    samples: CanonicalDigest,
    dataset: CanonicalDigest,
}

impl CanonicalDatasetDigests {
    /// Returns the canonical descriptor identity.
    pub const fn descriptor(self) -> CanonicalDigest {
        self.descriptor
    }
    /// Returns the canonical sample identity.
    pub const fn samples(self) -> CanonicalDigest {
        self.samples
    }
    /// Returns the descriptor/sample/schedule binding.
    pub const fn dataset(self) -> CanonicalDigest {
        self.dataset
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl CanonicalDatasetDigests {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&CanonicalDigest, &CanonicalDigest, &CanonicalDigest) {
        (&self.descriptor, &self.samples, &self.dataset)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (CanonicalDigest, CanonicalDigest, CanonicalDigest),
    ) -> Result<Self, crate::internal::ModelError> {
        let (descriptor, samples, dataset) = parts;
        let value = Self {
            descriptor,
            samples,
            dataset,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl CanonicalDigest {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&[u8; 32],) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: ([u8; 32],),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

impl CanonicalDigest {
    pub(super) fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl CanonicalDatasetDigests {
    pub(super) fn new(
        descriptor: CanonicalDigest,
        samples: CanonicalDigest,
        dataset: CanonicalDigest,
    ) -> Self {
        Self {
            descriptor,
            samples,
            dataset,
        }
    }
}
