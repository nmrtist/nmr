#[allow(unused_imports)]
use crate::acquisition::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for AssertionId {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.AssertionId.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.AssertionId.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for AxisIndex {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.AxisIndex.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.AxisIndex.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ChemicalShiftReference {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.ChemicalShiftReference.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.ChemicalShiftReference.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ComponentBasis {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Scalar => record("acquisition.ComponentBasis.v1.Scalar", vec![], budget),
                Self::SharedComplex { axis, conjugated } => record(
                    "acquisition.ComponentBasis.v1.SharedComplex",
                    vec![axis.to_wire(budget)?, conjugated.to_wire(budget)?],
                    budget,
                ),
                Self::Cartesian => {
                    record("acquisition.ComponentBasis.v1.Cartesian", vec![], budget)
                }
                Self::Encoded(v0) => record(
                    "acquisition.ComponentBasis.v1.Encoded",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.ComponentBasis.v1.Scalar" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Scalar)
                }
                "acquisition.ComponentBasis.v1.SharedComplex" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::SharedComplex {
                        axis: next(&mut f, budget)?,
                        conjugated: next(&mut f, budget)?,
                    })
                }
                "acquisition.ComponentBasis.v1.Cartesian" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Cartesian)
                }
                "acquisition.ComponentBasis.v1.Encoded" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Encoded(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ComponentEvidence {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.ComponentEvidence.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.ComponentEvidence.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for DerivationStepId {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.DerivationStepId.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.DerivationStepId.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for DirectSamples {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Real => record("acquisition.DirectSamples.v1.Real", vec![], budget),
                Self::Complex => record("acquisition.DirectSamples.v1.Complex", vec![], budget),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.DirectSamples.v1.Real" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Real)
                }
                "acquisition.DirectSamples.v1.Complex" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Complex)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for GroupDelayState {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::NotApplicable => record(
                    "acquisition.GroupDelayState.v1.NotApplicable",
                    vec![],
                    budget,
                ),
                Self::Unknown => record("acquisition.GroupDelayState.v1.Unknown", vec![], budget),
                Self::Pending(v0) => record(
                    "acquisition.GroupDelayState.v1.Pending",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.GroupDelayState.v1.NotApplicable" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::NotApplicable)
                }
                "acquisition.GroupDelayState.v1.Unknown" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Unknown)
                }
                "acquisition.GroupDelayState.v1.Pending" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Pending(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for IndirectComponents {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Scalar => record("acquisition.IndirectComponents.v1.Scalar", vec![], budget),
                Self::SharedComplex {
                    conjugated,
                    evidence,
                } => record(
                    "acquisition.IndirectComponents.v1.SharedComplex",
                    vec![conjugated.to_wire(budget)?, evidence.to_wire(budget)?],
                    budget,
                ),
                Self::Cartesian(v0) => record(
                    "acquisition.IndirectComponents.v1.Cartesian",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Encoded(v0) => record(
                    "acquisition.IndirectComponents.v1.Encoded",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.IndirectComponents.v1.Scalar" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Scalar)
                }
                "acquisition.IndirectComponents.v1.SharedComplex" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::SharedComplex {
                        conjugated: next(&mut f, budget)?,
                        evidence: next(&mut f, budget)?,
                    })
                }
                "acquisition.IndirectComponents.v1.Cartesian" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Cartesian(next(&mut f, budget)?))
                }
                "acquisition.IndirectComponents.v1.Encoded" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Encoded(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for LinearComponentTransform {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.LinearComponentTransform.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.LinearComponentTransform.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ModulationIndexDomain {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::AbsoluteGridCoordinate(v0) => record(
                    "acquisition.ModulationIndexDomain.v1.AbsoluteGridCoordinate",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::ObservationOrdinal => record(
                    "acquisition.ModulationIndexDomain.v1.ObservationOrdinal",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.ModulationIndexDomain.v1.AbsoluteGridCoordinate"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::AbsoluteGridCoordinate(next(&mut f, budget)?))
                }
                "acquisition.ModulationIndexDomain.v1.ObservationOrdinal" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::ObservationOrdinal)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for NormalizationEvidence {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.NormalizationEvidence.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.NormalizationEvidence.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for NormalizationFact {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::CanonicalLaneOrder { lanes } => record(
                    "acquisition.NormalizationFact.v1.CanonicalLaneOrder",
                    vec![lanes.to_wire(budget)?],
                    budget,
                ),
                Self::PublicComplexConvention => record(
                    "acquisition.NormalizationFact.v1.PublicComplexConvention",
                    vec![],
                    budget,
                ),
                Self::TraceMappingBijection => record(
                    "acquisition.NormalizationFact.v1.TraceMappingBijection",
                    vec![],
                    budget,
                ),
                Self::SourceCoefficients { active, values } => record(
                    "acquisition.NormalizationFact.v1.SourceCoefficients",
                    vec![active.to_wire(budget)?, values.to_wire(budget)?],
                    budget,
                ),
                Self::PeriodicModulation { period, domain } => record(
                    "acquisition.NormalizationFact.v1.PeriodicModulation",
                    vec![period.to_wire(budget)?, domain.to_wire(budget)?],
                    budget,
                ),
                Self::DirectSampleEncoding => record(
                    "acquisition.NormalizationFact.v1.DirectSampleEncoding",
                    vec![],
                    budget,
                ),
                Self::UserDefinedComponentTransform => record(
                    "acquisition.NormalizationFact.v1.UserDefinedComponentTransform",
                    vec![],
                    budget,
                ),
                Self::UserDefinedCartesianNormalization => record(
                    "acquisition.NormalizationFact.v1.UserDefinedCartesianNormalization",
                    vec![],
                    budget,
                ),
                Self::DigitalFilterDelay => record(
                    "acquisition.NormalizationFact.v1.DigitalFilterDelay",
                    vec![],
                    budget,
                ),
                Self::ChemicalShiftReference => record(
                    "acquisition.NormalizationFact.v1.ChemicalShiftReference",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.NormalizationFact.v1.CanonicalLaneOrder" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::CanonicalLaneOrder {
                        lanes: next(&mut f, budget)?,
                    })
                }
                "acquisition.NormalizationFact.v1.PublicComplexConvention" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::PublicComplexConvention)
                }
                "acquisition.NormalizationFact.v1.TraceMappingBijection" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::TraceMappingBijection)
                }
                "acquisition.NormalizationFact.v1.SourceCoefficients" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::SourceCoefficients {
                        active: next(&mut f, budget)?,
                        values: next(&mut f, budget)?,
                    })
                }
                "acquisition.NormalizationFact.v1.PeriodicModulation" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::PeriodicModulation {
                        period: next(&mut f, budget)?,
                        domain: next(&mut f, budget)?,
                    })
                }
                "acquisition.NormalizationFact.v1.DirectSampleEncoding" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::DirectSampleEncoding)
                }
                "acquisition.NormalizationFact.v1.UserDefinedComponentTransform"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::UserDefinedComponentTransform)
                }
                "acquisition.NormalizationFact.v1.UserDefinedCartesianNormalization"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::UserDefinedCartesianNormalization)
                }
                "acquisition.NormalizationFact.v1.DigitalFilterDelay" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::DigitalFilterDelay)
                }
                "acquisition.NormalizationFact.v1.ChemicalShiftReference" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::ChemicalShiftReference)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for PendingGroupDelay {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.PendingGroupDelay.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.PendingGroupDelay.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for PeriodicLaneModulation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.PeriodicLaneModulation.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.PeriodicLaneModulation.v1", 5)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RawAxisKind {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Direct(v0) => record(
                    "acquisition.RawAxisKind.v1.Direct",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Indirect(v0) => record(
                    "acquisition.RawAxisKind.v1.Indirect",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Parameter => record("acquisition.RawAxisKind.v1.Parameter", vec![], budget),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.RawAxisKind.v1.Direct" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Direct(next(&mut f, budget)?))
                }
                "acquisition.RawAxisKind.v1.Indirect" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Indirect(next(&mut f, budget)?))
                }
                "acquisition.RawAxisKind.v1.Parameter" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Parameter)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ResolutionAuthority {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::FormatRule(v0) => record(
                    "acquisition.ResolutionAuthority.v1.FormatRule",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::CallerAssertion(v0) => record(
                    "acquisition.ResolutionAuthority.v1.CallerAssertion",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::UserConstructed => record(
                    "acquisition.ResolutionAuthority.v1.UserConstructed",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "acquisition.ResolutionAuthority.v1.FormatRule" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::FormatRule(next(&mut f, budget)?))
                }
                "acquisition.ResolutionAuthority.v1.CallerAssertion" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::CallerAssertion(next(&mut f, budget)?))
                }
                "acquisition.ResolutionAuthority.v1.UserConstructed" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::UserConstructed)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ResolvedComponentTransform {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.ResolvedComponentTransform.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.ResolvedComponentTransform.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ResolvedTransformInner {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.ResolvedTransformInner.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.ResolvedTransformInner.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RuleId {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "acquisition.RuleId.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "acquisition.RuleId.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
};
