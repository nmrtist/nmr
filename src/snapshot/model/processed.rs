#[allow(unused_imports)]
use crate::processed::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for BrukerSourceMetadata {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.BrukerSourceMetadata.v1",
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
            let mut f = fields(node, "processed.BrukerSourceMetadata.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for Format {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::BrukerTopSpin => record("processed.Format.v1.BrukerTopSpin", vec![], budget),
                Self::JcampDx => record("processed.Format.v1.JcampDx", vec![], budget),
                Self::JeolDelta => record("processed.Format.v1.JeolDelta", vec![], budget),
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
                "processed.Format.v1.BrukerTopSpin" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::BrukerTopSpin)
                }
                "processed.Format.v1.JcampDx" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::JcampDx)
                }
                "processed.Format.v1.JeolDelta" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::JeolDelta)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for JcampSourceMetadata {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.JcampSourceMetadata.v1",
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
            let mut f = fields(node, "processed.JcampSourceMetadata.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for JeolSourceMetadata {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.JeolSourceMetadata.v1",
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
            let mut f = fields(node, "processed.JeolSourceMetadata.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SourceMetadata {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.SourceMetadata.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processed.SourceMetadata.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SourceMetadataInner {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Empty => record("processed.SourceMetadataInner.v1.Empty", vec![], budget),
                Self::BrukerTopSpin(v0) => record(
                    "processed.SourceMetadataInner.v1.BrukerTopSpin",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::JcampDx(v0) => record(
                    "processed.SourceMetadataInner.v1.JcampDx",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::JeolDelta(v0) => record(
                    "processed.SourceMetadataInner.v1.JeolDelta",
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
                "processed.SourceMetadataInner.v1.Empty" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Empty)
                }
                "processed.SourceMetadataInner.v1.BrukerTopSpin" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::BrukerTopSpin(next(&mut f, budget)?))
                }
                "processed.SourceMetadataInner.v1.JcampDx" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::JcampDx(next(&mut f, budget)?))
                }
                "processed.SourceMetadataInner.v1.JeolDelta" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::JeolDelta(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SourceParameterText {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processed.SourceParameterText.v1",
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
            let mut f = fields(node, "processed.SourceParameterText.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
};
