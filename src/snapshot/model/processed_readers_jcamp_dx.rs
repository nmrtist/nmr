#[allow(unused_imports)]
use crate::formats::jcamp_dx::decoding::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for Encoded {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Absolute(v0) => record(
                    "processed_readers.jcamp_dx.Encoded.v1.Absolute",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Difference(v0) => record(
                    "processed_readers.jcamp_dx.Encoded.v1.Difference",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Duplicate(v0) => record(
                    "processed_readers.jcamp_dx.Encoded.v1.Duplicate",
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
                "processed_readers.jcamp_dx.Encoded.v1.Absolute" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Absolute(next(&mut f, budget)?))
                }
                "processed_readers.jcamp_dx.Encoded.v1.Difference" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Difference(next(&mut f, budget)?))
                }
                "processed_readers.jcamp_dx.Encoded.v1.Duplicate" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Duplicate(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};
