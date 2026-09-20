//! Versioned processing profile values without numerical implementations.

/// Versioned normalized ACME-inspired objective and bounded search profile.
/// Experimental; versioning identifies the rule, not a validated quality guarantee.
/// This ordinary public API follows the crate's
/// [compatibility policy](crate#experimental-algorithms).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NormalizedAcmeV1;

/// Fixed positive-peak asymmetric least-squares profile.
/// Experimental; positive-peak assumptions require assessment on the target data.
/// This ordinary public API follows the crate's
/// [compatibility policy](crate#experimental-algorithms).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PositivePeaksV1;
