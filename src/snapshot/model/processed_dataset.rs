#[allow(unused_imports)]
use crate::processed::dataset::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessedDataset {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed_dataset.ProcessedDataset.v1",
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
            let mut f = fields(node, "processed_dataset.ProcessedDataset.v1", 5)?;
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
    impl Codec for ProcessedProvenance {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed_dataset.ProcessedProvenance.v1",
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
            let mut f = fields(node, "processed_dataset.ProcessedProvenance.v1", 5)?;
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
};
