#[allow(unused_imports)]
use crate::raw::model::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for DiffusionAcquisition {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.DiffusionAcquisition.v1",
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
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "internal.core.DiffusionAcquisition.v1", 10)?;
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
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ObservationOrdinal {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.ObservationOrdinal.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "internal.core.ObservationOrdinal.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RawAxis {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.RawAxis.v1",
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
                    (*self.model_parts().11).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "internal.core.RawAxis.v1", 12)?;
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
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RawData {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.RawData.v1",
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
            let mut f = fields(node, "internal.core.RawData.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RawDataset {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.RawDataset.v1",
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
            let mut f = fields(node, "internal.core.RawDataset.v1", 4)?;
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
    impl Codec for RawDescriptor {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.RawDescriptor.v1",
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
            let mut f = fields(node, "internal.core.RawDescriptor.v1", 4)?;
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
    impl Codec for RawFormat {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::BrukerRaw => record("internal.core.RawFormat.v1.BrukerRaw", vec![], budget),
                Self::JeolDelta => record("internal.core.RawFormat.v1.JeolDelta", vec![], budget),
                Self::VarianRaw => record("internal.core.RawFormat.v1.VarianRaw", vec![], budget),
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
                "internal.core.RawFormat.v1.BrukerRaw" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::BrukerRaw)
                }
                "internal.core.RawFormat.v1.JeolDelta" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::JeolDelta)
                }
                "internal.core.RawFormat.v1.VarianRaw" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::VarianRaw)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RawLayout {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.RawLayout.v1",
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
            let mut f = fields(node, "internal.core.RawLayout.v1", 4)?;
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
    impl Codec for RawMetadata {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.RawMetadata.v1",
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
            let mut f = fields(node, "internal.core.RawMetadata.v1", 6)?;
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
    impl Codec for RawProvenance {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.RawProvenance.v1",
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
            let mut f = fields(node, "internal.core.RawProvenance.v1", 4)?;
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
    impl Codec for SampleRepresentation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Dense(v0) => record(
                    "internal.core.SampleRepresentation.v1.Dense",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Sparse(v0) => record(
                    "internal.core.SampleRepresentation.v1.Sparse",
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
                "internal.core.SampleRepresentation.v1.Dense" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Dense(next(&mut f, budget)?))
                }
                "internal.core.SampleRepresentation.v1.Sparse" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Sparse(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SamplingCoordinate {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.SamplingCoordinate.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "internal.core.SamplingCoordinate.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    // Keep the reviewed optional-field encoding expression unchanged.
    #[allow(clippy::borrow_deref_ref)]
    impl Codec for SamplingSchedule {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            if let Some(declaration) = &(*self.model_parts().3) {
                return record(
                    "internal.core.SamplingSchedule.v1.Declared",
                    vec![
                        (*self.model_parts().0).to_wire(budget)?,
                        (*self.model_parts().1).to_wire(budget)?,
                        (*self.model_parts().2).to_wire(budget)?,
                        declaration.to_wire(budget)?,
                    ],
                    budget,
                );
            }
            record(
                "internal.core.SamplingSchedule.v1",
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
            let declared = matches!(&node, Value::Record(tag, _) if tag == "internal.core.SamplingSchedule.v1.Declared");
            let mut f = if declared {
                fields(node, "internal.core.SamplingSchedule.v1.Declared", 4)?
            } else {
                fields(node, "internal.core.SamplingSchedule.v1", 3)?
            };
            let grid: Vec<usize> = next(&mut f, budget)?;
            let coordinates: Vec<SamplingCoordinate> = next(&mut f, budget)?;
            let recorded: bool = next(&mut f, budget)?;
            let declaration: Option<std::sync::Arc<crate::SamplingDeclaration>> = if declared {
                Some(next(&mut f, budget)?)
            } else {
                None
            };
            budget.reserve::<&SamplingCoordinate>(coordinates.len())?;
            let value = Self::new(grid, coordinates)
                .map_err(|e| SnapshotError::Validation(e.to_string()))?;
            if recorded != *value.model_parts().2 {
                return Err(SnapshotError::Structure);
            }
            if let Some(declaration) = declaration {
                budget.reserve::<SamplingCoordinate>(value.coordinates().len())?;
                budget.reserve::<usize>(
                    value
                        .coordinates()
                        .len()
                        .checked_mul(value.grid().len())
                        .ok_or(SnapshotError::SizeOverflow)?,
                )?;
                budget.reserve::<&SamplingCoordinate>(value.coordinates().len())?;
                return declaration
                    .resolve(
                        value.grid(),
                        declaration.indirect_lanes(),
                        value.coordinates().len(),
                        Some(&value),
                        Some(budget.control.cancellation()),
                    )
                    .map_err(SnapshotError::sampling_declaration);
            }
            Ok(value)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SparseTrace {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.SparseTrace.v1",
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
            let mut f = fields(node, "internal.core.SparseTrace.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for StorageOrder {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::RowMajorDirectFastest => record(
                    "internal.core.StorageOrder.v1.RowMajorDirectFastest",
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
                "internal.core.StorageOrder.v1.RowMajorDirectFastest" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::RowMajorDirectFastest)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for VendorMetadata {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "internal.core.VendorMetadata.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "internal.core.VendorMetadata.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for VendorMetadataInner {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Bruker(v0) => record(
                    "internal.core.VendorMetadataInner.v1.Bruker",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Jeol(v0) => record(
                    "internal.core.VendorMetadataInner.v1.Jeol",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Varian(v0) => record(
                    "internal.core.VendorMetadataInner.v1.Varian",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::None => record("internal.core.VendorMetadataInner.v1.None", vec![], budget),
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
                "internal.core.VendorMetadataInner.v1.Bruker" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Bruker(next(&mut f, budget)?))
                }
                "internal.core.VendorMetadataInner.v1.Jeol" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Jeol(next(&mut f, budget)?))
                }
                "internal.core.VendorMetadataInner.v1.Varian" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Varian(next(&mut f, budget)?))
                }
                "internal.core.VendorMetadataInner.v1.None" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::None)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};
