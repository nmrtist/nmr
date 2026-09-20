#[allow(unused_imports)]
use crate::raw_error::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for InputSource {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Path(v0) => record(
                    "raw_error.InputSource.v1.Path",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Memory { role } => record(
                    "raw_error.InputSource.v1.Memory",
                    vec![role.to_wire(budget)?],
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
                "raw_error.InputSource.v1.Path" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Path(next(&mut f, budget)?))
                }
                "raw_error.InputSource.v1.Memory" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Memory {
                        role: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};
