//! References into ordered external-input collections.

/// An ordinal in one provenance record's ordered external-input collection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputSlot(usize);

impl InputSlot {
    /// Creates a reference; the owning provenance validates its range.
    pub const fn new(index: usize) -> Self {
        Self(index)
    }
    /// Returns the zero-based input ordinal.
    pub const fn index(self) -> usize {
        self.0
    }
}

/// An axis reference scoped to one ordered external-input collection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InputAxisRef {
    input: InputSlot,
    axis: usize,
}

impl InputAxisRef {
    /// Creates a reference; the owning provenance validates slot and axis bounds.
    pub const fn new(input: InputSlot, axis: usize) -> Self {
        Self { input, axis }
    }
    /// Returns the input slot in the owning record.
    pub const fn input(self) -> InputSlot {
        self.input
    }
    /// Returns the source axis within that input.
    pub const fn axis(self) -> usize {
        self.axis
    }
    pub(crate) fn single(axis: usize) -> Self {
        Self::new(InputSlot::new(0), axis)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl InputAxisRef {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&InputSlot, &usize) {
        (&self.input, &self.axis)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (InputSlot, usize),
    ) -> Result<Self, crate::internal::ModelError> {
        let (input, axis) = parts;
        let value = Self { input, axis };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl InputSlot {
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
