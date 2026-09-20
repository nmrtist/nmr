#[allow(unused_imports)]
use crate::processed::model::origin::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for DerivedRawOrigin {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.core.DerivedRawOrigin.v1",
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
            let mut f = fields(node, "processed.core.DerivedRawOrigin.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessedOrigin {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Library(v0) => record(
                    "processed.core.ProcessedOrigin.v1.Library",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::External(v0) => record(
                    "processed.core.ProcessedOrigin.v1.External",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Imported => {
                    record("processed.core.ProcessedOrigin.v1.Imported", vec![], budget)
                }
                Self::DeclaredRaw {
                    snapshot,
                    axis_lineage,
                } => record(
                    "processed.core.ProcessedOrigin.v1.DeclaredRaw",
                    vec![snapshot.to_wire(budget)?, axis_lineage.to_wire(budget)?],
                    budget,
                ),
                Self::DerivedRaw(v0) => record(
                    "processed.core.ProcessedOrigin.v1.DerivedRaw",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Unknown => {
                    record("processed.core.ProcessedOrigin.v1.Unknown", vec![], budget)
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
                "processed.core.ProcessedOrigin.v1.Library" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Library(next(&mut f, budget)?))
                }
                "processed.core.ProcessedOrigin.v1.External" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::External(next(&mut f, budget)?))
                }
                "processed.core.ProcessedOrigin.v1.Imported" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Imported)
                }
                "processed.core.ProcessedOrigin.v1.DeclaredRaw" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::DeclaredRaw {
                        snapshot: next(&mut f, budget)?,
                        axis_lineage: next(&mut f, budget)?,
                    })
                }
                "processed.core.ProcessedOrigin.v1.DerivedRaw" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::DerivedRaw(next(&mut f, budget)?))
                }
                "processed.core.ProcessedOrigin.v1.Unknown" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Unknown)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RawDatasetSnapshot {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.core.RawDatasetSnapshot.v1",
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
            let mut f = fields(node, "processed.core.RawDatasetSnapshot.v1", 6)?;
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
};
