#[allow(unused_imports)]
use crate::dataset::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for Dataset {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "dataset.Dataset.v1",
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
            let mut f = fields(node, "dataset.Dataset.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for DatasetData {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Raw(v0) => record(
                    "dataset.DatasetData.v1.Raw",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Processed(v0) => record(
                    "dataset.DatasetData.v1.Processed",
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
                "dataset.DatasetData.v1.Raw" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Raw(next(&mut f, budget)?))
                }
                "dataset.DatasetData.v1.Processed" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Processed(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for DatasetMetadata {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "dataset.DatasetMetadata.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "dataset.DatasetMetadata.v1", 4)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ReadSelection {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "dataset.ReadSelection.v1",
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
            let mut f = fields(node, "dataset.ReadSelection.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
};
