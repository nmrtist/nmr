#[allow(unused_imports)]
use crate::axis::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for AxisCoordinates {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Unknown => record("axis.AxisCoordinates.v1.Unknown", vec![], budget),
                Self::Uniform { start, step } => record(
                    "axis.AxisCoordinates.v1.Uniform",
                    vec![start.to_wire(budget)?, step.to_wire(budget)?],
                    budget,
                ),
                Self::Explicit(v0) => record(
                    "axis.AxisCoordinates.v1.Explicit",
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
                "axis.AxisCoordinates.v1.Unknown" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Unknown)
                }
                "axis.AxisCoordinates.v1.Uniform" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Uniform {
                        start: next(&mut f, budget)?,
                        step: next(&mut f, budget)?,
                    })
                }
                "axis.AxisCoordinates.v1.Explicit" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Explicit(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for AxisDomain {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Time => record("axis.AxisDomain.v1.Time", vec![], budget),
                Self::Frequency => record("axis.AxisDomain.v1.Frequency", vec![], budget),
                Self::Parameter => record("axis.AxisDomain.v1.Parameter", vec![], budget),
                Self::Unknown => record("axis.AxisDomain.v1.Unknown", vec![], budget),
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
                "axis.AxisDomain.v1.Time" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Time)
                }
                "axis.AxisDomain.v1.Frequency" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Frequency)
                }
                "axis.AxisDomain.v1.Parameter" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Parameter)
                }
                "axis.AxisDomain.v1.Unknown" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Unknown)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for AxisQuantity {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::MagneticFieldGradientStrength => record(
                    "axis.AxisQuantity.v1.MagneticFieldGradientStrength",
                    vec![],
                    budget,
                ),
                Self::TimeDelay => record("axis.AxisQuantity.v1.TimeDelay", vec![], budget),
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
                "axis.AxisQuantity.v1.MagneticFieldGradientStrength" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::MagneticFieldGradientStrength)
                }
                "axis.AxisQuantity.v1.TimeDelay" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::TimeDelay)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for AxisRole {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::DirectAcquisition => {
                    record("axis.AxisRole.v1.DirectAcquisition", vec![], budget)
                }
                Self::IndirectAcquisition => {
                    record("axis.AxisRole.v1.IndirectAcquisition", vec![], budget)
                }
                Self::ArrayParameter => record("axis.AxisRole.v1.ArrayParameter", vec![], budget),
                Self::Signal => record("axis.AxisRole.v1.Signal", vec![], budget),
                Self::Unknown => record("axis.AxisRole.v1.Unknown", vec![], budget),
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
                "axis.AxisRole.v1.DirectAcquisition" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::DirectAcquisition)
                }
                "axis.AxisRole.v1.IndirectAcquisition" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::IndirectAcquisition)
                }
                "axis.AxisRole.v1.ArrayParameter" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::ArrayParameter)
                }
                "axis.AxisRole.v1.Signal" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Signal)
                }
                "axis.AxisRole.v1.Unknown" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Unknown)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for AxisUnit {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Second => record("axis.AxisUnit.v1.Second", vec![], budget),
                Self::Hertz => record("axis.AxisUnit.v1.Hertz", vec![], budget),
                Self::Ppm => record("axis.AxisUnit.v1.Ppm", vec![], budget),
                Self::Tesla => record("axis.AxisUnit.v1.Tesla", vec![], budget),
                Self::TeslaPerMeter => record("axis.AxisUnit.v1.TeslaPerMeter", vec![], budget),
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
                "axis.AxisUnit.v1.Second" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Second)
                }
                "axis.AxisUnit.v1.Hertz" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Hertz)
                }
                "axis.AxisUnit.v1.Ppm" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Ppm)
                }
                "axis.AxisUnit.v1.Tesla" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Tesla)
                }
                "axis.AxisUnit.v1.TeslaPerMeter" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::TeslaPerMeter)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for FrequencyEvidence {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "axis.FrequencyEvidence.v1",
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
            let mut f = fields(node, "axis.FrequencyEvidence.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
};
