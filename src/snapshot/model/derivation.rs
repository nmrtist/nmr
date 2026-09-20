#[allow(unused_imports)]
use crate::derivation::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for DerivationInput {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Raw { snapshot, metadata } => record(
                    "derivation.DerivationInput.v1.Raw",
                    vec![snapshot.to_wire(budget)?, metadata.to_wire(budget)?],
                    budget,
                ),
                Self::Processed(v0) => record(
                    "derivation.DerivationInput.v1.Processed",
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
                "derivation.DerivationInput.v1.Raw" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Raw {
                        snapshot: next(&mut f, budget)?,
                        metadata: next(&mut f, budget)?,
                    })
                }
                "derivation.DerivationInput.v1.Processed" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Processed(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for DerivationOperation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::LinearCombination { scale } => record(
                    "derivation.DerivationOperation.v1.LinearCombination",
                    vec![scale.to_wire(budget)?],
                    budget,
                ),
                Self::NusReconstruction {
                    direct_operations,
                    direct_resolved,
                    settings,
                    noise_report,
                    iterations,
                    column_thresholds,
                    column_relative_changes,
                } => record(
                    "derivation.DerivationOperation.v1.NusReconstruction",
                    vec![
                        direct_operations.to_wire(budget)?,
                        direct_resolved.to_wire(budget)?,
                        settings.to_wire(budget)?,
                        noise_report.to_wire(budget)?,
                        iterations.to_wire(budget)?,
                        column_thresholds.to_wire(budget)?,
                        column_relative_changes.to_wire(budget)?,
                    ],
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
                "derivation.DerivationOperation.v1.LinearCombination" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::LinearCombination {
                        scale: next(&mut f, budget)?,
                    })
                }
                "derivation.DerivationOperation.v1.NusReconstruction" if values.len() == 7 => {
                    let mut f = values.into_iter();
                    Ok(Self::NusReconstruction {
                        direct_operations: next(&mut f, budget)?,
                        direct_resolved: next(&mut f, budget)?,
                        settings: next(&mut f, budget)?,
                        noise_report: next(&mut f, budget)?,
                        iterations: next(&mut f, budget)?,
                        column_thresholds: next(&mut f, budget)?,
                        column_relative_changes: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for LibraryDerivation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "derivation.LibraryDerivation.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                    (*self.model_parts().5).to_wire(budget)?,
                    (*self.model_parts().6).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "derivation.LibraryDerivation.v1", 7)?;
            Self::from_model_parts((
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
};
