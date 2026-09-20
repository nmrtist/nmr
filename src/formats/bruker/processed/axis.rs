use crate::MetadataField;
use crate::ReadWarning;
use crate::WarningImpact;
use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisRole;
use crate::axis::AxisUnit;
use crate::processed::ComponentBasis;
use crate::processed::ProcessedAxis;
use crate::read_error::ReadError;
use std::path::Path;

use super::parameters::{Parameters, invalid_parameter};

#[allow(clippy::too_many_arguments)]
pub(super) fn axis(
    parameters: &Parameters,
    source: &Path,
    points: usize,
    label: &str,
    axis_index: usize,
    basis: ComponentBasis,
    warnings: &mut Vec<ReadWarning>,
) -> Result<ProcessedAxis, ReadError> {
    let sw = valid_positive(parameters.optional_f64("SW_P", source)?, source, "SW_p")?;
    let sf = valid_positive(parameters.optional_f64("SF", source)?, source, "SF")?;
    let offset = valid_finite(parameters.optional_f64("OFFSET", source)?, source, "OFFSET")?;
    for (present, field) in [
        (sw.is_some(), MetadataField::SpectralWidth),
        (sf.is_some(), MetadataField::ReferenceFrequency),
        (offset.is_some(), MetadataField::Offset),
    ] {
        if !present {
            warnings.push(ReadWarning::MissingMetadata {
                field,
                axis: Some(axis_index),
                impact: WarningImpact::AxisCalibration,
            });
        }
    }
    let calibrated = sw.zip(sf).zip(offset);
    let (domain, unit, coordinates) = if let Some(((sw, sf), offset)) = calibrated {
        let step = -sw / sf / points as f64;
        if !step.is_finite() || points > 1 && step == 0.0 {
            return Err(ReadError::corrupt(
                source.into(),
                "invalid Bruker processed axis calibration",
            ));
        }
        (
            AxisDomain::Frequency,
            Some(AxisUnit::Ppm),
            if points == 1 {
                AxisCoordinates::Explicit(vec![offset])
            } else {
                AxisCoordinates::Uniform {
                    start: offset,
                    step,
                }
            },
        )
    } else {
        (AxisDomain::Unknown, None, AxisCoordinates::Unknown)
    };
    let mut axis = ProcessedAxis::new(AxisRole::Signal, domain, unit, points, coordinates, basis)?
        .with_label(Some(label.to_owned()));
    let nucleus = parameters.text("AXNUC");
    if nucleus.is_none() {
        warnings.push(ReadWarning::MissingMetadata {
            field: MetadataField::Nucleus,
            axis: Some(axis_index),
            impact: WarningImpact::AxisAnnotation,
        });
    }
    axis = axis.with_nucleus(nucleus)?.with_spectral_width_hz(sw)?;
    if let Some(sf) = sf {
        use crate::acquisition::{
            DerivationStepId, NormalizationEvidence, NormalizationFact, RuleId,
        };
        axis = axis.with_spectrum_reference(crate::processed::SpectrumReference {
            frequency_mhz: sf,
            evidence: NormalizationEvidence::from_format(
                RuleId::registered("bruker.processed-sf.v1"),
                vec![NormalizationFact::ChemicalShiftReference],
                vec![DerivationStepId::registered("bruker.processed-sf.v1")],
            )
            .expect("fixed nonempty reference rule"),
        });
    }
    Ok(axis)
}

pub(super) fn valid_positive(
    value: Option<f64>,
    source: &Path,
    name: &str,
) -> Result<Option<f64>, ReadError> {
    if value.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return Err(invalid_parameter(source, name));
    }
    Ok(value)
}

pub(super) fn valid_finite(
    value: Option<f64>,
    source: &Path,
    name: &str,
) -> Result<Option<f64>, ReadError> {
    if value.is_some_and(|value| !value.is_finite()) {
        return Err(invalid_parameter(source, name));
    }
    Ok(value)
}
