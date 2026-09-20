use super::parser::parse_procpar;

use crate::raw::{InputSource, ParameterError};
use std::collections::BTreeMap;

/// Parsed procpar records, retaining the complete original text.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameters {
    pub(super) records: BTreeMap<String, ParameterRecord>,
    pub(super) raw_text: String,
    pub(super) file_header: Option<FileHeader>,
    pub(super) block_headers: Vec<BlockHeader>,
    pub(super) input_source: InputSource,
}
impl Parameters {
    /// Parses a standalone Varian `procpar` file.
    pub fn parse(text: &str) -> Result<Self, ParameterError> {
        parse_procpar(text, InputSource::memory("procpar"))
    }

    /// Returns a typed procpar record.
    pub fn get(&self, name: &str) -> Option<&ParameterRecord> {
        self.records.get(name)
    }
    /// Returns all records in lexical name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ParameterRecord)> {
        self.records
            .iter()
            .map(|(name, record)| (name.as_str(), record))
    }
    /// Returns the unmodified procpar text.
    pub fn raw_text(&self) -> &str {
        &self.raw_text
    }
    /// Return the binary file header associated with these parameters.
    pub fn file_header(&self) -> Option<&FileHeader> {
        self.file_header.as_ref()
    }
    /// Return one common header for each binary data block.
    pub fn block_headers(&self) -> &[BlockHeader] {
        &self.block_headers
    }
}

/// Basic storage type declared by a procpar record.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterType {
    /// IEEE-754 real values serialized as decimal text.
    Real,
    /// Quoted string values.
    String,
}

/// One typed procpar value.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    /// A finite real value.
    Real(f64),
    /// A string value with procpar quoting removed.
    String(String),
}

impl ParameterValue {
    /// Returns the real value when this is [`ParameterValue::Real`].
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Real(value) => Some(*value),
            Self::String(_) => None,
        }
    }

    /// Returns the string value when this is [`ParameterValue::String`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Real(_) => None,
        }
    }
}

/// Typed procpar record and its declared constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterRecord {
    pub(super) subtype: i16,
    pub(super) parameter_type: ParameterType,
    pub(super) maximum: f64,
    pub(super) minimum: f64,
    pub(super) step: f64,
    pub(super) group: i16,
    pub(super) display_group: i16,
    pub(super) protection: i32,
    pub(super) active: bool,
    pub(super) values: Vec<ParameterValue>,
    pub(super) enumerated_values: Vec<ParameterValue>,
}

impl ParameterRecord {
    /// Returns the vendor subtype code.
    pub fn subtype(&self) -> i16 {
        self.subtype
    }

    /// Returns the declared basic storage type.
    pub fn parameter_type(&self) -> ParameterType {
        self.parameter_type
    }

    /// Returns the declared maximum.
    pub fn maximum(&self) -> f64 {
        self.maximum
    }

    /// Returns the declared minimum.
    pub fn minimum(&self) -> f64 {
        self.minimum
    }

    /// Returns the declared increment.
    pub fn step(&self) -> f64 {
        self.step
    }

    /// Returns the vendor group code.
    pub fn group(&self) -> i16 {
        self.group
    }

    /// Returns the vendor display-group code.
    pub fn display_group(&self) -> i16 {
        self.display_group
    }

    /// Returns the vendor protection bit field.
    pub fn protection(&self) -> i32 {
        self.protection
    }

    /// Returns whether the parameter was active.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Returns current values in acquisition order.
    pub fn values(&self) -> &[ParameterValue] {
        &self.values
    }

    /// Returns the declared allowed values.
    pub fn enumerated_values(&self) -> &[ParameterValue] {
        &self.enumerated_values
    }
}
/// Portable projection of a Varian binary file header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileHeader {
    pub(super) raw_bytes: [u8; 32],
    pub(super) nblocks: usize,
    pub(super) ntraces: usize,
    pub(super) stored_values_per_trace: usize,
    pub(super) bytes_per_value: usize,
    pub(super) trace_bytes: usize,
    pub(super) block_bytes: usize,
    pub(super) version_id: u16,
    pub(super) status: u16,
    pub(super) block_headers: usize,
    pub(super) raw_block_header_word: u32,
}
impl FileHeader {
    /// Returns the exact original binary header bytes.
    pub fn raw_bytes(&self) -> &[u8; 32] {
        &self.raw_bytes
    }
    /// Returns the number of binary blocks.
    pub fn nblocks(&self) -> usize {
        self.nblocks
    }
    /// Returns traces stored in each block.
    pub fn ntraces(&self) -> usize {
        self.ntraces
    }
    /// Returns real numeric values stored per trace before complex pairing.
    pub fn stored_values_per_trace(&self) -> usize {
        self.stored_values_per_trace
    }
    /// Returns bytes used by one stored numeric value.
    pub fn bytes_per_value(&self) -> usize {
        self.bytes_per_value
    }
    /// Returns the declared byte length of one trace.
    pub fn trace_bytes(&self) -> usize {
        self.trace_bytes
    }
    /// Returns the declared byte length of one complete block.
    pub fn block_bytes(&self) -> usize {
        self.block_bytes
    }
    /// Returns the raw version, file-type, and vendor bit field.
    pub fn version_id(&self) -> u16 {
        self.version_id
    }
    /// Returns the low six-bit file-format version.
    pub fn version(&self) -> u16 {
        self.version_id & 0x003f
    }
    /// Returns the raw file status bit field.
    pub fn status(&self) -> u16 {
        self.status
    }
    /// Returns the effective number of 28-byte headers per block.
    pub fn block_headers(&self) -> usize {
        self.block_headers
    }
    /// Returns the complete raw `nbheaders` word including dimension flags.
    pub fn raw_block_header_word(&self) -> u32 {
        self.raw_block_header_word
    }
}
/// Common 28-byte Varian block header. Samples are not normalized by `ctcount`.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockHeader {
    pub(super) raw_bytes: [u8; 28],
    pub(super) scale: i16,
    pub(super) status: u16,
    pub(super) index: i16,
    pub(super) mode: u16,
    pub(super) ctcount: i32,
    pub(super) left_phase: f32,
    pub(super) right_phase: f32,
    pub(super) level: f32,
    pub(super) tilt: f32,
}
impl BlockHeader {
    /// Returns the exact original common block header bytes.
    pub fn raw_bytes(&self) -> &[u8; 28] {
        &self.raw_bytes
    }
    /// Returns the signed base-two sample scale exponent.
    pub fn scale(&self) -> i16 {
        self.scale
    }
    /// Returns the raw block status bit field.
    pub fn status(&self) -> u16 {
        self.status
    }
    /// Returns the vendor block index.
    pub fn index(&self) -> i16 {
        self.index
    }
    /// Returns the raw block mode bit field.
    pub fn mode(&self) -> u16 {
        self.mode
    }
    /// Returns the completed transient count without normalizing samples.
    pub fn ctcount(&self) -> i32 {
        self.ctcount
    }
    /// Returns the stored left phase value.
    pub fn left_phase(&self) -> f32 {
        self.left_phase
    }
    /// Returns the stored right phase value.
    pub fn right_phase(&self) -> f32 {
        self.right_phase
    }
    /// Returns the stored baseline level.
    pub fn level(&self) -> f32 {
        self.level
    }
    /// Returns the stored baseline tilt.
    pub fn tilt(&self) -> f32 {
        self.tilt
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl BlockHeader {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &[u8; 28],
        &i16,
        &u16,
        &i16,
        &u16,
        &i32,
        &f32,
        &f32,
        &f32,
        &f32,
    ) {
        (
            &self.raw_bytes,
            &self.scale,
            &self.status,
            &self.index,
            &self.mode,
            &self.ctcount,
            &self.left_phase,
            &self.right_phase,
            &self.level,
            &self.tilt,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: ([u8; 28], i16, u16, i16, u16, i32, f32, f32, f32, f32),
    ) -> Result<Self, crate::internal::ModelError> {
        let (raw_bytes, scale, status, index, mode, ctcount, left_phase, right_phase, level, tilt) =
            parts;
        let value = Self {
            raw_bytes,
            scale,
            status,
            index,
            mode,
            ctcount,
            left_phase,
            right_phase,
            level,
            tilt,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl FileHeader {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &[u8; 32],
        &usize,
        &usize,
        &usize,
        &usize,
        &usize,
        &usize,
        &u16,
        &u16,
        &usize,
        &u32,
    ) {
        (
            &self.raw_bytes,
            &self.nblocks,
            &self.ntraces,
            &self.stored_values_per_trace,
            &self.bytes_per_value,
            &self.trace_bytes,
            &self.block_bytes,
            &self.version_id,
            &self.status,
            &self.block_headers,
            &self.raw_block_header_word,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            [u8; 32],
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            u16,
            u16,
            usize,
            u32,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            raw_bytes,
            nblocks,
            ntraces,
            stored_values_per_trace,
            bytes_per_value,
            trace_bytes,
            block_bytes,
            version_id,
            status,
            block_headers,
            raw_block_header_word,
        ) = parts;
        let value = Self {
            raw_bytes,
            nblocks,
            ntraces,
            stored_values_per_trace,
            bytes_per_value,
            trace_bytes,
            block_bytes,
            version_id,
            status,
            block_headers,
            raw_block_header_word,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ParameterRecord {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &i16,
        &ParameterType,
        &f64,
        &f64,
        &f64,
        &i16,
        &i16,
        &i32,
        &bool,
        &Vec<ParameterValue>,
        &Vec<ParameterValue>,
    ) {
        (
            &self.subtype,
            &self.parameter_type,
            &self.maximum,
            &self.minimum,
            &self.step,
            &self.group,
            &self.display_group,
            &self.protection,
            &self.active,
            &self.values,
            &self.enumerated_values,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            i16,
            ParameterType,
            f64,
            f64,
            f64,
            i16,
            i16,
            i32,
            bool,
            Vec<ParameterValue>,
            Vec<ParameterValue>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            subtype,
            parameter_type,
            maximum,
            minimum,
            step,
            group,
            display_group,
            protection,
            active,
            values,
            enumerated_values,
        ) = parts;
        let value = Self {
            subtype,
            parameter_type,
            maximum,
            minimum,
            step,
            group,
            display_group,
            protection,
            active,
            values,
            enumerated_values,
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
        &BTreeMap<String, ParameterRecord>,
        &String,
        &Option<FileHeader>,
        &Vec<BlockHeader>,
        &InputSource,
    ) {
        (
            &self.records,
            &self.raw_text,
            &self.file_header,
            &self.block_headers,
            &self.input_source,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            BTreeMap<String, ParameterRecord>,
            String,
            Option<FileHeader>,
            Vec<BlockHeader>,
            InputSource,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (records, raw_text, file_header, block_headers, input_source) = parts;
        let value = Self {
            records,
            raw_text,
            file_header,
            block_headers,
            input_source,
        };

        Ok(value)
    }
}
