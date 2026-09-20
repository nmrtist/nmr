//! Internal registry of format rules and derivation algorithms.

use super::{DerivationStepId, RuleId};

pub(crate) const USER_CONSTRUCTION_V1: DerivationStepId =
    DerivationStepId::registered("canonical.user-construction.v1");
pub(crate) const CALLER_ASSERTION_V1: DerivationStepId =
    DerivationStepId::registered("canonical.caller-assertion.v1");
pub(crate) const BRUKER_COMPONENT_V1: DerivationStepId =
    DerivationStepId::registered("bruker.component-transform.v1");
pub(crate) const JEOL_CARTESIAN_V1: DerivationStepId =
    DerivationStepId::registered("jeol.cartesian-sections.v1");
pub(crate) const JEOL_PN_V1: DerivationStepId =
    DerivationStepId::registered("jeol.pn-to-cartesian.v1");
pub(crate) const JEOL_SHARED_V1: DerivationStepId =
    DerivationStepId::registered("jeol.shared-complex.v1");
pub(crate) const VARIAN_EXPLICIT_F1COEF_DERIVATION_V1: DerivationStepId =
    DerivationStepId::registered("openvnmrj.5e20f6f.explicit-f1coef.v1");
pub(crate) const VARIAN_DEFAULT_PTYPE_DERIVATION_V1: DerivationStepId =
    DerivationStepId::registered("openvnmrj.5e20f6f.default-ptype.v1");
pub(crate) const GROUP_DELAY_V1: DerivationStepId =
    DerivationStepId::registered("canonical.group-delay.v1");
pub(crate) const CHEMICAL_SHIFT_V1: DerivationStepId =
    DerivationStepId::registered("canonical.chemical-shift-reference.v1");
pub(crate) const VARIAN_CHEMICAL_SHIFT_V1: DerivationStepId =
    DerivationStepId::registered("openvnmrj.5e20f6f.chemical-shift-reference.v1");
pub(crate) const FORMAT_LAYOUT_V1: DerivationStepId =
    DerivationStepId::registered("canonical.format-layout.v1");

pub(crate) const BRUKER_DIRECT_V1: RuleId = RuleId::registered("bruker.direct.v1");
pub(crate) const BRUKER_DSP_TABLE_V1: RuleId = RuleId::registered("bruker.dspfvs-decim-table.v1");
pub(crate) const BRUKER_LAYOUT_V1: RuleId = RuleId::registered("bruker.layout.v1");
pub(crate) const BRUKER_QF_V1: RuleId = RuleId::registered("bruker.fnmode-qf.v1");
pub(crate) const BRUKER_STATES_V1: RuleId = RuleId::registered("bruker.fnmode-states.v1");
pub(crate) const BRUKER_STATES_TPPI_V1: RuleId = RuleId::registered("bruker.fnmode-states-tppi.v1");
pub(crate) const BRUKER_ECHO_ANTI_ECHO_V1: RuleId =
    RuleId::registered("bruker.fnmode-echo-anti-echo.v1");
pub(crate) const JEOL_DIRECT_V1: RuleId = RuleId::registered("jeol.direct.v1");
pub(crate) const JEOL_CARTESIAN_2D_V1: RuleId = RuleId::registered("jeol.cartesian-2d.v1");
pub(crate) const JEOL_PN_2D_V1: RuleId = RuleId::registered("jeol.pn-y-2d.v1");
pub(crate) const JEOL_SHARED_2D_V1: RuleId = RuleId::registered("jeol.shared-complex-2d.v1");
pub(crate) const VARIAN_DIRECT_V1: RuleId = RuleId::registered("varian.direct.v1");
pub(crate) const VARIAN_DIRECT_ARRAY_V1: RuleId = RuleId::registered("varian.direct-array.v1");
pub(crate) const VARIAN_EXPLICIT_F1COEF_V1: RuleId =
    RuleId::registered("varian.2d-phase-explicit-f1coef.v1");
pub(crate) const VARIAN_DEFAULT_PTYPE_V1: RuleId =
    RuleId::registered("varian.2d-phase-default-ptype.v1");
