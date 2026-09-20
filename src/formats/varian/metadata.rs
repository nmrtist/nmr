use super::parameters::Parameters;
use super::semantics::AcquisitionLayout;
use super::semantics::frequency_reference;
use super::semantics::parse_finite_f64;
use super::semantics::parse_positive_f64;
use super::semantics::parse_u64;
use super::semantics::text_scalar;

use crate::acquisition::registry;
use crate::acquisition::{
    ChemicalShiftReference, DirectSamples, IndirectComponents, NormalizationEvidence,
    NormalizationFact, RawAxisKind, ResolutionAuthority, RuleId,
};
use crate::{
    AcquisitionDescriptor, AcquisitionMetadata, Axis, AxisCoordinates, AxisUnit, Domain, ReadError,
};

pub(super) fn descriptor_from_layout(
    parameters: &Parameters,
    layout: &AcquisitionLayout,
    complex: bool,
) -> Result<AcquisitionDescriptor, ReadError> {
    let spectral_width_hz = parse_positive_f64(parameters, "sw")?;
    let reference = frequency_reference(parameters, "sfrq", "tof", Some("reffrq"))?;
    let chemical_reference = chemical_shift_reference(
        parameters,
        "sw",
        "rfl",
        "rfp",
        "reffrq",
        registry::VARIAN_DIRECT_V1,
    )?;
    let mut axes = Vec::with_capacity(layout.shape.len());
    for (index, &points) in layout.shape.iter().enumerate() {
        let direct = index + 1 == layout.shape.len();
        let array = layout.array_dimensions.get(index);
        let indirect = (!direct && array.is_none())
            .then(|| index.checked_sub(layout.array_dimensions.len()))
            .flatten()
            .and_then(|index| layout.indirect.get(index));
        let sweep = if direct {
            spectral_width_hz
        } else if let Some(specification) = indirect {
            parse_positive_f64(parameters, specification.sweep_parameter)?
        } else {
            None
        };
        let coordinates = array.map_or_else(
            || {
                sweep
                    .filter(|_| !direct || complex)
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .map_or(AxisCoordinates::Unknown, |value| AxisCoordinates::Uniform {
                        start: 0.0,
                        step: 1.0 / value,
                    })
            },
            |dimension| dimension.coordinates.clone(),
        );
        let kind = if array.is_some() {
            RawAxisKind::Parameter
        } else if direct {
            RawAxisKind::Direct(if complex {
                DirectSamples::Complex
            } else {
                DirectSamples::Real
            })
        } else {
            let indirect_index = index - layout.array_dimensions.len();
            RawAxisKind::Indirect(layout.indirect_components[indirect_index].clone())
        };
        if kind.lane_count() != layout.component_lanes[index] {
            return Err(ReadError::invalid_metadata(
                parameters.input_source.clone(),
                None::<String>,
                "Varian resolved component semantics disagree with trace layout",
            ));
        }
        let mut axis = Axis::new(
            kind,
            if array.is_some() {
                Domain::Parameter
            } else if matches!(coordinates, AxisCoordinates::Unknown) {
                Domain::Unknown
            } else {
                Domain::Time
            },
            array.map_or_else(
                || (!matches!(coordinates, AxisCoordinates::Unknown)).then_some(AxisUnit::Second),
                |dimension| dimension.unit,
            ),
            points,
            coordinates,
        )?
        .with_label(array.and_then(|dimension| dimension.label.clone()));
        if direct {
            axis = axis
                .with_nucleus(text_scalar(parameters, "tn")?.map(str::to_owned))?
                .with_spectral_width_hz(
                    spectral_width_hz.filter(|value| value.is_finite() && *value > 0.0),
                )?
                .with_frequency_evidence(reference)?
                .with_chemical_shift_reference(chemical_reference.clone())?;
        } else if let Some(specification) = indirect {
            let indirect_index = index - layout.array_dimensions.len();
            let rule = match &layout.indirect_components[indirect_index] {
                IndirectComponents::Encoded(transform) => match transform.evidence().authority() {
                    ResolutionAuthority::FormatRule(rule) => *rule,
                    _ => registry::VARIAN_DIRECT_V1,
                },
                _ => registry::VARIAN_DIRECT_V1,
            };
            axis = axis
                .with_nucleus(
                    text_scalar(parameters, specification.nucleus_parameter)?.map(str::to_owned),
                )?
                .with_spectral_width_hz(sweep.filter(|value| value.is_finite() && *value > 0.0))?
                .with_frequency_evidence(frequency_reference(
                    parameters,
                    specification.observe_frequency_parameter,
                    specification.transmitter_offset_parameter,
                    specification.reference_frequency_parameter,
                )?)?
                .with_chemical_shift_reference(
                    specification
                        .reference_frequency_parameter
                        .map(|reference| {
                            chemical_shift_reference(
                                parameters,
                                specification.sweep_parameter,
                                specification.reference_offset_parameter,
                                specification.reference_position_parameter,
                                reference,
                                rule,
                            )
                        })
                        .transpose()?
                        .flatten(),
                )?;
        }
        axes.push(axis);
    }
    let temperature_kelvin = parse_finite_f64(parameters, "temp")?
        .map(|celsius| celsius + 273.15)
        .map(|kelvin| {
            if kelvin < 0.0 {
                Err(super::projection_error(
                    parameters,
                    "temp",
                    "temperature is below absolute zero",
                ))
            } else {
                Ok(kelvin)
            }
        })
        .transpose()?;
    let acquisition = AcquisitionMetadata::new(
        text_scalar(parameters, "comment")?.map(str::to_owned),
        text_scalar(parameters, "solvent")?.map(str::to_owned),
        temperature_kelvin,
        parse_u64(parameters, "nt")?,
        text_scalar(parameters, "seqfil")?.map(str::to_owned),
    )?;
    let layout_evidence = layout
        .indirect_components
        .iter()
        .find_map(|components| match components {
            IndirectComponents::Encoded(value) => Some(value.evidence().clone()),
            _ => None,
        })
        .map(Ok)
        .unwrap_or_else(|| {
            NormalizationEvidence::from_format(
                if layout.array_dimensions.is_empty() {
                    registry::VARIAN_DIRECT_V1
                } else {
                    registry::VARIAN_DIRECT_ARRAY_V1
                },
                vec![NormalizationFact::TraceMappingBijection],
                vec![registry::FORMAT_LAYOUT_V1],
            )
        })?;
    Ok(AcquisitionDescriptor::new_resolved(
        axes,
        acquisition,
        layout_evidence,
    )?)
}

fn chemical_shift_reference(
    parameters: &Parameters,
    sweep_parameter: &str,
    reference_offset_parameter: &str,
    reference_position_parameter: &str,
    reference_parameter: &str,
    rule: RuleId,
) -> Result<Option<ChemicalShiftReference>, ReadError> {
    let Some(sweep_hz) = parse_positive_f64(parameters, sweep_parameter)? else {
        return Ok(None);
    };
    // OpenVnmrJ get_reference defaults absent rfl/rfp to zero independently.
    let reference_offset_hz =
        parse_finite_f64(parameters, reference_offset_parameter)?.unwrap_or(0.0);
    let reference_position_hz =
        parse_finite_f64(parameters, reference_position_parameter)?.unwrap_or(0.0);
    let Some(reference_mhz) = parse_positive_f64(parameters, reference_parameter)? else {
        return Ok(None);
    };
    Ok(Some(ChemicalShiftReference::resolved(
        (sweep_hz / 2.0 - reference_offset_hz + reference_position_hz) / reference_mhz,
        reference_mhz,
        NormalizationEvidence::from_format(
            rule,
            vec![NormalizationFact::ChemicalShiftReference],
            vec![
                registry::VARIAN_CHEMICAL_SHIFT_V1,
                registry::CHEMICAL_SHIFT_V1,
            ],
        )?,
    )?))
}
