use crate::{AxisUnit, Complex64};
use std::collections::BTreeMap;

/// Raw JEOL axis unit descriptor retained in vendor metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawAxisUnit {
    pub(super) prefix_exponent: i8,
    pub(super) power: u8,
    pub(super) base_code: u8,
}
impl RawAxisUnit {
    /// Returns the decimal SI prefix exponent encoded by the high nibble.
    pub fn prefix_exponent(&self) -> i8 {
        self.prefix_exponent
    }
    /// Returns the unit power encoded by the low nibble.
    pub fn power(&self) -> u8 {
        self.power
    }
    /// Returns the vendor base-unit code.
    pub fn base_code(&self) -> u8 {
        self.base_code
    }
}

/// JEOL metadata with raw record areas retained losslessly.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameters {
    pub(super) raw_header: Vec<u8>,
    pub(super) raw_pre_data_records: Vec<u8>,
    pub(super) raw_trailing_records: Vec<u8>,
    pub(super) embedded_axes: Vec<EmbeddedAxisEvidence>,
    pub(super) axis_units: Vec<RawAxisUnit>,
    pub(super) axis_types: Vec<u8>,
    pub(super) sample_transform: SampleTransform,
    pub(super) values: BTreeMap<String, ParameterRecord>,
}
impl Parameters {
    /// Explicit list/ramp declarations interpreted from retained record areas.
    /// Header-only axes have no entry. This interpretation remains experimental.
    pub fn embedded_axes(&self) -> &[EmbeddedAxisEvidence] {
        &self.embedded_axes
    }
    /// Returns the lossless 1,360-byte fixed JDF header.
    pub fn raw_header(&self) -> &[u8] {
        &self.raw_header
    }
    /// Returns uninterpreted records between the fixed header and sample payload.
    pub fn raw_pre_data_records(&self) -> &[u8] {
        &self.raw_pre_data_records
    }
    /// Returns uninterpreted records after the sample payload.
    pub fn raw_trailing_records(&self) -> &[u8] {
        &self.raw_trailing_records
    }
    /// Returns raw unit descriptors for active disk axes.
    pub fn axis_units(&self) -> &[RawAxisUnit] {
        &self.axis_units
    }
    /// Returns raw JEOL axis-type codes for active disk axes.
    pub fn axis_types(&self) -> &[u8] {
        &self.axis_types
    }
    /// Returns sign and component-lane transformations applied by the adapter.
    pub fn sample_transform(&self) -> &SampleTransform {
        &self.sample_transform
    }
    /// Returns a typed parameter by case-insensitive name.
    pub fn get(&self, name: &str) -> Option<&ParameterRecord> {
        self.values.get(&name.to_ascii_lowercase())
    }
    /// Returns all typed parameters keyed by normalized lowercase name.
    pub fn values(&self) -> &BTreeMap<String, ParameterRecord> {
        &self.values
    }
}

/// Location of an interpreted textual parameter-axis declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedRecordArea {
    /// Between the fixed header and the sample payload.
    BeforeData,
    /// After the sample payload.
    AfterData,
}

/// Syntax supplying explicit parameter coordinates; never inferred from samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddedAxisKind {
    /// Ordered list; individual entries may use different compatible SI prefixes.
    List,
    /// Inclusive start/stop and approximate increment; count must match the header.
    Ramp,
}

/// Experimental text evidence for one parameter axis, normalized to SI.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddedAxisEvidence {
    pub(super) disk_axis: usize,
    pub(super) parameter: String,
    pub(super) unit: AxisUnit,
    pub(super) values: Vec<f64>,
    pub(super) area: EmbeddedRecordArea,
    pub(super) kind: EmbeddedAxisKind,
}
impl EmbeddedAxisEvidence {
    /// Zero-based JEOL disk axis (x=0, y=1).
    pub fn disk_axis(&self) -> usize {
        self.disk_axis
    }
    /// Parameter name before the arrow.
    pub fn parameter(&self) -> &str {
        &self.parameter
    }
    /// Canonical SI unit.
    pub fn unit(&self) -> AxisUnit {
        self.unit
    }
    /// Complete declared values, before the valid-range selection.
    pub fn values(&self) -> &[f64] {
        &self.values
    }
    /// Retained source record area.
    pub fn area(&self) -> EmbeddedRecordArea {
        self.area
    }
    /// List or ramp syntax.
    pub fn kind(&self) -> EmbeddedAxisKind {
        self.kind
    }
}

/// One typed JEOL parameter record with its raw unit descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterRecord {
    pub(super) class: [u8; 4],
    pub(super) unit_scaler: i16,
    pub(super) raw_units: [u8; 10],
    pub(super) value: ParameterValue,
    pub(super) name: String,
}
impl ParameterRecord {
    /// Returns the original parameter name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the four raw vendor class bytes.
    pub fn class(&self) -> [u8; 4] {
        self.class
    }
    /// Returns the signed decimal scale applied to numeric values.
    pub fn unit_scaler(&self) -> i16 {
        self.unit_scaler
    }
    /// Returns five packed raw unit descriptors.
    pub fn raw_units(&self) -> &[u8; 10] {
        &self.raw_units
    }
    /// Returns the unscaled typed value.
    pub fn value(&self) -> &ParameterValue {
        &self.value
    }
    /// Returns a finite scaled numeric value for integer and float records.
    pub fn scaled_f64(&self) -> Option<f64> {
        let value = match self.value {
            ParameterValue::Integer(value) => value as f64,
            ParameterValue::Float(value) => value,
            _ => return None,
        };
        let scaled = value * 10f64.powi(self.unit_scaler as i32);
        scaled.is_finite().then_some(scaled)
    }
    /// Returns the first packed unit descriptor.
    pub fn primary_unit(&self) -> RawAxisUnit {
        decode_raw_unit(self.raw_units[0], self.raw_units[1])
    }
}

/// Typed value stored in a JEOL parameter record.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    /// Fixed-width UTF-8 string.
    String(String),
    /// Signed 32-bit integer.
    Integer(i32),
    /// Finite 64-bit floating-point value.
    Float(f64),
    /// Finite complex value.
    Complex(Complex64),
    /// Vendor infinity sentinel payload.
    Infinity(i32),
    /// Unrecognized value type retained losslessly.
    Unknown {
        /// Raw vendor value-type code.
        type_code: i32,
        /// Raw fixed-width value bytes.
        bytes: [u8; 16],
    },
}

/// Vendor-to-public complex sign transformation applied by the adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleTransform {
    pub(super) direct_imaginary_multiplier: i8,
    pub(super) indirect_lane_multipliers: Vec<i8>,
}
impl SampleTransform {
    /// Returns the multiplier applied to the direct imaginary section.
    pub fn direct_imaginary_multiplier(&self) -> i8 {
        self.direct_imaginary_multiplier
    }
    /// Returns section-pair signs, before any declared P/N-to-Cartesian mixing.
    /// For pn_type=y the reader subsequently maps A,B to (A-B)/2,(A+B)/(2i);
    /// the descriptor's normalization evidence records that conversion.
    pub fn indirect_lane_multipliers(&self) -> &[i8] {
        &self.indirect_lane_multipliers
    }
}

pub(crate) fn decode_raw_unit(scaler: u8, base: u8) -> RawAxisUnit {
    let prefix = (scaler >> 4) as i8;
    let prefix_exponent = if prefix < 8 {
        -3 * prefix
    } else {
        -3 * (prefix - 16)
    };
    RawAxisUnit {
        prefix_exponent,
        power: scaler & 0x0f,
        base_code: base,
    }
}

pub(crate) fn decode_fixed_string(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    let value = String::from_utf8_lossy(&bytes[..end]).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl EmbeddedAxisEvidence {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &usize,
        &String,
        &AxisUnit,
        &Vec<f64>,
        &EmbeddedRecordArea,
        &EmbeddedAxisKind,
    ) {
        (
            &self.disk_axis,
            &self.parameter,
            &self.unit,
            &self.values,
            &self.area,
            &self.kind,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            usize,
            String,
            AxisUnit,
            Vec<f64>,
            EmbeddedRecordArea,
            EmbeddedAxisKind,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (disk_axis, parameter, unit, values, area, kind) = parts;
        let value = Self {
            disk_axis,
            parameter,
            unit,
            values,
            area,
            kind,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ParameterRecord {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&[u8; 4], &i16, &[u8; 10], &ParameterValue, &String) {
        (
            &self.class,
            &self.unit_scaler,
            &self.raw_units,
            &self.value,
            &self.name,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: ([u8; 4], i16, [u8; 10], ParameterValue, String),
    ) -> Result<Self, crate::internal::ModelError> {
        let (class, unit_scaler, raw_units, value, name) = parts;
        let value = Self {
            class,
            unit_scaler,
            raw_units,
            value,
            name,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl Parameters {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Vec<u8>,
        &Vec<u8>,
        &Vec<u8>,
        &Vec<EmbeddedAxisEvidence>,
        &Vec<RawAxisUnit>,
        &Vec<u8>,
        &SampleTransform,
        &BTreeMap<String, ParameterRecord>,
    ) {
        (
            &self.raw_header,
            &self.raw_pre_data_records,
            &self.raw_trailing_records,
            &self.embedded_axes,
            &self.axis_units,
            &self.axis_types,
            &self.sample_transform,
            &self.values,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
            Vec<EmbeddedAxisEvidence>,
            Vec<RawAxisUnit>,
            Vec<u8>,
            SampleTransform,
            BTreeMap<String, ParameterRecord>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            raw_header,
            raw_pre_data_records,
            raw_trailing_records,
            embedded_axes,
            axis_units,
            axis_types,
            sample_transform,
            values,
        ) = parts;
        let value = Self {
            raw_header,
            raw_pre_data_records,
            raw_trailing_records,
            embedded_axes,
            axis_units,
            axis_types,
            sample_transform,
            values,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawAxisUnit {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&i8, &u8, &u8) {
        (&self.prefix_exponent, &self.power, &self.base_code)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (i8, u8, u8),
    ) -> Result<Self, crate::internal::ModelError> {
        let (prefix_exponent, power, base_code) = parts;
        let value = Self {
            prefix_exponent,
            power,
            base_code,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SampleTransform {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&i8, &Vec<i8>) {
        (
            &self.direct_imaginary_multiplier,
            &self.indirect_lane_multipliers,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (i8, Vec<i8>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (direct_imaginary_multiplier, indirect_lane_multipliers) = parts;
        let value = Self {
            direct_imaginary_multiplier,
            indirect_lane_multipliers,
        };

        Ok(value)
    }
}
