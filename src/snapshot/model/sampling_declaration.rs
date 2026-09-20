#[allow(unused_imports)]
use crate::raw::model::sampling_declaration::{SamplingDeclaration, SamplingIndexBase};
const _: () = {
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    impl Codec for SamplingDeclaration {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "read.SamplingDeclaration.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    Value::Boolean((*self.model_parts().4) == SamplingIndexBase::One),
                    (*self.model_parts().5).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "read.SamplingDeclaration.v1", 6)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                if next(&mut f, budget)? {
                    SamplingIndexBase::One
                } else {
                    SamplingIndexBase::Zero
                },
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
};
