#[allow(unused_imports)]
use crate::processing::methods::combine::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for LinearCombination {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_combine.LinearCombination.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_combine.LinearCombination.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
};
