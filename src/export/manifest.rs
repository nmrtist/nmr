use super::*;

/// Stable schema identifier stored in `manifest.npy`.
pub const PLOT_DATA_SCHEMA_ID: &str = "org.nmrtist.plot-data";
/// Stable schema version stored in `manifest.npy`.
pub const PLOT_DATA_SCHEMA_VERSION: u32 = 1;
/// Absolute V1 export limit for the complete ZIP archive.
pub const MAX_EXPORT_BYTES: u64 = 512 * 1024 * 1024;

pub(super) fn manifest_json(plot: &PlotData) -> Vec<u8> {
    let mut json = String::new();
    json.push_str("{\"schema_id\":\"");
    json.push_str(PLOT_DATA_SCHEMA_ID);
    json.push_str("\",\"version\":");
    json.push_str(&PLOT_DATA_SCHEMA_VERSION.to_string());
    json.push_str(",\"order\":\"C\",\"shape\":[");
    for (index, extent) in plot.shape().iter().enumerate() {
        if index != 0 {
            json.push(',');
        }
        json.push_str(&extent.to_string());
    }
    json.push_str("],\"axes\":[");
    for (index, axis) in plot.axes().iter().enumerate() {
        if index != 0 {
            json.push(',');
        }
        json.push_str("{\"member\":\"axis_");
        json.push_str(&index.to_string());
        json.push_str(".npy\",\"label\":");
        match axis.label() {
            Some(label) => push_json_string(&mut json, label),
            None => json.push_str("null"),
        }
        json.push_str(",\"role\":\"");
        json.push_str(axis_role(axis.role()));
        json.push_str("\",\"coordinates\":\"");
        json.push_str(match axis.coordinates() {
            PlotCoordinates::Physical { .. } => "Physical",
            PlotCoordinates::LogicalIndex(_) => "LogicalIndex",
        });
        json.push_str("\",\"unit\":");
        match axis.coordinates() {
            PlotCoordinates::Physical {
                unit: Some(unit), ..
            } => {
                push_json_string(&mut json, axis_unit(*unit));
            }
            PlotCoordinates::Physical { unit: None, .. } | PlotCoordinates::LogicalIndex(_) => {
                json.push_str("null");
            }
        }
        json.push_str(",\"direction\":\"");
        json.push_str(axis_direction(axis.direction()));
        json.push_str("\",\"source_quality\":\"");
        json.push_str(match axis.source_quality() {
            CoordinateSourceQuality::Established => "Established",
            CoordinateSourceQuality::Unknown => "Unknown",
        });
        json.push_str("\"}");
    }
    let records = plot
        .provenance()
        .history()
        .map_or(0, |history| history.records().len());
    json.push_str("],\"processing_records\":");
    json.push_str(&records.to_string());
    json.push('}');
    json.into_bytes()
}

pub(super) fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value < '\u{20}' => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{:04x}", value as u32);
            }
            value => output.push(value),
        }
    }
    output.push('"');
}

pub(super) fn axis_role(role: AxisRole) -> &'static str {
    match role {
        AxisRole::DirectAcquisition => "DirectAcquisition",
        AxisRole::IndirectAcquisition => "IndirectAcquisition",
        AxisRole::ArrayParameter => "ArrayParameter",
        AxisRole::Signal => "Signal",
        AxisRole::Unknown => "Unknown",
    }
}

pub(super) fn axis_unit(unit: AxisUnit) -> &'static str {
    match unit {
        AxisUnit::Second => "Second",
        AxisUnit::Hertz => "Hertz",
        AxisUnit::Ppm => "Ppm",
        AxisUnit::Tesla => "Tesla",
        AxisUnit::TeslaPerMeter => "TeslaPerMeter",
    }
}

pub(super) fn axis_direction(direction: AxisDirection) -> &'static str {
    match direction {
        AxisDirection::Ascending => "Ascending",
        AxisDirection::Descending => "Descending",
        AxisDirection::Unknown => "Unknown",
    }
}
