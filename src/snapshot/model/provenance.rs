#[allow(unused_imports)]
use crate::provenance::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for InputAxisRef {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "provenance.InputAxisRef.v1",
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
            let mut f = fields(node, "provenance.InputAxisRef.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for InputSlot {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "provenance.InputSlot.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "provenance.InputSlot.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessedReadRecord {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "provenance.ProcessedReadRecord.v1",
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
            let mut f = fields(node, "provenance.ProcessedReadRecord.v1", 4)?;
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
    impl Codec for ProcessedReadTransform {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::JeolDelta2D {
                    float64,
                    big_endian,
                    disk_points,
                    crop_start,
                    crop_points,
                    submatrix_edge,
                    imaginary_multiplier,
                } => record(
                    "provenance.ProcessedReadTransform.v1.JeolDelta2D",
                    vec![
                        float64.to_wire(budget)?,
                        big_endian.to_wire(budget)?,
                        disk_points.to_wire(budget)?,
                        crop_start.to_wire(budget)?,
                        crop_points.to_wire(budget)?,
                        submatrix_edge.to_wire(budget)?,
                        imaginary_multiplier.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::Bruker {
                    nc_proc,
                    float64,
                    big_endian,
                } => record(
                    "provenance.ProcessedReadTransform.v1.Bruker",
                    vec![
                        nc_proc.to_wire(budget)?,
                        float64.to_wire(budget)?,
                        big_endian.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::JcampDx { x_factor, y_factor } => record(
                    "provenance.ProcessedReadTransform.v1.JcampDx",
                    vec![x_factor.to_wire(budget)?, y_factor.to_wire(budget)?],
                    budget,
                ),
                Self::JeolDelta {
                    float64,
                    big_endian,
                    disk_points,
                    crop_start,
                    crop_points,
                    imaginary_multiplier,
                    submatrix_edge,
                    coordinate_scale,
                    explicit_coordinates,
                } => record(
                    "provenance.ProcessedReadTransform.v1.JeolDelta",
                    vec![
                        float64.to_wire(budget)?,
                        big_endian.to_wire(budget)?,
                        disk_points.to_wire(budget)?,
                        crop_start.to_wire(budget)?,
                        crop_points.to_wire(budget)?,
                        imaginary_multiplier.to_wire(budget)?,
                        submatrix_edge.to_wire(budget)?,
                        coordinate_scale.to_wire(budget)?,
                        explicit_coordinates.to_wire(budget)?,
                    ],
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
                "provenance.ProcessedReadTransform.v1.JeolDelta2D" if values.len() == 7 => {
                    let mut f = values.into_iter();
                    Ok(Self::JeolDelta2D {
                        float64: next(&mut f, budget)?,
                        big_endian: next(&mut f, budget)?,
                        disk_points: next(&mut f, budget)?,
                        crop_start: next(&mut f, budget)?,
                        crop_points: next(&mut f, budget)?,
                        submatrix_edge: next(&mut f, budget)?,
                        imaginary_multiplier: next(&mut f, budget)?,
                    })
                }
                "provenance.ProcessedReadTransform.v1.Bruker" if values.len() == 3 => {
                    let mut f = values.into_iter();
                    Ok(Self::Bruker {
                        nc_proc: next(&mut f, budget)?,
                        float64: next(&mut f, budget)?,
                        big_endian: next(&mut f, budget)?,
                    })
                }
                "provenance.ProcessedReadTransform.v1.JcampDx" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::JcampDx {
                        x_factor: next(&mut f, budget)?,
                        y_factor: next(&mut f, budget)?,
                    })
                }
                "provenance.ProcessedReadTransform.v1.JeolDelta" if values.len() == 9 => {
                    let mut f = values.into_iter();
                    Ok(Self::JeolDelta {
                        float64: next(&mut f, budget)?,
                        big_endian: next(&mut f, budget)?,
                        disk_points: next(&mut f, budget)?,
                        crop_start: next(&mut f, budget)?,
                        crop_points: next(&mut f, budget)?,
                        imaginary_multiplier: next(&mut f, budget)?,
                        submatrix_edge: next(&mut f, budget)?,
                        coordinate_scale: next(&mut f, budget)?,
                        explicit_coordinates: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SampleNormalization {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "provenance.SampleNormalization.v1",
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
            let mut f = fields(node, "provenance.SampleNormalization.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SourceDigest {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::NotComputed => {
                    record("provenance.SourceDigest.v1.NotComputed", vec![], budget)
                }
                Self::Sha256(v0) => record(
                    "provenance.SourceDigest.v1.Sha256",
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
                "provenance.SourceDigest.v1.NotComputed" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::NotComputed)
                }
                "provenance.SourceDigest.v1.Sha256" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Sha256(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SourceFile {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "provenance.SourceFile.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                    (*self.model_parts().5).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "provenance.SourceFile.v1", 6)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
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
    impl Codec for SourceId {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "provenance.SourceId.v1",
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
            let mut f = fields(node, "provenance.SourceId.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SourceIdentity {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "provenance.SourceIdentity.v1",
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
            let mut f = fields(node, "provenance.SourceIdentity.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SourceKind {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Data => record("provenance.SourceKind.v1.Data", vec![], budget),
                Self::Parameters => record("provenance.SourceKind.v1.Parameters", vec![], budget),
                Self::SamplingSchedule => {
                    record("provenance.SourceKind.v1.SamplingSchedule", vec![], budget)
                }
                Self::Other => record("provenance.SourceKind.v1.Other", vec![], budget),
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
                "provenance.SourceKind.v1.Data" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Data)
                }
                "provenance.SourceKind.v1.Parameters" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Parameters)
                }
                "provenance.SourceKind.v1.SamplingSchedule" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::SamplingSchedule)
                }
                "provenance.SourceKind.v1.Other" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Other)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};
