#[allow(unused_imports)]
use crate::reference::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for PolarityState {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Ambiguous180 => {
                    record("reference.PolarityState.v1.Ambiguous180", vec![], budget)
                }
                Self::UserAssertedPositive => record(
                    "reference.PolarityState.v1.UserAssertedPositive",
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
                "reference.PolarityState.v1.Ambiguous180" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Ambiguous180)
                }
                "reference.PolarityState.v1.UserAssertedPositive" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::UserAssertedPositive)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};
