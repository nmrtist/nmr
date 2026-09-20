#[allow(unused_imports)]
use crate::processed::model::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessedAxis {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.core.ProcessedAxis.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processed.core.ProcessedAxis.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    // Keep the reviewed optional-field encoding expression unchanged.
    #[allow(clippy::borrow_deref_ref)]
    impl Codec for ProcessedAxisInner {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            if let Some(reference) = &(*self.model_parts().11) {
                return record(
                    "processed.core.ProcessedAxisInner.v1.Referenced",
                    vec![
                        (*self.model_parts().0).to_wire(budget)?,
                        (*self.model_parts().1).to_wire(budget)?,
                        (*self.model_parts().2).to_wire(budget)?,
                        (*self.model_parts().3).to_wire(budget)?,
                        (*self.model_parts().4).to_wire(budget)?,
                        (*self.model_parts().5).to_wire(budget)?,
                        (*self.model_parts().6).to_wire(budget)?,
                        (*self.model_parts().7).to_wire(budget)?,
                        (*self.model_parts().8).to_wire(budget)?,
                        (*self.model_parts().9).to_wire(budget)?,
                        (*self.model_parts().10).to_wire(budget)?,
                        reference.to_wire(budget)?,
                    ],
                    budget,
                );
            }
            record(
                "processed.core.ProcessedAxisInner.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                    (*self.model_parts().5).to_wire(budget)?,
                    (*self.model_parts().6).to_wire(budget)?,
                    (*self.model_parts().7).to_wire(budget)?,
                    (*self.model_parts().8).to_wire(budget)?,
                    (*self.model_parts().9).to_wire(budget)?,
                    (*self.model_parts().10).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let referenced = matches!(&node, Value::Record(tag, _) if tag == "processed.core.ProcessedAxisInner.v1.Referenced");
            let mut f = if referenced {
                fields(node, "processed.core.ProcessedAxisInner.v1.Referenced", 12)?
            } else {
                fields(node, "processed.core.ProcessedAxisInner.v1", 11)?
            };
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                if referenced {
                    Some(next(&mut f, budget)?)
                } else {
                    None
                },
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessedData {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.core.ProcessedData.v1",
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
            let mut f = fields(node, "processed.core.ProcessedData.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessedDescriptor {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.core.ProcessedDescriptor.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processed.core.ProcessedDescriptor.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
};
