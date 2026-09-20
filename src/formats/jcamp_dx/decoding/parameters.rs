use crate::ReadLimits;
use crate::axis::AxisUnit;
use crate::read_error::ReadError;
use crate::read_error::ReadResource;
use std::collections::BTreeMap;
use std::path::Path;

use super::Jcamp;
use super::numeric::decode_xydata;
use super::reading::enforce_working;

pub(super) fn enforce_metadata_limit(
    control: &mut crate::ExecutionContext<'_>,
    text: &str,
    limit: usize,
) -> Result<(), ReadError> {
    let mut total = 0usize;
    let mut in_data = false;
    for line in text.split_inclusive('\n') {
        control.check_cancelled()?;
        let trimmed = line.trim_start();
        if let Some(record) = trimmed.strip_prefix("##") {
            total = total
                .checked_add(line.len())
                .ok_or(ReadError::SizeOverflow)?;
            in_data = record
                .split_once('=')
                .is_some_and(|(name, _)| normalize_label(name) == "XYDATA");
        } else if !in_data {
            total = total
                .checked_add(line.len())
                .ok_or(ReadError::SizeOverflow)?;
        }
        if total > limit {
            return Err(ReadError::limit(ReadResource::MetadataBytes, limit, total));
        }
    }
    Ok(())
}

pub(super) fn parameter_ranges(
    text: &str,
    source: crate::provenance::SourceId,
) -> Vec<crate::processed::SourceParameterText> {
    let mut records = Vec::new();
    let mut in_data = false;
    let mut offset = 0;
    let mut start = None;
    for line in text.split_inclusive('\n') {
        let label = line.trim_start().strip_prefix("##");
        let retain = label.is_some() || !in_data;
        if let Some(label) = label {
            in_data = label
                .split_once('=')
                .is_some_and(|(name, _)| normalize_label(name) == "XYDATA");
        }
        if retain {
            start.get_or_insert(offset);
        } else if let Some(begin) = start.take() {
            records.push(crate::processed::SourceParameterText::at_offset(
                source,
                begin,
                text[begin..offset].to_owned(),
            ));
        }
        offset += line.len();
    }
    if let Some(begin) = start {
        records.push(crate::processed::SourceParameterText::at_offset(
            source,
            begin,
            text[begin..].to_owned(),
        ));
    }
    records
}

impl Jcamp {
    pub(super) fn parse(
        control: &mut crate::ExecutionContext<'_>,
        text: &str,
        source: &Path,
        limits: &ReadLimits,
        owned_source_bytes: usize,
    ) -> Result<Self, ReadError> {
        let mut labels = Vec::new();
        let mut map = BTreeMap::new();
        // One borrowed slice per source line is a conservative upper bound for
        // the XYDATA index. Reserve once so growth cannot escape the estimate.
        let line_capacity = text.lines().count();
        let index_bytes = line_capacity
            .checked_mul(std::mem::size_of::<&str>())
            .ok_or(ReadError::SizeOverflow)?;
        enforce_working(
            owned_source_bytes
                .checked_add(index_bytes)
                .ok_or(ReadError::SizeOverflow)?,
            limits.working_bytes(),
        )?;
        let mut data_lines = Vec::new();
        data_lines
            .try_reserve_exact(line_capacity)
            .map_err(|_| ReadError::allocation(index_bytes))?;
        let mut in_data = false;
        let mut ended = false;
        for line in text.lines() {
            control.check_cancelled()?;
            let trimmed = line.trim();
            if ended {
                if trimmed.is_empty() || trimmed.starts_with("$$") {
                    continue;
                }
                return Err(ReadError::corrupt(
                    source.into(),
                    "content follows the JCAMP END record",
                ));
            }
            if let Some(record) = trimmed.strip_prefix("##") {
                let (name, value) = record
                    .split_once('=')
                    .ok_or_else(|| ReadError::corrupt(source.into(), "malformed JCAMP label"))?;
                let name = normalize_label(name);
                let value = value
                    .split_once("$$")
                    .map_or(value, |(value, _)| value)
                    .trim()
                    .to_owned();
                if name == "END" && !value.is_empty() {
                    return Err(ReadError::corrupt(
                        source.into(),
                        "JCAMP END record must not contain a value",
                    ));
                }
                if map.insert(name.clone(), value.clone()).is_some() {
                    return Err(ReadError::corrupt(
                        source.into(),
                        format!("duplicate JCAMP label {name}"),
                    ));
                }
                labels.push((name.clone(), value));
                in_data = name == "XYDATA";
                ended = name == "END";
            } else if in_data && !trimmed.is_empty() && !trimmed.starts_with("$$") {
                let data = trimmed
                    .split_once("$$")
                    .map_or(trimmed, |(data, _)| data)
                    .trim();
                if !data.is_empty() {
                    data_lines.push(data);
                }
            }
        }
        if !ended {
            return Err(ReadError::incomplete(
                source.into(),
                "required JCAMP label END is missing",
            ));
        }
        let required = |name: &str| {
            map.get(name).map(String::as_str).ok_or_else(|| {
                ReadError::incomplete(
                    source.into(),
                    format!("required JCAMP label {name} is missing"),
                )
            })
        };
        // IUPAC Recommendations 1999, example 2, retains NMR XYDATA in 5.01.
        // The version does not authorize any additional data representation.
        if !matches!(required("JCAMP-DX")?, "5.00" | "5.01") {
            return Err(ReadError::unsupported_code(
                source.into(),
                crate::raw::UnsupportedFeatureCode::SPECTRUM_REPRESENTATION,
                "only JCAMP-DX 5.00 and 5.01 NMR XYDATA are supported",
            ));
        }
        let _title = required("TITLE")?;
        if !required("DATA TYPE")?.eq_ignore_ascii_case("NMR SPECTRUM") {
            return Err(ReadError::unsupported_code(
                source.into(),
                crate::raw::UnsupportedFeatureCode::SPECTRUM_REPRESENTATION,
                "JCAMP DATA TYPE is not NMR SPECTRUM",
            ));
        }
        if map.contains_key("BLOCKS")
            || map.contains_key("NTUPLES")
            || map
                .get("DATA CLASS")
                .is_some_and(|value| value.eq_ignore_ascii_case("NTUPLES"))
        {
            return Err(ReadError::unsupported_code(
                source.into(),
                crate::raw::UnsupportedFeatureCode::SPECTRUM_REPRESENTATION,
                "LINK and NTUPLES JCAMP files are not supported",
            ));
        }
        if map
            .get("DATA CLASS")
            .is_some_and(|value| !value.eq_ignore_ascii_case("XYDATA"))
        {
            return Err(ReadError::unsupported_code(
                source.into(),
                crate::raw::UnsupportedFeatureCode::SPECTRUM_REPRESENTATION,
                "only JCAMP DATA CLASS=XYDATA is supported",
            ));
        }
        if ["XYPOINTS", "PEAK TABLE", "PEAKTABLE"]
            .into_iter()
            .any(|label| map.contains_key(label))
            || map
                .get("DATA CLASS")
                .is_some_and(|value| value.to_ascii_uppercase().contains("PEAK"))
        {
            return Err(ReadError::unsupported_code(
                source.into(),
                crate::raw::UnsupportedFeatureCode::SPECTRUM_REPRESENTATION,
                "XYPOINTS and peak-table JCAMP representations are not supported",
            ));
        }
        let xydata = required("XYDATA")?
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .collect::<String>();
        if !xydata.eq_ignore_ascii_case("(X++(Y..Y))") {
            return Err(ReadError::unsupported(
                source.into(),
                "only XYDATA=(X++(Y..Y)) is supported",
            ));
        }
        let unit = match required("XUNITS")?.to_ascii_uppercase().as_str() {
            "HZ" => AxisUnit::Hertz,
            "PPM" => AxisUnit::Ppm,
            _ => {
                return Err(ReadError::unsupported_code(
                    source.into(),
                    crate::raw::UnsupportedFeatureCode::AXIS_UNITS,
                    "JCAMP XYDATA X units must be Hz or ppm",
                ));
            }
        };
        let _y_units = required("YUNITS")?;
        let x_factor = finite(required("XFACTOR")?, source, "XFACTOR")?;
        let y_factor = finite(required("YFACTOR")?, source, "YFACTOR")?;
        if x_factor == 0.0 {
            return Err(invalid(source, "XFACTOR"));
        }
        if y_factor == 0.0 {
            return Err(invalid(source, "YFACTOR"));
        }
        let first_x = finite(required("FIRSTX")?, source, "FIRSTX")?;
        let last_x = finite(required("LASTX")?, source, "LASTX")?;
        let npoints = required("NPOINTS")?
            .parse::<usize>()
            .map_err(|_| invalid(source, "NPOINTS"))?;
        if npoints == 0 {
            return Err(invalid(source, "NPOINTS"));
        }
        let result_bytes = npoints
            .checked_mul(std::mem::size_of::<f64>())
            .ok_or(ReadError::SizeOverflow)?;
        if result_bytes > limits.materialized_bytes() {
            return Err(ReadError::limit(
                ReadResource::MaterializedBytes,
                limits.materialized_bytes(),
                result_bytes,
            ));
        }
        // Coordinates retain their declared units even when optional conversion
        // evidence is absent. Invalid present evidence is never ignored.
        let observe_frequency_mhz = map
            .get(".OBSERVE FREQUENCY")
            .map(|v| finite(v, source, ".OBSERVE FREQUENCY"))
            .transpose()?;
        if observe_frequency_mhz.is_some_and(|v| v <= 0.0) {
            return Err(invalid(source, ".OBSERVE FREQUENCY"));
        }
        let nucleus = map.get(".OBSERVE NUCLEUS").map(|v| {
            v.trim()
                .trim_matches(|c| c == '<' || c == '>')
                .trim()
                .to_owned()
        });
        if nucleus.as_ref().is_some_and(|v| v.is_empty()) {
            return Err(invalid(source, ".OBSERVE NUCLEUS"));
        }
        let delta_x = if npoints == 1 {
            if first_x != last_x {
                return Err(ReadError::corrupt(
                    source.into(),
                    "FIRSTX and LASTX must agree when NPOINTS is one",
                ));
            }
            0.0
        } else {
            (last_x - first_x) / (npoints - 1) as f64
        };
        if npoints > 1 && (!delta_x.is_finite() || delta_x == 0.0) {
            return Err(ReadError::corrupt(
                source.into(),
                "invalid JCAMP axis interval",
            ));
        }
        if let Some(value) = map.get("DELTAX") {
            let declared = finite(value, source, "DELTAX")?;
            let tolerance = f64::EPSILON * 32.0 * declared.abs().max(delta_x.abs()).max(1.0);
            if (declared - delta_x).abs() > tolerance {
                return Err(ReadError::corrupt(
                    source.into(),
                    "DELTAX is inconsistent with FIRSTX, LASTX, and NPOINTS",
                ));
            }
        }
        let samples = decode_xydata(
            control,
            &data_lines,
            first_x,
            delta_x,
            npoints,
            x_factor,
            y_factor,
            source,
        )?;
        Ok(Self {
            x_factor,
            y_factor,
            first_x,
            delta_x,
            unit,
            observe_frequency_mhz,
            nucleus,
            samples,
            labels,
        })
    }
}

pub(super) fn normalize_label(value: &str) -> String {
    value
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

pub(super) fn finite(value: &str, source: &Path, name: &str) -> Result<f64, ReadError> {
    let value = value
        .trim()
        .parse::<f64>()
        .map_err(|_| invalid(source, name))?;
    if !value.is_finite() {
        return Err(invalid(source, name));
    }
    Ok(value)
}

pub(super) fn invalid(source: &Path, name: &str) -> ReadError {
    ReadError::corrupt(
        source.into(),
        format!("JCAMP label {name} has an invalid value"),
    )
}
