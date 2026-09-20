#[allow(unused_imports)]
use crate::reading::api::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for DatasetIdentity {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "read_api.DatasetIdentity.v1",
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
            let mut f = fields(node, "read_api.DatasetIdentity.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for Format {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Raw(v0) => {
                    record("read_api.Format.v1.Raw", vec![v0.to_wire(budget)?], budget)
                }
                Self::Processed(v0) => record(
                    "read_api.Format.v1.Processed",
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
                "read_api.Format.v1.Raw" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Raw(next(&mut f, budget)?))
                }
                "read_api.Format.v1.Processed" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Processed(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for MetadataField {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Subject => record("read_api.MetadataField.v1.Subject", vec![], budget),
                Self::Acquisition => {
                    record("read_api.MetadataField.v1.Acquisition", vec![], budget)
                }
                Self::SourceLabel => {
                    record("read_api.MetadataField.v1.SourceLabel", vec![], budget)
                }
                Self::Nucleus => record("read_api.MetadataField.v1.Nucleus", vec![], budget),
                Self::SpectralWidth => {
                    record("read_api.MetadataField.v1.SpectralWidth", vec![], budget)
                }
                Self::ObserveFrequency => {
                    record("read_api.MetadataField.v1.ObserveFrequency", vec![], budget)
                }
                Self::ReferenceFrequency => record(
                    "read_api.MetadataField.v1.ReferenceFrequency",
                    vec![],
                    budget,
                ),
                Self::Offset => record("read_api.MetadataField.v1.Offset", vec![], budget),
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
                "read_api.MetadataField.v1.Subject" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Subject)
                }
                "read_api.MetadataField.v1.Acquisition" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Acquisition)
                }
                "read_api.MetadataField.v1.SourceLabel" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::SourceLabel)
                }
                "read_api.MetadataField.v1.Nucleus" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Nucleus)
                }
                "read_api.MetadataField.v1.SpectralWidth" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::SpectralWidth)
                }
                "read_api.MetadataField.v1.ObserveFrequency" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::ObserveFrequency)
                }
                "read_api.MetadataField.v1.ReferenceFrequency" if values.is_empty() => {
                    Ok(Self::ReferenceFrequency)
                }
                "read_api.MetadataField.v1.Offset" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Offset)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ReadWarning {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::ExperimentalVendorSemantics { format, details } => record(
                    "read_api.ReadWarning.v1.ExperimentalVendorSemantics",
                    vec![format.to_wire(budget)?, details.to_wire(budget)?],
                    budget,
                ),
                Self::MissingOptionalSource {
                    kind,
                    role,
                    path,
                    impact,
                } => record(
                    "read_api.ReadWarning.v1.MissingOptionalSource",
                    vec![
                        kind.to_wire(budget)?,
                        role.to_wire(budget)?,
                        path.to_wire(budget)?,
                        impact.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::MissingMetadata {
                    field,
                    axis,
                    impact,
                } => record(
                    "read_api.ReadWarning.v1.MissingMetadata",
                    vec![
                        field.to_wire(budget)?,
                        axis.to_wire(budget)?,
                        impact.to_wire(budget)?,
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
                "read_api.ReadWarning.v1.ExperimentalVendorSemantics" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::ExperimentalVendorSemantics {
                        format: next(&mut f, budget)?,
                        details: next(&mut f, budget)?,
                    })
                }
                "read_api.ReadWarning.v1.MissingOptionalSource" if values.len() == 4 => {
                    let mut f = values.into_iter();
                    Ok(Self::MissingOptionalSource {
                        kind: next(&mut f, budget)?,
                        role: next(&mut f, budget)?,
                        path: next(&mut f, budget)?,
                        impact: next(&mut f, budget)?,
                    })
                }
                "read_api.ReadWarning.v1.MissingMetadata" if values.len() == 3 => {
                    let mut f = values.into_iter();
                    Ok(Self::MissingMetadata {
                        field: next(&mut f, budget)?,
                        axis: next(&mut f, budget)?,
                        impact: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for WarningImpact {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Identity => record("read_api.WarningImpact.v1.Identity", vec![], budget),
                Self::AxisCalibration => {
                    record("read_api.WarningImpact.v1.AxisCalibration", vec![], budget)
                }
                Self::AxisAnnotation => {
                    record("read_api.WarningImpact.v1.AxisAnnotation", vec![], budget)
                }
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
                "read_api.WarningImpact.v1.Identity" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Identity)
                }
                "read_api.WarningImpact.v1.AxisCalibration" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::AxisCalibration)
                }
                "read_api.WarningImpact.v1.AxisAnnotation" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::AxisAnnotation)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};

#[cfg(test)]
mod tests {
    use crate::snapshot::{
        SnapshotError, SnapshotLimits,
        wire::{Budget, Codec, Value},
    };
    use crate::{ExecutionContext, Format, ReadWarning, raw::RawFormat};

    #[test]
    fn experimental_warning_requires_the_fixed_two_field_v1_record() {
        for details in [
            Vec::new(),
            vec!["layout evidence remains incomplete".to_owned()],
        ] {
            static FORMAT: Format = Format::Raw(RawFormat::JeolDelta);
            let warning = ReadWarning::ExperimentalVendorSemantics {
                format: FORMAT,
                details: details.clone(),
            };
            let mut control = ExecutionContext::default();
            let mut budget = Budget::new(&mut control, SnapshotLimits::default());
            let Value::Record(tag, fields) = warning.to_wire(&mut budget).unwrap() else {
                panic!("expected a warning record");
            };
            assert_eq!(tag, "read_api.ReadWarning.v1.ExperimentalVendorSemantics");
            assert_eq!(fields.len(), 2);
            for (tag, count) in [
                ("read_api.ReadWarning.v1.ExperimentalVendorSemantics", 1),
                ("read_api.ReadWarning.v1.ExperimentalVendorSemantics", 2),
                ("read_api.ReadWarning.v1.ExperimentalVendorSemantics", 3),
                ("read_api.ReadWarning.v2.ExperimentalVendorSemantics", 2),
            ] {
                let mut fields = vec![FORMAT.to_wire(&mut budget).unwrap()];
                if count >= 2 {
                    fields.push(Value::Sequence(
                        details
                            .iter()
                            .cloned()
                            .map(|s| Value::Text(s.into()))
                            .collect(),
                    ));
                }
                if count == 3 {
                    fields.push(Value::Boolean(false));
                }
                let result = ReadWarning::from_wire(Value::Record(tag.into(), fields), &mut budget);
                if count == 2 && tag.contains(".v1.") {
                    assert_eq!(result.unwrap(), warning);
                } else {
                    assert!(matches!(result, Err(SnapshotError::Structure)));
                }
            }
        }
    }
}
