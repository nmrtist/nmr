#[allow(unused_imports)]
use crate::formats::jeol::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for EmbeddedAxisEvidence {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "jeol.mod.EmbeddedAxisEvidence.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                    (*self.model_parts().5).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "jeol.mod.EmbeddedAxisEvidence.v1", 6)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
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
    impl Codec for EmbeddedAxisKind {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::List => record("jeol.mod.EmbeddedAxisKind.v1.List", vec![], budget),
                Self::Ramp => record("jeol.mod.EmbeddedAxisKind.v1.Ramp", vec![], budget),
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
                "jeol.mod.EmbeddedAxisKind.v1.List" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::List)
                }
                "jeol.mod.EmbeddedAxisKind.v1.Ramp" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Ramp)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for EmbeddedRecordArea {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::BeforeData => {
                    record("jeol.mod.EmbeddedRecordArea.v1.BeforeData", vec![], budget)
                }
                Self::AfterData => {
                    record("jeol.mod.EmbeddedRecordArea.v1.AfterData", vec![], budget)
                }
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
                "jeol.mod.EmbeddedRecordArea.v1.BeforeData" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::BeforeData)
                }
                "jeol.mod.EmbeddedRecordArea.v1.AfterData" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::AfterData)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ParameterRecord {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "jeol.mod.ParameterRecord.v1",
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
            let mut f = fields(node, "jeol.mod.ParameterRecord.v1", 5)?;
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
    impl Codec for ParameterValue {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::String(v0) => record(
                    "jeol.mod.ParameterValue.v1.String",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Integer(v0) => record(
                    "jeol.mod.ParameterValue.v1.Integer",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Float(v0) => record(
                    "jeol.mod.ParameterValue.v1.Float",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Complex(v0) => record(
                    "jeol.mod.ParameterValue.v1.Complex",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Infinity(v0) => record(
                    "jeol.mod.ParameterValue.v1.Infinity",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Unknown { type_code, bytes } => record(
                    "jeol.mod.ParameterValue.v1.Unknown",
                    vec![type_code.to_wire(budget)?, bytes.to_wire(budget)?],
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
                "jeol.mod.ParameterValue.v1.String" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::String(next(&mut f, budget)?))
                }
                "jeol.mod.ParameterValue.v1.Integer" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Integer(next(&mut f, budget)?))
                }
                "jeol.mod.ParameterValue.v1.Float" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Float(next(&mut f, budget)?))
                }
                "jeol.mod.ParameterValue.v1.Complex" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Complex(next(&mut f, budget)?))
                }
                "jeol.mod.ParameterValue.v1.Infinity" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Infinity(next(&mut f, budget)?))
                }
                "jeol.mod.ParameterValue.v1.Unknown" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Unknown {
                        type_code: next(&mut f, budget)?,
                        bytes: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for Parameters {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "jeol.mod.Parameters.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                    (*self.model_parts().5).to_wire(budget)?,
                    (*self.model_parts().6).to_wire(budget)?,
                    (*self.model_parts().7).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "jeol.mod.Parameters.v1", 8)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
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
    impl Codec for RawAxisUnit {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "jeol.mod.RawAxisUnit.v1",
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
            let mut f = fields(node, "jeol.mod.RawAxisUnit.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SampleTransform {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "jeol.mod.SampleTransform.v1",
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
            let mut f = fields(node, "jeol.mod.SampleTransform.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
};
