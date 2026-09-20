#[allow(unused_imports)]
use crate::processing::contracts::profile::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for NormalizedAcmeV1 {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record("processing_profile.NormalizedAcmeV1.v1", vec![], budget)
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_profile.NormalizedAcmeV1.v1", 0)?;
            let value = Self;
            Ok(value)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for PositivePeaksV1 {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record("processing_profile.PositivePeaksV1.v1", vec![], budget)
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_profile.PositivePeaksV1.v1", 0)?;
            let value = Self;
            Ok(value)
        }
    }
};
