use super::header::JdfHeader;
use super::metadata;
use super::parameters::ParameterRecord;
use super::parameters::ParameterValue;
use super::parameters::Parameters;
use crate::AxisCoordinates;
use crate::ReadError;
use crate::SamplingCoordinate;
use crate::SamplingSchedule;
use crate::raw::InputSource;
use std::collections::BTreeMap;

/// Evidence status for one JEOL axis/section layout.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LayoutEvidenceStatus {
    /// Synthetic regressions and selected real-file audits exist; layout-wide evidence is incomplete.
    Experimental,
    /// The layout is recognized but rejected until authoritative evidence exists.
    RejectedPendingEvidence,
}

/// Returns the normative evidence status for a JEOL axis/section layout.
pub(crate) fn layout_evidence_status(axis_types: &[u8]) -> LayoutEvidenceStatus {
    match axis_types {
        [1] | [3] | [3, 1] | [3, 3] | [4, 4] => LayoutEvidenceStatus::Experimental,
        _ => LayoutEvidenceStatus::RejectedPendingEvidence,
    }
}

pub(crate) fn decode_axis_lists(
    header: &JdfHeader,
    mut read: impl FnMut(usize, usize) -> Result<Vec<u8>, ReadError>,
) -> Result<Vec<Option<Vec<f64>>>, ReadError> {
    let mut lists = Vec::with_capacity(header.points_disk.len());
    for axis in 0..header.points_disk.len() {
        let length = header.list_length[axis];
        if length == 0 {
            lists.push(None);
            continue;
        }
        if length / 8 != header.points_disk[axis] {
            return Err(ReadError::corrupt(
                header.input_source.clone(),
                format!(
                    "JEOL axis {} coordinate-list length disagrees with its point count",
                    axis + 1
                ),
            ));
        }
        let bytes = read(header.list_start[axis], length)?;
        let scale = 10f64.powi(i32::from(header.raw_axis_units[axis].prefix_exponent));
        let mut values = Vec::with_capacity(length / 8);
        for chunk in bytes.chunks_exact(8) {
            let value = f64::from_be_bytes(chunk.try_into().unwrap()) * scale;
            if !value.is_finite() {
                return Err(ReadError::corrupt(
                    header.input_source.clone(),
                    "JEOL axis coordinate list contains a non-finite value",
                ));
            }
            values.push(value);
        }
        lists.push(Some(values));
    }
    Ok(lists)
}

pub(crate) fn coordinate_overrides(
    header: &JdfHeader,
    lists: &[Option<Vec<f64>>],
) -> Result<Vec<Option<AxisCoordinates>>, ReadError> {
    if lists.len() != header.points_disk.len() {
        return Err(ReadError::corrupt(
            header.input_source.clone(),
            "JEOL coordinate-list rank mismatch",
        ));
    }
    (0..header.points_disk.len())
        .rev()
        .map(|disk_axis| {
            let Some(values) = &lists[disk_axis] else {
                return Ok(None);
            };
            let start = header.offset_start[disk_axis];
            let end = header.offset_stop[disk_axis]
                .checked_add(1)
                .ok_or(ReadError::SizeOverflow)?;
            let values = values.get(start..end).ok_or_else(|| {
                ReadError::corrupt(
                    header.input_source.clone(),
                    "JEOL coordinate list does not cover the valid window",
                )
            })?;
            Ok(Some(AxisCoordinates::Explicit(values.to_vec())))
        })
        .collect()
}

pub(crate) struct JeolSamplingPlan {
    pub(super) logical_points: usize,
    pub(super) acquired_indices: Vec<usize>,
    pub(super) schedule: SamplingSchedule,
    pub(super) coordinates: AxisCoordinates,
}

pub(crate) fn jeol_sampling_plan(
    header: &JdfHeader,
    parameters: &BTreeMap<String, ParameterRecord>,
    lists: &[Option<Vec<f64>>],
    declaration: Option<&std::sync::Arc<crate::SamplingDeclaration>>,
    cancellation: Option<&crate::CancellationToken>,
) -> Result<Option<JeolSamplingPlan>, ReadError> {
    if header.points_disk.len() != 2 {
        if declaration.is_some() {
            return Err(ReadError::sampling_declaration(
                "JEOL sampling declarations require a two-dimensional reduced acquisition",
            ));
        }
        return Ok(None);
    }
    let disk_axis = 1;
    let acquired_points = header.points_disk[disk_axis];
    let Some(original_points) =
        metadata::parameter_integer(parameters, "y_orig_points", &header.input_source)?
            .map(usize::try_from)
            .transpose()
            .map_err(|_| ReadError::SizeOverflow)?
    else {
        if declaration.is_some() {
            return Err(ReadError::sampling_declaration(
                "JEOL sampling declaration requires vendor y_orig_points",
            ));
        }
        return Ok(None);
    };
    if original_points <= acquired_points {
        if declaration.is_some() {
            return Err(ReadError::sampling_declaration(
                "JEOL sampling declaration requires vendor evidence of a reduced grid",
            ));
        }
        return Ok(None);
    }
    let values = lists.get(disk_axis).and_then(Option::as_ref);
    if values.is_none() && declaration.is_none() {
        return Err(ReadError::unsupported_code(
            header.input_source.clone(),
            crate::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT,
            "JEOL reduced indirect point count has no coordinate list",
        ));
    }
    if declaration.is_some()
        && (header.offset_start[disk_axis] != 0
            || header.offset_stop[disk_axis] + 1 != acquired_points)
    {
        return Err(ReadError::sampling_declaration(
            "JEOL sampling declaration does not support a cropped indirect observation list",
        ));
    }
    let sweep_hz = metadata::parameter_number_with_unit(parameters, "y_sweep", 13)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| {
            ReadError::unsupported_code(
                header.input_source.clone(),
                crate::raw::UnsupportedFeatureCode::MISSING_CALIBRATION,
                "JEOL reduced indirect point count has no typed sweep width",
            )
        })?;
    let dwell = sweep_hz.recip();
    let mut acquired_indices = Vec::with_capacity(values.map_or(0, Vec::len));
    for &coordinate in values.into_iter().flatten() {
        let index = coordinate / dwell;
        let rounded = index.round();
        let tolerance = 1e-6 * index.abs().max(1.0);
        if !index.is_finite()
            || (index - rounded).abs() > tolerance
            || rounded < 0.0
            || rounded >= original_points as f64
        {
            return Err(ReadError::unsupported_code(
                header.input_source.clone(),
                crate::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT,
                "JEOL listed indirect coordinate does not map to the original sampling grid",
            ));
        }
        acquired_indices.push(rounded as usize);
    }
    let coordinates = acquired_indices
        .iter()
        .copied()
        .map(|index| SamplingCoordinate::new(vec![index]))
        .collect();
    let vendor = if values.is_some() {
        Some(SamplingSchedule::new(vec![original_points], coordinates)?)
    } else {
        None
    };
    let schedule = if let Some(declaration) = declaration {
        declaration.resolve(
            &[original_points],
            &[if header.axis_types[disk_axis] == 3 {
                2
            } else {
                1
            }],
            acquired_points,
            vendor.as_ref(),
            cancellation,
        )?
    } else {
        vendor.expect("coordinate list was checked above")
    };
    let acquired_indices = schedule
        .coordinates()
        .iter()
        .map(|c| c.as_slice()[0])
        .collect();
    Ok(Some(JeolSamplingPlan {
        logical_points: original_points,
        acquired_indices,
        schedule,
        coordinates: AxisCoordinates::Uniform {
            start: 0.0,
            step: dwell,
        },
    }))
}

pub(crate) fn explicit_experiment(parameters: &Parameters) -> Option<String> {
    ["experiment", "content"]
        .into_iter()
        .find_map(|name| {
            parameters
                .get(name)
                .and_then(|record| match record.value() {
                    ParameterValue::String(value) => Some(value),
                    _ => None,
                })
        })
        .map(|value| {
            let value = value.trim();
            value
                .get(..value.len().saturating_sub(4))
                .filter(|_| {
                    value
                        .get(value.len().saturating_sub(4)..)
                        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".jxp"))
                })
                .unwrap_or(value)
                .to_owned()
        })
        .filter(|value| !value.is_empty())
}

pub(crate) fn validate_supported_layout(
    axis_types: &[u8],
    source: &InputSource,
) -> Result<(), ReadError> {
    if layout_evidence_status(axis_types) == LayoutEvidenceStatus::Experimental {
        Ok(())
    } else {
        Err(ReadError::unsupported(
            source.clone(),
            format!(
                "JEOL axis/section layout {axis_types:?} is rejected pending authoritative section/lane/sign evidence"
            ),
        ))
    }
}

pub(crate) fn require_experimental_opt_in(
    enabled: bool,
    source: InputSource,
) -> Result<(), ReadError> {
    if enabled {
        Ok(())
    } else {
        Err(ReadError::unsupported_feature(
            source,
            crate::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS,
            None,
            vec!["JEOL section/sign, tile, list and filter interpretations have incomplete independent evidence; set allow_experimental_vendor_semantics(true) to opt in".into()],
        ))
    }
}

pub(crate) fn jeol_complex_dims(
    axis_types: &[u8],
    source: &InputSource,
) -> Result<usize, ReadError> {
    let mut complex_dims = 0usize;
    for &axis_type in axis_types {
        match axis_type {
            0 => break,
            1 | 2 | 5 => {}
            3 => complex_dims += 1,
            4 => return Ok(1),
            _ => {
                return Err(ReadError::unsupported_code(
                    source.clone(),
                    crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
                    "unknown JEOL quadrature",
                ));
            }
        }
    }
    Ok(complex_dims)
}

/// Successful schedule validation is not evidence for section or acquisition semantics.
/// Keep this policy beside the admission matrix; selected-file audits do not qualify
/// every precision, tile, domain, coordinate list or acquisition of a layout.
pub(crate) fn experimental_details(
    parameters: &Parameters,
    kind: crate::DatasetKind,
    sampled: bool,
) -> Vec<String> {
    let axes = parameters.axis_types();
    debug_assert_eq!(
        layout_evidence_status(axes),
        LayoutEvidenceStatus::Experimental
    );
    let coverage = match (kind, axes) {
        (crate::DatasetKind::Raw, [3]) => {
            "selected proton raw components and FFT independently audited; other encoding and calibration combinations remain unqualified"
        }
        (crate::DatasetKind::Raw, [4, 4]) => {
            "selected COSY shared-complex components, orientation and FFT independently audited; other encoding and calibration combinations remain unqualified"
        }
        (crate::DatasetKind::Raw, [3, 3]) => {
            "selected HSQC components and declared P/N inversion independently audited; general acquisition semantics remain unqualified"
        }
        _ => {
            "synthetic regression coverage only; independent section, lane, sign and calibration evidence remains incomplete"
        }
    };
    let mut details = vec![format!("JEOL disk-axis layout {axes:?}: {coverage}")];
    if kind == crate::DatasetKind::Processed {
        details.push(
            "Processed frequency-domain semantics are not qualified by raw-data FFT audits".into(),
        );
    }
    if sampled {
        details.push("NUS coordinate validation does not independently establish sampling-list, acquisition or reconstruction semantics, including externally declared schedules".into());
    }
    if !parameters.embedded_axes().is_empty() {
        details.push(
            "Embedded parameter-axis list/ramp interpretation lacks independent qualification"
                .into(),
        );
    }
    if parameters.get("digital_filter").is_some_and(|record|
        matches!(record.value(), ParameterValue::String(value) if value.eq_ignore_ascii_case("true")))
    {
        details.push("Digital-filter order/factor interpretation has selected-file numerical checks, not general vendor qualification".into());
    }
    details
}
