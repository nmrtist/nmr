use std::num::NonZeroUsize;
use std::sync::Arc;
use thiserror::Error;

use super::{AssertionId, DerivationStepId, ModulationIndexDomain, RuleId};

use super::registry;

/// Authority that established normalized acquisition semantics.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolutionAuthority {
    /// A checked format rule established the value.
    FormatRule(RuleId),
    /// A complete, non-conflicting caller assertion established the value.
    CallerAssertion(AssertionId),
    /// A public constructor created the value without source-format authority.
    UserConstructed,
}

/// Trust classification derived solely from [`ResolutionAuthority`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustClass {
    /// Established by a versioned parser rule.
    FormatResolved,
    /// Established by an explicit caller assertion.
    CallerAsserted,
    /// Constructed independently of an input source.
    UserProvided,
}

impl ResolutionAuthority {
    /// Returns the derived trust class.
    pub const fn trust_class(&self) -> TrustClass {
        match self {
            Self::FormatRule(_) => TrustClass::FormatResolved,
            Self::CallerAssertion(_) => TrustClass::CallerAsserted,
            Self::UserConstructed => TrustClass::UserProvided,
        }
    }
}

/// Typed fact used by an acquisition-semantic derivation.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NormalizationFact {
    /// Component lanes have a checked canonical order.
    CanonicalLaneOrder {
        /// Number of lanes covered by the fact.
        lanes: NonZeroUsize,
    },
    /// Complex samples use the public `R + iI` convention.
    PublicComplexConvention,
    /// Physical traces map bijectively to logical coordinates and lane tuples.
    TraceMappingBijection,
    /// Coefficients were obtained from a typed source field.
    SourceCoefficients {
        /// Whether that source field was marked active.
        active: bool,
        /// Number of real source coefficients.
        values: usize,
    },
    /// A periodic modulation has a checked index domain.
    PeriodicModulation {
        /// Period of the modulation.
        period: NonZeroUsize,
        /// Domain used to select the modulation phase.
        domain: ModulationIndexDomain,
    },
    /// Direct real/complex storage was established from source structure.
    DirectSampleEncoding,
    /// The caller supplied a complete component transform.
    UserDefinedComponentTransform,
    /// Cartesian component normalization was supplied by the caller.
    UserDefinedCartesianNormalization,
    /// A digital-filter delay was established from typed acquisition fields.
    DigitalFilterDelay,
    /// A chemical-shift reference was supplied explicitly.
    ChemicalShiftReference,
}

/// Evidence supporting one normalized acquisition statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizationEvidence {
    authority: ResolutionAuthority,
    facts: Arc<[NormalizationFact]>,
    derivation: Arc<[DerivationStepId]>,
}

impl NormalizationEvidence {
    pub(crate) fn from_format(
        rule: RuleId,
        facts: Vec<NormalizationFact>,
        derivation: Vec<DerivationStepId>,
    ) -> Result<Self, EvidenceValidationError> {
        Self::try_new(ResolutionAuthority::FormatRule(rule), facts, derivation)
    }

    pub(crate) fn from_assertion(
        assertion: AssertionId,
        facts: Vec<NormalizationFact>,
        derivation: Vec<DerivationStepId>,
    ) -> Result<Self, EvidenceValidationError> {
        Self::try_new(
            ResolutionAuthority::CallerAssertion(assertion),
            facts,
            derivation,
        )
    }

    /// Creates explicitly user-authored normalization evidence.
    pub fn user_constructed(
        facts: Vec<NormalizationFact>,
    ) -> Result<Self, EvidenceValidationError> {
        Self::try_new(
            ResolutionAuthority::UserConstructed,
            facts,
            vec![registry::USER_CONSTRUCTION_V1],
        )
    }

    fn try_new(
        authority: ResolutionAuthority,
        facts: Vec<NormalizationFact>,
        derivation: Vec<DerivationStepId>,
    ) -> Result<Self, EvidenceValidationError> {
        if facts.is_empty() {
            return Err(EvidenceValidationError::MissingFact);
        }
        if derivation.is_empty() {
            return Err(EvidenceValidationError::MissingDerivation);
        }
        Ok(Self {
            authority,
            facts: facts.into(),
            derivation: derivation.into(),
        })
    }

    /// Returns the authority that established this evidence.
    pub fn authority(&self) -> &ResolutionAuthority {
        &self.authority
    }

    /// Returns the trust class derived from the authority.
    pub fn trust_class(&self) -> TrustClass {
        self.authority.trust_class()
    }

    /// Returns typed facts without interpretation of textual identifiers.
    pub fn facts(&self) -> &[NormalizationFact] {
        &self.facts
    }

    /// Returns the ordered, versioned derivation steps.
    pub fn derivation(&self) -> &[DerivationStepId] {
        &self.derivation
    }
}

/// Evidence that two raw lanes are already canonical Cartesian components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentEvidence(NormalizationEvidence);

impl ComponentEvidence {
    /// Creates user-authored Cartesian normalization evidence.
    pub fn user_constructed() -> Self {
        Self(
            NormalizationEvidence::user_constructed(vec![
                NormalizationFact::CanonicalLaneOrder {
                    lanes: NonZeroUsize::new(2).expect("two is nonzero"),
                },
                NormalizationFact::PublicComplexConvention,
                NormalizationFact::UserDefinedCartesianNormalization,
            ])
            .expect("built-in evidence is valid"),
        )
    }

    pub(crate) fn resolved(evidence: NormalizationEvidence) -> Self {
        Self(evidence)
    }

    /// Returns the supporting normalization evidence.
    pub fn evidence(&self) -> &NormalizationEvidence {
        &self.0
    }
}

/// Invalid normalization evidence.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EvidenceValidationError {
    /// A caller assertion identifier is empty.
    #[error("evidence identifier must not be empty")]
    EmptyIdentifier,
    /// Evidence contains no typed facts.
    #[error("normalization evidence requires at least one typed fact")]
    MissingFact,
    /// Evidence contains no versioned derivation.
    #[error("normalization evidence requires a versioned derivation")]
    MissingDerivation,
    /// Chemical-shift reference values are non-finite or non-positive.
    #[error("chemical-shift reference is invalid")]
    InvalidChemicalShiftReference,
    /// A requested frequency offset is non-finite.
    #[error("frequency offset is not finite")]
    InvalidFrequencyOffset,
    /// A finite frequency offset produced a non-finite chemical shift.
    #[error("chemical-shift conversion result is not finite")]
    InvalidChemicalShiftResult,
    /// A group delay is non-finite or negative.
    #[error("group delay must be finite and non-negative")]
    InvalidGroupDelay,
    /// A complete physical-to-canonical trace permutation is empty or not bijective.
    #[error("trace permutation must be a nonempty bijection over 0..len")]
    InvalidTracePermutation,
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ComponentEvidence {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&NormalizationEvidence,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (NormalizationEvidence,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl NormalizationEvidence {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &ResolutionAuthority,
        &Arc<[NormalizationFact]>,
        &Arc<[DerivationStepId]>,
    ) {
        (&self.authority, &self.facts, &self.derivation)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            ResolutionAuthority,
            Arc<[NormalizationFact]>,
            Arc<[DerivationStepId]>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (authority, facts, derivation) = parts;
        let value = Self {
            authority,
            facts,
            derivation,
        };

        if value.facts.is_empty() || value.derivation.is_empty() {
            return Err(crate::internal::ModelError::Structure);
        }

        Ok(value)
    }
}
