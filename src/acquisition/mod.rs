//! Canonical, vendor-neutral acquisition semantics.
//!
//! Values in this module are the evidence-bearing boundary between format
//! resolvers and processing. Processing code never needs to inspect vendor
//! metadata in order to interpret component lanes.

mod identifiers;
pub use identifiers::{AssertionId, AxisIndex, DerivationStepId, RuleId};
mod evidence;
pub use evidence::{
    ComponentEvidence, EvidenceValidationError, NormalizationEvidence, NormalizationFact,
    ResolutionAuthority, TrustClass,
};
mod components;
pub(crate) use components::ResolvedTransformInner;
pub use components::{
    LinearComponentTransform, ModulationIndexDomain, PeriodicLaneModulation,
    ResolvedComponentTransform, TransformValidationError,
};
mod axis;
pub use axis::{ComponentBasis, DirectSamples, IndirectComponents, RawAxisKind};
mod reference;
pub use reference::{ChemicalShiftReference, GroupDelayState, PendingGroupDelay};

pub(crate) mod registry;

#[cfg(test)]
mod tests;
