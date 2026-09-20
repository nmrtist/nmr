//! Phase-polarity authorization shared by processing operations.

/// Polarity requested for phase-sensitive processing.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExpectedPolarity {
    /// Positive and negative peaks are both expected; 180-degree polarity is unknown.
    #[default]
    Signed,
    /// The caller explicitly asserts that expected peaks are positive.
    Positive,
}

/// Current authorization for phase-sensitive scalar projection.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PolarityState {
    /// The spectrum may be globally inverted by 180 degrees.
    #[default]
    Ambiguous180,
    /// The public caller explicitly asserts positive expected peaks.
    UserAssertedPositive,
}

impl PolarityState {
    /// Resolves a public caller's requested polarity.
    pub fn from_request(expected: ExpectedPolarity) -> Self {
        match expected {
            ExpectedPolarity::Signed => Self::Ambiguous180,
            ExpectedPolarity::Positive => Self::UserAssertedPositive,
        }
    }

    /// Returns whether positive polarity is established.
    pub fn is_established(self) -> bool {
        matches!(self, Self::UserAssertedPositive)
    }
}
