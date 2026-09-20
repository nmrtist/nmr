use super::header::JdfHeader;
use super::parameters::ParameterRecord;
use super::parameters::ParameterValue;

use crate::acquisition::registry;
use crate::acquisition::{
    ChemicalShiftReference, ComponentEvidence, DirectSamples, GroupDelayState, IndirectComponents,
    NormalizationEvidence, NormalizationFact, PendingGroupDelay, RawAxisKind,
};
use crate::{
    AcquisitionDescriptor, AcquisitionMetadata, Axis, AxisCoordinates, AxisQuantity, AxisRole,
    AxisUnit, DiffusionAcquisition, Domain, FrequencyEvidence, ReadError,
};
use std::collections::BTreeMap;

/// `axis_types` describes storage, not the pulse sequence's quadrature scheme.
/// A typed pn_type=y declares Y echo/antiecho (Delta's pn_to_shr step).
/// Do not infer this from an experiment name or from four sections alone.
pub(super) fn pn_y(
    header: &JdfHeader,
    parameters: &BTreeMap<String, ParameterRecord>,
) -> Result<bool, ReadError> {
    let Some(record) = parameters.get("pn_type") else {
        return Ok(false);
    };
    if matches!(record.value(), ParameterValue::String(value) if value.trim().eq_ignore_ascii_case("y"))
        && header.axis_types == [3, 3]
        && header
            .axis_units
            .iter()
            .all(|unit| *unit == Some(AxisUnit::Second))
    {
        return Ok(true);
    }
    Err(ReadError::invalid_metadata(
        header.input_source.clone(),
        Some("pn_type"),
        "unsupported JEOL P/N declaration; only time-domain [3,3] with pn_type=y is resolved",
    ))
}

pub(super) fn descriptor_from_header(
    header: &JdfHeader,
    parameters: &BTreeMap<String, ParameterRecord>,
    shape: &[usize],
    component_lanes: &[usize],
    coordinate_overrides: &[Option<AxisCoordinates>],
    embedded_axes: &[super::EmbeddedAxisEvidence],
) -> Result<AcquisitionDescriptor, ReadError> {
    let pn_y = pn_y(header, parameters)?;
    let ndim = shape.len();
    let direct_group_delay = digital_filter_group_delay(parameters)?;
    let axes = (0..ndim)
        .map(|output_axis| {
            let disk_axis = ndim - 1 - output_axis;
            // Time endpoints describe the valid acquisition interval. Storage
            // padding is not elapsed acquisition time, and the valid start has
            // already been incorporated in axis_start by the source.
            let time_interval = header.axis_units[disk_axis] == Some(AxisUnit::Second)
                && (disk_axis == 0 || header.axis_types[disk_axis] != 1);
            let raw_points = if time_interval {
                header.offset_stop[disk_axis] - header.offset_start[disk_axis] + 1
            } else {
                header.points_disk[disk_axis]
            };
            let raw_step = if raw_points > 1
                && header.axis_start[disk_axis].is_finite()
                && header.axis_stop[disk_axis].is_finite()
            {
                Some(
                    (header.axis_stop[disk_axis] - header.axis_start[disk_axis])
                        / (raw_points - 1) as f64,
                )
            } else {
                None
            };
            let role = if output_axis + 1 == ndim {
                AxisRole::DirectAcquisition
            } else if header.axis_types[disk_axis] == 1 {
                AxisRole::ArrayParameter
            } else {
                AxisRole::IndirectAcquisition
            };
            let prefix = ["x", "y", "z", "a"][disk_axis];
            let embedded = embedded_axes.iter().find(|v| v.disk_axis() == disk_axis);
            let parameter = if let Some(evidence) = embedded {
                ArrayParameterEvidence {
                    label: Some(evidence.parameter().to_owned()),
                    unit: Some(evidence.unit()),
                    quantity: Some(if evidence.unit() == AxisUnit::Second {
                        AxisQuantity::TimeDelay
                    } else {
                        AxisQuantity::MagneticFieldGradientStrength
                    }),
                }
            } else if role == AxisRole::ArrayParameter {
                array_parameter_evidence(parameters, prefix, header.axis_units[disk_axis])
            } else {
                ArrayParameterEvidence {
                    label: None,
                    unit: header.axis_units[disk_axis],
                    quantity: None,
                }
            };
            let unit = parameter.unit;
            let scale = unit.map_or(1.0, |_| {
                10f64.powi(i32::from(header.raw_axis_units[disk_axis].prefix_exponent))
            });
            let coordinates = match (unit, raw_step) {
                (None, _) => AxisCoordinates::Unknown,
                (Some(_), Some(step)) if scale.is_finite() && step.is_finite() && step != 0.0 => {
                    let start = (header.axis_start[disk_axis]
                        + step
                            * if time_interval {
                                0.0
                            } else {
                                header.offset_start[disk_axis] as f64
                            })
                        * scale;
                    let step = step * scale;
                    if start.is_finite() && step.is_finite() && step != 0.0 {
                        AxisCoordinates::Uniform { start, step }
                    } else {
                        AxisCoordinates::Unknown
                    }
                }
                (Some(_), None)
                    if raw_points == 1
                        && header.axis_start[disk_axis].is_finite()
                        && scale.is_finite() =>
                {
                    let value = header.axis_start[disk_axis] * scale;
                    if value.is_finite() {
                        AxisCoordinates::Explicit(vec![value])
                    } else {
                        AxisCoordinates::Unknown
                    }
                }
                _ => AxisCoordinates::Unknown,
            };
            let coordinates = coordinate_overrides
                .get(output_axis)
                .and_then(Clone::clone)
                .unwrap_or(coordinates);
            let coordinates = if let Some(evidence) = embedded {
                let textual = &evidence.values()
                    [header.offset_start[disk_axis]..=header.offset_stop[disk_axis]];
                if let Some(binary) = coordinate_overrides
                    .get(output_axis)
                    .and_then(Option::as_ref)
                {
                    // Binary lists are already SI-normalized by decode_axis_lists;
                    // embedded scalars are normalized by the declaration parser.
                    // Allow floating arithmetic roundoff, not acquisition rounding.
                    // The abbreviated JEOL gradient header stores only Tesla;
                    // the explicit T/m declaration supplies the missing /m.
                    let compatible_unit = header.axis_units[disk_axis] == Some(evidence.unit())
                        || (header.axis_units[disk_axis] == Some(AxisUnit::Tesla)
                            && evidence.unit() == AxisUnit::TeslaPerMeter);
                    let agrees = compatible_unit
                        && matches!(binary, AxisCoordinates::Explicit(values)
                            if values.len() == textual.len() && values.iter().zip(textual).all(|(a, b)|
                                (a - b).abs() <= 64.0 * f64::EPSILON * a.abs().max(b.abs())));
                    if !agrees {
                        return Err(ReadError::invalid_metadata(
                            header.input_source.clone(),
                            Some(evidence.parameter()),
                            "conflicting JEOL binary and textual axis lists",
                        ));
                    }
                    // Preserve binary precision when both sources agree.
                    binary.clone()
                } else {
                    AxisCoordinates::Explicit(textual.to_vec())
                }
            } else {
                coordinates
            };
            let nucleus = parameter_string(parameters, &format!("{prefix}_domain"));
            let sweep_hz = parameter_number_with_unit(parameters, &format!("{prefix}_sweep"), 13);
            let observe_hz = parameter_number_with_unit(parameters, &format!("{prefix}_freq"), 13);
            let offset_hz = frequency_offset_hz(parameters, prefix, observe_hz);
            let reference_mhz = header.base_frequency[disk_axis];
            let frequency_reference = if observe_hz.is_some()
                || offset_hz.is_some()
                || reference_mhz.is_finite() && reference_mhz > 0.0
            {
                Some(FrequencyEvidence::new(
                    observe_hz.map(|value| value / 1e6),
                    offset_hz,
                )?)
            } else {
                None
            };
            let chemical_shift_reference = offset_hz
                .filter(|_| reference_mhz.is_finite() && reference_mhz > 0.0)
                .map(|offset_hz| {
                    ChemicalShiftReference::resolved(
                        offset_hz / reference_mhz,
                        reference_mhz,
                        NormalizationEvidence::from_format(
                            if role == AxisRole::DirectAcquisition {
                                registry::JEOL_DIRECT_V1
                            } else if header.axis_types == [4, 4] {
                                registry::JEOL_SHARED_2D_V1
                            } else {
                                registry::JEOL_CARTESIAN_2D_V1
                            },
                            vec![NormalizationFact::ChemicalShiftReference],
                            vec![registry::CHEMICAL_SHIFT_V1],
                        )?,
                    )
                })
                .transpose()?;
            let kind = if role == AxisRole::ArrayParameter {
                RawAxisKind::Parameter
            } else if role == AxisRole::DirectAcquisition {
                RawAxisKind::Direct(if matches!(header.axis_types[disk_axis], 3 | 4) {
                    DirectSamples::Complex
                } else {
                    DirectSamples::Real
                })
            } else if header.axis_types[disk_axis] == 3 {
                let evidence = NormalizationEvidence::from_format(
                    if pn_y { registry::JEOL_PN_2D_V1 } else { registry::JEOL_CARTESIAN_2D_V1 },
                    vec![
                        NormalizationFact::CanonicalLaneOrder {
                            lanes: std::num::NonZeroUsize::new(2).expect("two is nonzero"),
                        },
                        NormalizationFact::PublicComplexConvention,
                        NormalizationFact::TraceMappingBijection,
                    ],
                    vec![if pn_y { registry::JEOL_PN_V1 } else { registry::JEOL_CARTESIAN_V1 }],
                )?;
                RawAxisKind::Indirect(IndirectComponents::Cartesian(ComponentEvidence::resolved(
                    evidence,
                )))
            } else if header.axis_types == [4, 4] {
                RawAxisKind::Indirect(IndirectComponents::SharedComplex {
                    conjugated: true,
                    evidence: ComponentEvidence::resolved(NormalizationEvidence::from_format(
                        registry::JEOL_SHARED_2D_V1,
                        vec![
                            NormalizationFact::PublicComplexConvention,
                            NormalizationFact::TraceMappingBijection,
                        ],
                        vec![registry::JEOL_SHARED_V1],
                    )?),
                })
            } else {
                RawAxisKind::Indirect(IndirectComponents::Scalar)
            };
            if kind.lane_count() != component_lanes[output_axis] {
                return Err(ReadError::invalid_metadata(
                    header.input_source.clone(),
                    None::<String>,
                    "JEOL section mapping disagrees with resolved axis semantics",
                ));
            }
            let mut axis = Axis::new(
                kind,
                if role == AxisRole::ArrayParameter {
                    Domain::Parameter
                } else {
                    unit.map_or(Domain::Unknown, AxisUnit::domain)
                },
                unit,
                shape[output_axis],
                coordinates,
            )?
            .with_label(parameter.label)
            .with_quantity(parameter.quantity)?;
            if role != AxisRole::ArrayParameter {
                axis = axis
                    .with_nucleus(nucleus.or_else(|| header.axis_titles[disk_axis].clone()))?
                    .with_spectral_width_hz(sweep_hz)?
                    .with_frequency_evidence(frequency_reference)?
                    .with_chemical_shift_reference(chemical_shift_reference)?;
            }
            if role == AxisRole::DirectAcquisition && axis.domain() == Domain::Time {
                axis = axis.with_group_delay(direct_group_delay.clone())?;
            }
            Ok::<Axis, ReadError>(axis)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scans = match parameter_integer(parameters, "scans", &header.input_source)? {
        Some(value) => Some(value),
        None => parameter_integer(parameters, "scan", &header.input_source)?,
    };
    let temperature_kelvin = parameter_number_with_unit(parameters, "temp_get", 4)
        .map(|celsius| celsius + 273.15)
        .map(|value| {
            if value >= 0.0 && value.is_finite() {
                Ok(value)
            } else {
                Err(ReadError::invalid_metadata(
                    header.input_source.clone(),
                    Some("temp_get"),
                    "JEOL temperature is below absolute zero",
                ))
            }
        })
        .transpose()?;
    let acquisition = AcquisitionMetadata::new(
        header.title.clone(),
        parameter_string(parameters, "solvent"),
        temperature_kelvin,
        scans,
        parameter_string(parameters, "experiment"),
    )?
    .with_diffusion(diffusion_acquisition(parameters, &axes)?)?;
    let rule = if pn_y {
        registry::JEOL_PN_2D_V1
    } else if header.axis_types == [3, 3] {
        registry::JEOL_CARTESIAN_2D_V1
    } else if header.axis_types == [4, 4] {
        registry::JEOL_SHARED_2D_V1
    } else {
        registry::JEOL_DIRECT_V1
    };
    Ok(AcquisitionDescriptor::new_resolved(
        axes,
        acquisition,
        NormalizationEvidence::from_format(
            rule,
            vec![NormalizationFact::TraceMappingBijection],
            vec![registry::FORMAT_LAYOUT_V1],
        )?,
    )?)
}

fn digital_filter_group_delay(
    parameters: &BTreeMap<String, ParameterRecord>,
) -> Result<GroupDelayState, ReadError> {
    let Some(enabled) = parameter_string(parameters, "digital_filter") else {
        return Ok(GroupDelayState::Unknown);
    };
    if enabled.eq_ignore_ascii_case("false") {
        return Ok(GroupDelayState::NotApplicable);
    }
    if !enabled.eq_ignore_ascii_case("true") {
        return Ok(GroupDelayState::Unknown);
    }

    let Some(orders) = parameter_number_list(parameters, "orders") else {
        return Ok(GroupDelayState::Unknown);
    };
    let Some(factors) = parameter_number_list(parameters, "factors") else {
        return Ok(GroupDelayState::Unknown);
    };
    let Some((&stage_count, taps)) = orders.split_first() else {
        return Ok(GroupDelayState::Unknown);
    };
    if stage_count.fract() != 0.0
        || stage_count < 1.0
        || stage_count as usize != taps.len()
        || taps.len() != factors.len()
        || taps
            .iter()
            .chain(&factors)
            .any(|value| !value.is_finite() || *value < 1.0 || value.fract() != 0.0)
    {
        return Ok(GroupDelayState::Unknown);
    }

    let mut cumulative_decimation = 1.0;
    let mut delay_at_input_rate = 0.0;
    for (&tap_count, &factor) in taps.iter().zip(&factors) {
        delay_at_input_rate += (tap_count - 1.0) * 0.5 * cumulative_decimation;
        cumulative_decimation *= factor;
    }
    let delay = delay_at_input_rate / cumulative_decimation;
    if !delay.is_finite() {
        return Ok(GroupDelayState::Unknown);
    }
    let evidence = NormalizationEvidence::from_format(
        registry::JEOL_DIRECT_V1,
        vec![NormalizationFact::DigitalFilterDelay],
        vec![registry::GROUP_DELAY_V1],
    )?;
    Ok(GroupDelayState::Pending(PendingGroupDelay::resolved(
        delay, evidence,
    )?))
}

fn parameter_number_list(
    parameters: &BTreeMap<String, ParameterRecord>,
    name: &str,
) -> Option<Vec<f64>> {
    parameter_string(parameters, name)?
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().ok())
        .collect()
}

struct ArrayParameterEvidence {
    label: Option<String>,
    unit: Option<AxisUnit>,
    quantity: Option<AxisQuantity>,
}

fn array_parameter_evidence(
    parameters: &BTreeMap<String, ParameterRecord>,
    axis_prefix: &str,
    header_unit: Option<AxisUnit>,
) -> ArrayParameterEvidence {
    let Some(parameter_name) = parameter_string(parameters, &format!("{axis_prefix}_acq")) else {
        return ArrayParameterEvidence {
            label: None,
            unit: header_unit,
            quantity: None,
        };
    };
    let Some(record) = parameters.get(&parameter_name.to_ascii_lowercase()) else {
        return ArrayParameterEvidence {
            label: Some(parameter_name),
            unit: header_unit,
            quantity: None,
        };
    };
    let units = record.raw_units();
    if signed_unit_power(units[0]) == 1
        && units[1] == 31
        && signed_unit_power(units[2]) == -1
        && units[3] == 19
        && units[4..].iter().all(|value| *value == 0)
    {
        ArrayParameterEvidence {
            label: Some(parameter_name),
            unit: Some(AxisUnit::TeslaPerMeter),
            quantity: Some(AxisQuantity::MagneticFieldGradientStrength),
        }
    } else if signed_unit_power(units[0]) == 1
        && units[1] == 28
        && units[2..].iter().all(|value| *value == 0)
    {
        ArrayParameterEvidence {
            label: Some(parameter_name),
            unit: Some(AxisUnit::Second),
            quantity: Some(AxisQuantity::TimeDelay),
        }
    } else if units[2..].iter().any(|value| *value != 0) {
        ArrayParameterEvidence {
            label: Some(parameter_name),
            unit: None,
            quantity: None,
        }
    } else {
        ArrayParameterEvidence {
            label: Some(parameter_name),
            unit: header_unit,
            quantity: None,
        }
    }
}

fn frequency_offset_hz(
    parameters: &BTreeMap<String, ParameterRecord>,
    axis_prefix: &str,
    observe_hz: Option<f64>,
) -> Option<f64> {
    let name = format!("{axis_prefix}_offset");
    parameter_number_with_unit(parameters, &name, 13).or_else(|| {
        let offset_ppm = parameter_number_with_unit(parameters, &name, 26)?;
        let offset_hz = offset_ppm * observe_hz? / 1e6;
        offset_hz.is_finite().then_some(offset_hz)
    })
}

fn diffusion_acquisition(
    parameters: &BTreeMap<String, ParameterRecord>,
    axes: &[Axis],
) -> Result<Option<DiffusionAcquisition>, crate::raw::ValidationError> {
    let Some((gradient_axis, axis)) = axes
        .iter()
        .enumerate()
        .find(|(_, axis)| axis.quantity() == Some(AxisQuantity::MagneticFieldGradientStrength))
    else {
        return Ok(None);
    };
    let Some(gradient_parameter) = axis.label() else {
        return Ok(None);
    };
    let small_delta = parameter_seconds(parameters, "delta")
        .map(|value| (value, "delta"))
        .or_else(|| {
            parameter_seconds(parameters, "delta_small").map(|value| (value, "delta_small"))
        });
    let diffusion_time = parameter_seconds(parameters, "delta_large")
        .map(|value| (value, "delta_large"))
        .or_else(|| {
            parameter_seconds(parameters, "diffusion_time").map(|value| (value, "diffusion_time"))
        });
    let (Some((small_delta, small_source)), Some((diffusion_time, diffusion_source))) =
        (small_delta, diffusion_time)
    else {
        return Ok(None);
    };
    if small_delta <= 0.0 || diffusion_time <= 0.0 {
        return Ok(None);
    }
    let recovery_delay = parameter_seconds(parameters, "tau")
        .filter(|value| *value >= 0.0)
        .map(|value| (value, "tau".to_owned()));
    let gradient_shape =
        parameter_string(parameters, "grad_shape").map(|value| (value, "grad_shape".to_owned()));
    DiffusionAcquisition::new(
        gradient_axis,
        gradient_parameter.to_owned(),
        small_delta,
        small_source.to_owned(),
        diffusion_time,
        diffusion_source.to_owned(),
        recovery_delay,
        gradient_shape,
    )
    .map(Some)
}

fn parameter_seconds(parameters: &BTreeMap<String, ParameterRecord>, name: &str) -> Option<f64> {
    let record = parameters.get(&name.to_ascii_lowercase())?;
    let units = record.raw_units();
    if signed_unit_power(units[0]) != 1
        || units[1] != 28
        || units[2..].iter().any(|value| *value != 0)
    {
        return None;
    }
    let prefix = units[0] >> 4;
    let signed_prefix = if prefix < 8 {
        prefix as i8
    } else {
        prefix as i8 - 16
    };
    let value = record.scaled_f64()? * 10f64.powi(-3 * i32::from(signed_prefix));
    value.is_finite().then_some(value)
}

fn signed_unit_power(scaler: u8) -> i8 {
    let power = (scaler & 0x0f) as i8;
    if power < 8 { power } else { power - 16 }
}

pub(super) fn parameter_string(
    parameters: &BTreeMap<String, ParameterRecord>,
    name: &str,
) -> Option<String> {
    parameters
        .get(&name.to_ascii_lowercase())
        .and_then(|record| match record.value() {
            ParameterValue::String(value) if !value.trim().is_empty() => {
                Some(value.trim().to_owned())
            }
            _ => None,
        })
}

pub(super) fn parameter_number_with_unit(
    parameters: &BTreeMap<String, ParameterRecord>,
    name: &str,
    base_code: u8,
) -> Option<f64> {
    parameters
        .get(&name.to_ascii_lowercase())
        .and_then(|record| {
            let unit = record.primary_unit();
            (unit.power == 1 && unit.base_code == base_code)
                .then(|| record.scaled_f64())
                .flatten()
        })
}

pub(super) fn parameter_integer(
    parameters: &BTreeMap<String, ParameterRecord>,
    name: &str,
    source: &crate::raw::InputSource,
) -> Result<Option<u64>, ReadError> {
    let Some(record) = parameters.get(&name.to_ascii_lowercase()) else {
        return Ok(None);
    };
    let value = record.scaled_f64().ok_or_else(|| {
        ReadError::invalid_metadata(
            source.clone(),
            Some(name),
            format!("JEOL parameter {name} is not an integer"),
        )
    })?;
    if value >= 0.0 && value.fract() == 0.0 && value <= u64::MAX as f64 {
        Ok(Some(value as u64))
    } else {
        Err(ReadError::invalid_metadata(
            source.clone(),
            Some(name),
            format!("JEOL parameter {name} is not a non-negative integer"),
        ))
    }
}
