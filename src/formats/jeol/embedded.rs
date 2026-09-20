//! Restricted, explicit parameter declarations in record areas. Never scan samples.

use super::header::JdfHeader;
use super::parameters::EmbeddedAxisEvidence;
use super::parameters::EmbeddedAxisKind;
use super::parameters::EmbeddedRecordArea;
use crate::AxisUnit;
use crate::ReadError;
use crate::raw::ReadLimits;

pub(super) fn parse(
    header: &JdfHeader,
    before: &[u8],
    after: &[u8],
    limits: &ReadLimits,
    existing_bytes: usize,
) -> Result<Vec<EmbeddedAxisEvidence>, ReadError> {
    let mut result = Vec::new();
    let mut bytes_used = existing_bytes;
    for (bytes, area) in [
        (before, EmbeddedRecordArea::BeforeData),
        (after, EmbeddedRecordArea::AfterData),
    ] {
        for run in bytes.split(|b| !b.is_ascii() || (*b < 32 && !b.is_ascii_whitespace())) {
            let text = std::str::from_utf8(run).expect("ASCII run");
            for (arrow, _) in text.match_indices("=>") {
                let name = text[..arrow].split_whitespace().last().unwrap_or("");
                let rest = text[arrow + 2..].trim_start();
                let target = rest.split_whitespace().next().unwrap_or("");
                let Some(disk_axis) = ["x_acq", "y_acq", "z_acq", "a_acq"]
                    .iter()
                    .position(|v| target.eq_ignore_ascii_case(v))
                else {
                    continue;
                };
                if disk_axis >= header.axis_types.len()
                    || header.axis_types[disk_axis] != 1
                    || disk_axis == 0
                {
                    continue;
                }
                let invalid = || {
                    ReadError::invalid_metadata(
                        header.input_source.clone(),
                        Some(target),
                        "invalid, conflicting or unsupported JEOL parameter-axis declaration",
                    )
                };
                if name.is_empty()
                    || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    || result
                        .iter()
                        .any(|v: &EmbeddedAxisEvidence| v.disk_axis == disk_axis)
                {
                    return Err(invalid());
                }
                let body = rest[target.len()..].trim_start();
                let count = header.points_disk[disk_axis];
                bytes_used = bytes_used
                    .checked_add(count.checked_mul(16).ok_or(ReadError::SizeOverflow)?)
                    .and_then(|v| {
                        v.checked_add(name.len() + std::mem::size_of::<EmbeddedAxisEvidence>())
                    })
                    .ok_or(ReadError::SizeOverflow)?;
                for (resource, limit) in [
                    (
                        crate::raw::ReadResource::MetadataBytes,
                        limits.metadata_bytes(),
                    ),
                    (
                        crate::raw::ReadResource::WorkingBytes,
                        limits.working_bytes(),
                    ),
                ] {
                    if bytes_used > limit {
                        return Err(ReadError::limit(resource, limit, bytes_used));
                    }
                }
                let (unit, values, kind) = if let Some(body) = body.strip_prefix('{') {
                    let end = body.find('}').ok_or_else(invalid)?;
                    let mut values = Vec::new();
                    let mut unit = None;
                    for token in body[..end].split(',') {
                        if values.len() >= count {
                            return Err(invalid());
                        }
                        let (value, current) = scalar(token).ok_or_else(invalid)?;
                        if unit.is_some_and(|u| u != current) {
                            return Err(invalid());
                        }
                        unit = Some(current);
                        values.push(value);
                    }
                    // Storage may contain tile padding beyond the declared
                    // acquisition (e.g. 18 gradient values in 20 stored rows).
                    if values.len() <= header.offset_stop[disk_axis] {
                        return Err(invalid());
                    }
                    (unit.ok_or_else(invalid)?, values, EmbeddedAxisKind::List)
                } else {
                    // A comma starts declaration attributes (range constraints,
                    // help text), not part of the increment's scalar value.
                    let body = body.split([',', '\r', '\n', ';']).next().unwrap_or("");
                    let (start, tail) = body.split_once("..").ok_or_else(invalid)?;
                    let (stop, step) = tail.split_once(':').ok_or_else(invalid)?;
                    let (start, u) = scalar(start).ok_or_else(invalid)?;
                    let (stop, v) = scalar(stop).ok_or_else(invalid)?;
                    let (step, w) = scalar(step).ok_or_else(invalid)?;
                    if u != v || u != w || step == 0.0 {
                        return Err(invalid());
                    }
                    let intervals = (stop - start) / step;
                    if !intervals.is_finite()
                        || intervals.round() < 1.0
                        || (intervals - intervals.round()).abs() > 1e-4
                        || intervals.round() >= count as f64
                        || intervals.round() < header.offset_stop[disk_axis] as f64
                    {
                        return Err(invalid());
                    }
                    let count = intervals.round() as usize + 1;
                    // JEOL's binary ramp uses the stated increment, including its
                    // printed rounding. The stop bounds the ramp; redistributing
                    // that rounding across points would change acquired values.
                    let values = (0..count).map(|i| start + step * i as f64).collect();
                    (u, values, EmbeddedAxisKind::Ramp)
                };
                result.push(EmbeddedAxisEvidence {
                    disk_axis,
                    parameter: name.to_owned(),
                    unit,
                    values,
                    area,
                    kind,
                });
            }
        }
    }
    Ok(result)
}

fn scalar(text: &str) -> Option<(f64, AxisUnit)> {
    let (number, unit) = text.trim().split_once('[')?;
    let unit = unit.strip_suffix(']')?;
    let (scale, canonical) = match unit {
        "s" => (1.0, AxisUnit::Second),
        "ms" => (1e-3, AxisUnit::Second),
        "us" => (1e-6, AxisUnit::Second),
        "ns" => (1e-9, AxisUnit::Second),
        "T/m" => (1.0, AxisUnit::TeslaPerMeter),
        "mT/m" => (1e-3, AxisUnit::TeslaPerMeter),
        "G/cm" => (0.01, AxisUnit::TeslaPerMeter),
        _ => return None,
    };
    let value = number.trim().parse::<f64>().ok()? * scale;
    value.is_finite().then_some((value, canonical))
}
