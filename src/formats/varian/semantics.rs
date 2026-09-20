use super::layout::LayoutPlan;
use super::options::ReadAssertions;
use super::options::TraceOrder;
use super::options::VarianOptions;
use super::parameters::BlockHeader;
use super::parameters::FileHeader;
use super::parameters::ParameterRecord;
use super::parameters::ParameterValue;
use super::parameters::Parameters;
use super::parser;
use crate::AxisCoordinates;
use crate::AxisUnit;
use crate::Complex64;
use crate::FrequencyEvidence;
use crate::ReadError;
use crate::SamplingCoordinate;
use crate::SamplingSchedule;
use crate::acquisition::DirectSamples;
use crate::acquisition::IndirectComponents;
use crate::acquisition::LinearComponentTransform;
use crate::acquisition::NormalizationEvidence;
use crate::acquisition::NormalizationFact;
use crate::acquisition::PeriodicLaneModulation;
use crate::acquisition::ResolvedComponentTransform;
use crate::acquisition::registry;
use crate::raw::InputSource;
use crate::raw::ParameterError;
use crate::raw::RawFormat;
use std::fs;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;

impl LayoutPlan {
    pub(super) fn parse(
        path: &Path,
        source_len: usize,
        options: &VarianOptions,
    ) -> Result<(Self, FileHeader, Vec<BlockHeader>), ReadError> {
        if source_len < 32 {
            return Err(ReadError::truncated(path.into(), 32, source_len));
        }
        let mut file = fs::File::open(path).map_err(|error| ReadError::io(path, error))?;
        let mut header = [0u8; 32];
        file.read_exact(&mut header)
            .map_err(|error| ReadError::io(path, error))?;
        let source = InputSource::from(path);
        let parsed = parser::parse_binary(&header, source_len, options, &source, |offset| {
            file.seek(SeekFrom::Start(
                u64::try_from(offset).map_err(|_| ReadError::SizeOverflow)?,
            ))
            .map_err(|error| ReadError::io(path, error))?;
            let mut common = [0u8; 28];
            file.read_exact(&mut common)
                .map_err(|error| ReadError::io(path, error))?;
            Ok(common)
        })?;
        Ok((parsed.layout, parsed.file_header, parsed.block_headers))
    }
}

pub(crate) fn nus_enabled(value: &ParameterValue) -> bool {
    match value {
        ParameterValue::Real(value) => *value != 0.0,
        ParameterValue::String(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "n" | "no" | "false" | "off"
        ),
    }
}
#[derive(Clone, Debug)]
pub(crate) struct IndirectDimension {
    pub(super) phase_parameter: &'static str,
    pub(super) sweep_parameter: &'static str,
    pub(super) reference_offset_parameter: &'static str,
    pub(super) reference_position_parameter: &'static str,
    pub(super) nucleus_parameter: &'static str,
    pub(super) observe_frequency_parameter: &'static str,
    pub(super) transmitter_offset_parameter: &'static str,
    pub(super) reference_frequency_parameter: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub(crate) struct ArrayDimension {
    pub(super) label: Option<String>,
    pub(super) coordinates: AxisCoordinates,
    pub(super) unit: Option<AxisUnit>,
}

#[derive(Clone, Debug)]
pub(crate) struct AcquisitionLayout {
    pub(super) shape: Vec<usize>,
    pub(super) component_lanes: Vec<usize>,
    pub(super) array_dimensions: Vec<ArrayDimension>,
    pub(super) indirect: Vec<IndirectDimension>,
    pub(super) indirect_components: Vec<IndirectComponents>,
    pub(super) asserted_trace_permutation: Option<Vec<usize>>,
}

pub(crate) fn resolve_layout_from_procpar(
    pp: &Parameters,
    direct_points: usize,
    stored_traces: usize,
    scheduled: bool,
    assertions: Option<&ReadAssertions>,
) -> Result<AcquisitionLayout, ReadError> {
    match layout_from_procpar(pp, direct_points, stored_traces, scheduled) {
        Ok(layout) => Ok(layout),
        Err(error)
            if matches!(
                error.reason(),
                crate::raw::ReadErrorReason::UnsupportedFeature { code, .. }
                    if *code == crate::raw::UnsupportedFeatureCode::VARIAN_UNSUPPORTED_COMPONENT_LAYOUT
            ) =>
        {
            asserted_layout_from_missing_facts(
                pp,
                direct_points,
                stored_traces,
                scheduled,
                assertions,
            )?
            .ok_or(error)
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn asserted_layout_from_missing_facts(
    pp: &Parameters,
    direct_points: usize,
    stored_traces: usize,
    scheduled: bool,
    assertions: Option<&ReadAssertions>,
) -> Result<Option<AcquisitionLayout>, ReadError> {
    let Some(assertions) = assertions else {
        return Ok(None);
    };
    let transform = assertions.transform.clone();
    if scheduled
        || parse_usize(pp, "ni2")?.unwrap_or(1).max(1) > 1
        || parse_usize(pp, "ni3")?.unwrap_or(1).max(1) > 1
    {
        return Ok(None);
    }
    let ni = parse_usize(pp, "ni")?.unwrap_or(1).max(1);
    if ni <= 1 || !array_parameters(pp)?.is_empty() {
        return Ok(None);
    }
    if pp
        .get("f1coef")
        .is_some_and(|record| !record.values().is_empty())
    {
        return Ok(None);
    }
    let lanes = transform.input_lanes();
    let implied_traces = ni.checked_mul(lanes).ok_or(ReadError::SizeOverflow)?;
    if implied_traces != stored_traces
        || parse_usize(pp, "arraydim")?.is_some_and(|value| value != implied_traces)
        || assertions.trace_permutation.len() != stored_traces
    {
        return Err(ReadError::assertion_conflict(
            "asserted transform/permutation disagrees with ni, arraydim, or fid trace count",
        ));
    }
    if transform.transform().modulation().period() > 1
        && matches!(
            transform.transform().modulation().domain(),
            crate::raw::ModulationIndexDomain::ObservationOrdinal
        )
        && !assertions
            .trace_permutation
            .iter()
            .copied()
            .eq(0..stored_traces)
    {
        return Err(ReadError::assertion_conflict(
            "observation-ordinal modulation requires an identity trace permutation; use absolute-grid modulation for reordered traces",
        ));
    }
    let homonuclear = text_scalar(pp, "apptype")?
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("homo2d"));
    Ok(Some(AcquisitionLayout {
        shape: vec![ni, direct_points],
        component_lanes: vec![lanes, 1],
        array_dimensions: Vec::new(),
        indirect: vec![IndirectDimension {
            phase_parameter: "phase",
            sweep_parameter: "sw1",
            reference_offset_parameter: "rfl1",
            reference_position_parameter: "rfp1",
            nucleus_parameter: if homonuclear { "tn" } else { "dn" },
            observe_frequency_parameter: if homonuclear { "sfrq" } else { "dfrq" },
            transmitter_offset_parameter: if homonuclear { "tof" } else { "dof" },
            reference_frequency_parameter: Some("reffrq1"),
        }],
        indirect_components: vec![IndirectComponents::Encoded(transform)],
        asserted_trace_permutation: Some(assertions.trace_permutation.clone()),
    }))
}

pub(crate) fn layout_from_procpar(
    pp: &Parameters,
    direct_points: usize,
    stored_traces: usize,
    scheduled: bool,
) -> Result<AcquisitionLayout, ReadError> {
    let ni = parse_usize(pp, "ni")?.unwrap_or(1).max(1);
    let ni2 = parse_usize(pp, "ni2")?.unwrap_or(1).max(1);
    let ni3 = parse_usize(pp, "ni3")?.unwrap_or(1).max(1);
    let array = array_parameters(pp)?;
    let arraydim = parse_usize(pp, "arraydim")?;

    if ni == 1 && ni2 == 1 && ni3 == 1 {
        if scheduled || array.iter().any(|name| name.starts_with("phase")) || array.len() > 1 {
            return Err(unsupported_component_layout(pp));
        }
        if arraydim.is_some_and(|value| value != stored_traces) {
            return Err(procpar_read_error(
                pp,
                format!("arraydim does not match {stored_traces} physical traces"),
            ));
        }
        let mut shape = Vec::new();
        let mut component_lanes = Vec::new();
        let mut array_dimensions = Vec::new();
        if let Some(name) = array.first() {
            let record = pp.get(name).ok_or_else(|| {
                procpar_read_error(pp, format!("array names {name} but procpar has no record"))
            })?;
            if record.values().len() != stored_traces {
                return Err(procpar_read_error(
                    pp,
                    format!(
                        "array parameter {name} has {} values but fid has {stored_traces} traces",
                        record.values().len()
                    ),
                ));
            }
            let coordinates = if matches!(name.as_str(), "d2" | "d3" | "d4") {
                AxisCoordinates::Explicit(
                    record
                        .values()
                        .iter()
                        .map(|value| {
                            value.as_f64().ok_or_else(|| {
                                procpar_read_error(
                                    pp,
                                    format!("array parameter {name} must contain real values"),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else {
                AxisCoordinates::Unknown
            };
            shape.push(stored_traces);
            component_lanes.push(1);
            array_dimensions.push(ArrayDimension {
                label: Some(name.clone()),
                coordinates,
                unit: matches!(name.as_str(), "d2" | "d3" | "d4").then_some(AxisUnit::Second),
            });
        } else if stored_traces != 1 {
            return Err(procpar_read_error(
                pp,
                "a direct Varian acquisition without an array must contain one trace",
            ));
        }
        shape.push(direct_points);
        component_lanes.push(1);
        return Ok(AcquisitionLayout {
            shape,
            component_lanes,
            array_dimensions,
            indirect: Vec::new(),
            indirect_components: Vec::new(),
            asserted_trace_permutation: None,
        });
    }

    if scheduled || ni <= 1 || ni2 > 1 || ni3 > 1 || array.as_slice() != ["phase"] {
        return Err(unsupported_component_layout(pp));
    }
    let phase = pp.get("phase").ok_or_else(|| {
        procpar_read_error(pp, "array names phase but procpar has no phase record")
    })?;
    if !phase.active()
        || phase.values().len() != 2
        || phase.values()[0].as_f64() != Some(1.0)
        || phase.values()[1].as_f64() != Some(2.0)
    {
        return Err(unsupported_component_layout(pp));
    }
    let expected_traces = ni.checked_mul(2).ok_or(ReadError::SizeOverflow)?;
    if arraydim != Some(expected_traces) || stored_traces != expected_traces {
        return Err(procpar_read_error(
            pp,
            format!(
                "2D phase layout requires arraydim, ni, phase, and fid trace count to agree at {expected_traces}"
            ),
        ));
    }

    let f1coef = pp.get("f1coef");
    let explicit = f1coef.is_some_and(|record| !record.values().is_empty());
    let coefficient_values =
        if let Some(record) = f1coef.filter(|record| !record.values().is_empty()) {
            let values = if let [ParameterValue::String(value)] = record.values() {
                value
                    .split_ascii_whitespace()
                    .map(|token| {
                        token
                            .parse::<f64>()
                            .ok()
                            .filter(|value| value.is_finite())
                            .ok_or_else(|| {
                                procpar_read_error(pp, "f1coef must contain finite real values")
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                record
                    .values()
                    .iter()
                    .map(|value| {
                        value.as_f64().ok_or_else(|| {
                            procpar_read_error(pp, "f1coef must contain finite real values")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };
            if values.len() != 8 {
                return Err(procpar_read_error(
                    pp,
                    format!(
                        "f1coef must contain exactly 8 finite real values, found {}",
                        values.len()
                    ),
                ));
            }
            values
        } else {
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        };
    // OpenVnmrJ combine() uses (a - i*b) on stored-sign complex lanes.
    // decoder already conjugates those lanes, so the canonical matrix uses
    // (a + i*b) for BOTH rows. An explicit f1coef is not evidence for an
    // additional F1 conjugation: importing ft2d's ntype processing switch here
    // negates the sine component and mirrors the indirect spectrum.
    let coefficients: Vec<_> = coefficient_values
        .chunks_exact(2)
        .map(|pair| Complex64::new(pair[0], pair[1]))
        .collect();
    let transform =
        LinearComponentTransform::try_new(2, coefficients, PeriodicLaneModulation::identity(2)?)
            .map_err(|error| {
                ReadError::unsupported_code(
                    pp.input_source.clone(),
                    crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
                    format!("resolved Varian component matrix is degenerate: {error}"),
                )
            })?;
    let (rule, derivation) = if explicit {
        (
            registry::VARIAN_EXPLICIT_F1COEF_V1,
            registry::VARIAN_EXPLICIT_F1COEF_DERIVATION_V1,
        )
    } else {
        (
            registry::VARIAN_DEFAULT_PTYPE_V1,
            registry::VARIAN_DEFAULT_PTYPE_DERIVATION_V1,
        )
    };
    let evidence = NormalizationEvidence::from_format(
        rule,
        vec![
            NormalizationFact::CanonicalLaneOrder {
                lanes: std::num::NonZeroUsize::new(2).expect("two is nonzero"),
            },
            NormalizationFact::TraceMappingBijection,
            NormalizationFact::SourceCoefficients {
                active: f1coef.is_some_and(ParameterRecord::active),
                values: coefficient_values.len(),
            },
        ],
        vec![derivation],
    )?;
    let components =
        IndirectComponents::Encoded(ResolvedComponentTransform::resolved(transform, evidence));
    let homonuclear = text_scalar(pp, "apptype")?
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("homo2d"));
    Ok(AcquisitionLayout {
        shape: vec![ni, direct_points],
        component_lanes: vec![2, 1],
        array_dimensions: Vec::new(),
        indirect: vec![IndirectDimension {
            phase_parameter: "phase",
            sweep_parameter: "sw1",
            reference_offset_parameter: "rfl1",
            reference_position_parameter: "rfp1",
            nucleus_parameter: if homonuclear { "tn" } else { "dn" },
            observe_frequency_parameter: if homonuclear { "sfrq" } else { "dfrq" },
            transmitter_offset_parameter: if homonuclear { "tof" } else { "dof" },
            reference_frequency_parameter: Some("reffrq1"),
        }],
        indirect_components: vec![components],
        asserted_trace_permutation: None,
    })
}

pub(crate) fn unsupported_component_layout(pp: &Parameters) -> ReadError {
    ReadError::unsupported_feature(
        pp.input_source.clone(),
        crate::raw::UnsupportedFeatureCode::VARIAN_UNSUPPORTED_COMPONENT_LAYOUT,
        None,
        vec!["varian.unsupported-component-layout.v1".to_owned()],
    )
}

pub(crate) fn validate_assertions(
    assertions: Option<&ReadAssertions>,
    source_complex: bool,
    stored_traces: usize,
    layout: &AcquisitionLayout,
) -> Result<(), ReadError> {
    let Some(assertions) = assertions else {
        return Ok(());
    };
    let source_direct = if source_complex {
        DirectSamples::Complex
    } else {
        DirectSamples::Real
    };
    if assertions.direct_samples != source_direct {
        return Err(ReadError::assertion_conflict(
            "asserted direct encoding disagrees with the fid status header",
        ));
    }
    let permutation_matches = layout.asserted_trace_permutation.as_ref().map_or_else(
        || {
            assertions.trace_permutation.len() == stored_traces
                && assertions
                    .trace_permutation
                    .iter()
                    .copied()
                    .eq(0..stored_traces)
        },
        |resolved| resolved == &assertions.trace_permutation,
    );
    if !permutation_matches {
        return Err(ReadError::assertion_conflict(
            "asserted trace permutation disagrees with the v1 physical trace mapping",
        ));
    }
    let source_transform =
        layout
            .indirect_components
            .iter()
            .find_map(|components| match components {
                IndirectComponents::Encoded(value) => Some(value),
                _ => None,
            });
    match source_transform {
        Some(source) if source.transform() == assertions.transform.transform() => Ok(()),
        _ => Err(ReadError::assertion_conflict(
            "asserted component transform disagrees with source-resolved semantics",
        )),
    }
}

pub(crate) fn projection_error(
    pp: &Parameters,
    name: &str,
    detail: impl Into<String>,
) -> ParameterError {
    ParameterError::new(
        RawFormat::VarianRaw,
        pp.input_source.clone(),
        Some(name.to_owned()),
        crate::raw::ParameterErrorKind::Invalid,
        detail,
    )
}

pub(crate) fn procpar_read_error(pp: &Parameters, detail: impl Into<String>) -> ReadError {
    ReadError::invalid_metadata(pp.input_source.clone(), None::<String>, detail)
}

pub(crate) fn layout_error(pp: &Parameters, detail: impl Into<String>) -> ReadError {
    ReadError::unsupported(pp.input_source.clone(), detail)
}

pub(crate) fn parse_usize(pp: &Parameters, name: &str) -> Result<Option<usize>, ParameterError> {
    real_scalar(pp, name)?
        .map(|value| {
            if value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
                Err(projection_error(
                    pp,
                    name,
                    "expected a non-negative integer within usize range",
                ))
            } else {
                Ok(value as usize)
            }
        })
        .transpose()
}

pub(crate) fn parse_positive_f64(
    pp: &Parameters,
    name: &str,
) -> Result<Option<f64>, ParameterError> {
    real_scalar(pp, name)?
        .map(|value| {
            if value <= 0.0 {
                return Err(projection_error(pp, name, "expected a positive value"));
            }
            Ok(value)
        })
        .transpose()
}

pub(crate) fn parse_finite_f64(pp: &Parameters, name: &str) -> Result<Option<f64>, ParameterError> {
    real_scalar(pp, name)
}

pub(crate) fn parse_u64(pp: &Parameters, name: &str) -> Result<Option<u64>, ParameterError> {
    real_scalar(pp, name)?
        .map(|value| {
            if value < 0.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
                Err(projection_error(
                    pp,
                    name,
                    "expected a non-negative integer within u64 range",
                ))
            } else {
                Ok(value as u64)
            }
        })
        .transpose()
}

pub(crate) fn scalar_parameter<'a>(
    pp: &'a Parameters,
    name: &str,
) -> Result<Option<&'a ParameterValue>, ParameterError> {
    let Some(record) = pp.get(name) else {
        return Ok(None);
    };
    match record.values() {
        [value] => Ok(Some(value)),
        values => Err(projection_error(
            pp,
            name,
            format!("expected a scalar value, found {} values", values.len()),
        )),
    }
}

pub(crate) fn real_scalar(pp: &Parameters, name: &str) -> Result<Option<f64>, ParameterError> {
    scalar_parameter(pp, name)?
        .map(|value| {
            value
                .as_f64()
                .ok_or_else(|| projection_error(pp, name, "expected real basic type"))
        })
        .transpose()
}

pub(crate) fn text_scalar<'a>(
    pp: &'a Parameters,
    name: &str,
) -> Result<Option<&'a str>, ParameterError> {
    scalar_parameter(pp, name)?
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| projection_error(pp, name, "expected string basic type"))
        })
        .transpose()
}

pub(crate) fn array_parameters(pp: &Parameters) -> Result<Vec<String>, ReadError> {
    let Some(value) = text_scalar(pp, "array")? else {
        return Ok(Vec::new());
    };
    if value.contains(['(', ')']) {
        return Err(layout_error(
            pp,
            "grouped Varian array parameters require an explicit layout parser",
        ));
    }
    Ok(value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect())
}

pub(crate) fn select_trace_order(
    pp: &Parameters,
    layout: &AcquisitionLayout,
    requested: Option<TraceOrder>,
) -> Result<TraceOrder, ReadError> {
    let indirect_rank = layout.indirect.len();
    let array_rank = layout.array_dimensions.len();
    let components = &layout.component_lanes[array_rank..array_rank + indirect_rank];
    let component_axes = components.iter().filter(|&&lanes| lanes > 1).count();
    if !layout.array_dimensions.is_empty() && component_axes > 0 {
        return Err(layout_error(
            pp,
            "combined non-phase arrays and indirect quadrature need an explicit trace mapper",
        ));
    }
    if indirect_rank <= 1 || component_axes <= 1 {
        if let Some(order @ (TraceOrder::Regular | TraceOrder::Opposite)) = requested {
            return Ok(order);
        }
        return Ok(TraceOrder::Flat);
    }
    if let Some(order) = requested {
        if order == TraceOrder::Flat {
            return Err(layout_error(
                pp,
                "flat trace order is only valid for at most one indirect component axis",
            ));
        }
        return Ok(order);
    }
    let array = array_parameters(pp)?;
    let phase_names: Vec<&str> = layout
        .indirect
        .iter()
        .zip(components)
        .filter_map(|(dimension, &lanes)| (lanes > 1).then_some(dimension.phase_parameter))
        .collect();
    let known_delays = ["d2", "d3", "d4"];
    if array
        .iter()
        .any(|name| !phase_names.contains(&name.as_str()) && !known_delays.contains(&name.as_str()))
    {
        return Err(layout_error(
            pp,
            "non-phase Varian array parameters are not representable by the inferred layout",
        ));
    }
    let positions = phase_names
        .iter()
        .map(|name| {
            array
                .iter()
                .position(|candidate| candidate == name)
                .ok_or_else(|| {
                    layout_error(
                        pp,
                        format!("array and {name} component metadata are inconsistent"),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if positions.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(TraceOrder::Regular)
    } else if positions.windows(2).all(|pair| pair[0] > pair[1]) {
        Ok(TraceOrder::Opposite)
    } else {
        Err(layout_error(
            pp,
            "Varian phase parameters are neither regular nor opposite ordered",
        ))
    }
}
pub(crate) fn frequency_reference(
    pp: &Parameters,
    observe_parameter: &str,
    transmitter_parameter: &str,
    reference_parameter: Option<&str>,
) -> Result<Option<FrequencyEvidence>, ReadError> {
    let observe = parse_positive_f64(pp, observe_parameter)?;
    let transmitter = parse_finite_f64(pp, transmitter_parameter)?;
    if observe.is_none() && transmitter.is_none() {
        Ok(None)
    } else {
        let _ = reference_parameter;
        Ok(Some(FrequencyEvidence::new(observe, transmitter)?))
    }
}
pub(crate) fn parse_schedule(
    text: &str,
    grid: &[usize],
    source: &InputSource,
) -> Result<SamplingSchedule, ReadError> {
    let mut coords = Vec::new();
    for line in text.lines() {
        let l = line.split('#').next().unwrap_or("").trim();
        if l.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = l.split_whitespace().collect();
        if tokens.len() != grid.len() - 1 {
            return Err(ReadError::corrupt(
                source.clone(),
                "NUS schedule row has the wrong number of coordinates",
            ));
        }
        let mut vals: Vec<usize> = tokens
            .into_iter()
            .map(|token| {
                token.parse().map_err(|_| {
                    ReadError::corrupt(
                        source.clone(),
                        "NUS schedule contains a non-integer coordinate",
                    )
                })
            })
            .collect::<Result<_, _>>()?;
        // sampling.sch is written in ni, ni2, ni3 order; the public model is
        // slowest indirect axis to fastest direct axis.
        vals.reverse();
        if vals.iter().enumerate().any(|(i, &v)| v >= grid[i]) {
            return Err(ReadError::corrupt(
                source.clone(),
                "schedule coordinate out of bounds",
            ));
        }
        coords.push(SamplingCoordinate::new(vals));
    }
    if coords.is_empty() {
        return Err(ReadError::incomplete(
            source.clone(),
            "NUS schedule is empty",
        ));
    }
    Ok(SamplingSchedule::new(
        grid[..grid.len() - 1].to_vec(),
        coords,
    )?)
}
