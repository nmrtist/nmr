use super::parameters::RawAxisUnit;
use super::parameters::decode_fixed_string;
use super::parameters::decode_raw_unit;
use super::semantics::jeol_complex_dims;
use crate::AxisUnit;
use crate::Domain;
use crate::ReadError;
use crate::raw::InputSource;

pub(crate) const HEADER_LEN: usize = 1360;

pub(crate) mod header_offset {
    pub const ENDIAN: usize = 8;
    pub const DIMENSION: usize = 12;
    pub const AXIS_TYPE: usize = 24;
    pub const AXIS_UNIT: usize = 32;
    pub const DATA_POINTS: usize = 176;
    pub const OFFSET_START: usize = 208;
    pub const OFFSET_STOP: usize = 240;
    pub const AXIS_START: usize = 272;
    pub const AXIS_STOP: usize = 336;
    pub const TITLE: usize = 48;
    pub const AXIS_TITLES: usize = 808;
    pub const BASE_FREQUENCY: usize = 1064;
    pub const PARAM_START: usize = 1212;
    pub const PARAM_LENGTH: usize = 1216;
    pub const LIST_START: usize = 1220;
    pub const LIST_LENGTH: usize = 1252;
    pub const DATA_START: usize = 1284;
    pub const DATA_LENGTH: usize = 1288;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DataFormat {
    OneD,
    TwoD,
    ThreeD,
    FourD,
    SmallTwoD,
    SmallThreeD,
    SmallFourD,
}

impl DataFormat {
    pub(super) fn decode(code: u8, source: &InputSource) -> Result<Self, ReadError> {
        match code {
            1 => Ok(Self::OneD),
            2 => Ok(Self::TwoD),
            3 => Ok(Self::ThreeD),
            4 => Ok(Self::FourD),
            12 => Ok(Self::SmallTwoD),
            13 => Ok(Self::SmallThreeD),
            14 => Ok(Self::SmallFourD),
            _ => Err(ReadError::unsupported_code(
                source.clone(),
                crate::raw::UnsupportedFeatureCode::NUMERIC_ENCODING,
                format!("unknown JEOL data format {code}"),
            )),
        }
    }

    pub(super) fn dimensions(self) -> usize {
        match self {
            Self::OneD => 1,
            Self::TwoD | Self::SmallTwoD => 2,
            Self::ThreeD | Self::SmallThreeD => 3,
            Self::FourD | Self::SmallFourD => 4,
        }
    }

    pub(super) fn submatrix_edge(self) -> usize {
        match self {
            Self::OneD => 8,
            Self::TwoD => 32,
            Self::ThreeD | Self::FourD => 8,
            Self::SmallTwoD | Self::SmallThreeD | Self::SmallFourD => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Precision {
    F32,
    F64,
}

impl Precision {
    pub(super) fn decode(code: u8, source: &InputSource) -> Result<Self, ReadError> {
        match code {
            0 => Ok(Self::F64),
            1 => Ok(Self::F32),
            _ => Err(ReadError::unsupported_code(
                source.clone(),
                crate::raw::UnsupportedFeatureCode::NUMERIC_ENCODING,
                format!("unknown JEOL precision code {code}"),
            )),
        }
    }

    pub(super) fn size(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BodyEndian {
    Big,
    Little,
}

impl AxisUnit {
    pub(super) fn decode(base: u8) -> Option<Self> {
        match base {
            13 => Some(Self::Hertz),
            26 => Some(Self::Ppm),
            28 => Some(Self::Second),
            31 => Some(Self::Tesla),
            _ => None,
        }
    }

    pub(super) fn domain(self) -> Domain {
        match self {
            Self::Second => Domain::Time,
            Self::Hertz | Self::Ppm => Domain::Frequency,
            Self::Tesla | Self::TeslaPerMeter => Domain::Parameter,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct JdfHeader {
    pub(super) input_source: InputSource,
    pub(super) body_endian: BodyEndian,
    pub(super) precision: Precision,
    pub(super) data_format: DataFormat,
    pub(super) axis_types: Vec<u8>,
    pub(super) axis_units: Vec<Option<AxisUnit>>,
    pub(super) raw_axis_units: Vec<RawAxisUnit>,
    pub(super) points_disk: Vec<usize>,
    pub(super) offset_start: Vec<usize>,
    pub(super) offset_stop: Vec<usize>,
    pub(super) axis_start: Vec<f64>,
    pub(super) axis_stop: Vec<f64>,
    pub(super) data_start: usize,
    pub(super) data_length: usize,
    pub(super) title: Option<String>,
    pub(super) axis_titles: Vec<Option<String>>,
    pub(super) base_frequency: Vec<f64>,
    pub(super) param_start: usize,
    pub(super) param_length: usize,
    pub(super) list_start: Vec<usize>,
    pub(super) list_length: Vec<usize>,
}

impl JdfHeader {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, ReadError> {
        Self::parse_for_source(bytes, bytes.len(), InputSource::memory("jdf"))
    }

    pub(super) fn parse_for_source(
        bytes: &[u8],
        source_len: usize,
        input_source: InputSource,
    ) -> Result<Self, ReadError> {
        if bytes.len() < HEADER_LEN {
            return Err(ReadError::truncated(
                input_source.clone(),
                HEADER_LEN,
                bytes.len(),
            ));
        }
        if &bytes[..8] != b"JEOL.NMR" {
            return Err(ReadError::unrecognized(
                input_source,
                "missing JEOL.NMR magic",
            ));
        }
        let body_endian = match bytes[header_offset::ENDIAN] {
            0 => BodyEndian::Big,
            1 => BodyEndian::Little,
            marker => {
                return Err(ReadError::corrupt(
                    input_source,
                    format!("invalid JEOL body endian marker {marker}"),
                ));
            }
        };
        let ndim = bytes[header_offset::DIMENSION] as usize;
        if !(1..=4).contains(&ndim) {
            return Err(ReadError::unsupported_code(
                input_source,
                crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
                format!("JEOL dimension {ndim}"),
            ));
        }
        let format_info = bytes[14];
        let dimension_mask = bytes[13];
        if dimension_mask != 0 && dimension_mask.count_ones() as usize != ndim {
            return Err(ReadError::corrupt(
                input_source,
                format!("JEOL dimension-existence mask does not contain {ndim} active axes"),
            ));
        }
        let precision = Precision::decode(format_info >> 6, &input_source)?;
        let data_format = DataFormat::decode(format_info & 0x3f, &input_source)?;
        if data_format.dimensions() != ndim {
            return Err(ReadError::corrupt(
                input_source,
                format!(
                    "JEOL data format has {} dimensions, header declares {ndim}",
                    data_format.dimensions()
                ),
            ));
        }
        let axis_types = bytes[header_offset::AXIS_TYPE..header_offset::AXIS_TYPE + ndim].to_vec();
        for (axis, &axis_type) in axis_types.iter().enumerate() {
            if !matches!(axis_type, 1..=5) {
                return Err(ReadError::unsupported_code(
                    input_source,
                    crate::raw::UnsupportedFeatureCode::COMPONENT_LAYOUT,
                    format!("unknown JEOL axis type {axis_type} on axis {}", axis + 1),
                ));
            }
        }
        let mut axis_units = Vec::with_capacity(ndim);
        let mut raw_axis_units = Vec::with_capacity(ndim);
        for axis in 0..ndim {
            let (unit, raw) = decode_axis_unit(bytes, axis, &input_source)?;
            axis_units.push(unit);
            raw_axis_units.push(raw);
        }
        let read_u32 = |offset: usize| -> Result<usize, ReadError> {
            let end = offset.checked_add(4).ok_or(ReadError::SizeOverflow)?;
            let value = bytes
                .get(offset..end)
                .ok_or_else(|| ReadError::truncated(input_source.clone(), end, bytes.len()))?;
            Ok(u32::from_be_bytes(value.try_into().unwrap()) as usize)
        };
        let read_u64 = |offset: usize| -> Result<usize, ReadError> {
            let end = offset.checked_add(8).ok_or(ReadError::SizeOverflow)?;
            let value = bytes
                .get(offset..end)
                .ok_or_else(|| ReadError::truncated(input_source.clone(), end, bytes.len()))?;
            usize::try_from(u64::from_be_bytes(value.try_into().unwrap()))
                .map_err(|_| ReadError::SizeOverflow)
        };
        let read_f64 = |offset: usize| -> Result<f64, ReadError> {
            let end = offset.checked_add(8).ok_or(ReadError::SizeOverflow)?;
            let value = bytes
                .get(offset..end)
                .ok_or_else(|| ReadError::truncated(input_source.clone(), end, bytes.len()))?;
            Ok(f64::from_be_bytes(value.try_into().unwrap()))
        };
        let mut points_disk = Vec::with_capacity(ndim);
        let mut offset_start = Vec::with_capacity(ndim);
        let mut offset_stop = Vec::with_capacity(ndim);
        let mut axis_start = Vec::with_capacity(ndim);
        let mut axis_stop = Vec::with_capacity(ndim);
        for axis in 0..ndim {
            let points = read_u32(header_offset::DATA_POINTS + axis * 4)?;
            if points == 0 {
                return Err(ReadError::corrupt(
                    input_source.clone(),
                    format!("JEOL axis {} has zero points", axis + 1),
                ));
            }
            points_disk.push(points);
            let start = read_u32(header_offset::OFFSET_START + axis * 4)?;
            let stop = read_u32(header_offset::OFFSET_STOP + axis * 4)?;
            if start > stop || stop >= points {
                return Err(ReadError::corrupt(
                    input_source.clone(),
                    format!(
                        "JEOL axis {} has invalid valid window {start}..={stop} for {points} points",
                        axis + 1
                    ),
                ));
            }
            offset_start.push(start);
            offset_stop.push(stop);
            axis_start.push(read_f64(header_offset::AXIS_START + axis * 8)?);
            axis_stop.push(read_f64(header_offset::AXIS_STOP + axis * 8)?);
        }
        let data_start = read_u32(header_offset::DATA_START)?;
        if data_start < HEADER_LEN {
            return Err(ReadError::corrupt(
                input_source.clone(),
                "JEOL data start precedes the fixed header",
            ));
        }
        let data_length = read_u64(header_offset::DATA_LENGTH)?;
        if data_length > 0
            && data_start
                .checked_add(data_length)
                .is_none_or(|end| end > source_len)
        {
            return Err(ReadError::truncated(
                input_source.clone(),
                data_start
                    .checked_add(data_length)
                    .ok_or(ReadError::SizeOverflow)?,
                source_len,
            ));
        }
        let title = decode_fixed_string(&bytes[header_offset::TITLE..header_offset::TITLE + 124]);
        let axis_titles = (0..ndim)
            .map(|axis| {
                let start = header_offset::AXIS_TITLES + axis * 32;
                decode_fixed_string(&bytes[start..start + 32])
            })
            .collect();
        let base_frequency = (0..ndim)
            .map(|axis| read_f64(header_offset::BASE_FREQUENCY + axis * 8))
            .collect::<Result<Vec<_>, _>>()?;
        let param_start = read_u32(header_offset::PARAM_START)?;
        let param_length = read_u32(header_offset::PARAM_LENGTH)?;
        if param_length > 0
            && (param_start < HEADER_LEN
                || param_start
                    .checked_add(param_length)
                    .is_none_or(|end| end > source_len))
        {
            return Err(ReadError::truncated(
                input_source.clone(),
                param_start
                    .checked_add(param_length)
                    .ok_or(ReadError::SizeOverflow)?,
                source_len,
            ));
        }
        let list_start = (0..ndim)
            .map(|axis| read_u32(header_offset::LIST_START + axis * 4))
            .collect::<Result<Vec<_>, _>>()?;
        let list_length = (0..ndim)
            .map(|axis| read_u32(header_offset::LIST_LENGTH + axis * 4))
            .collect::<Result<Vec<_>, _>>()?;
        for axis in 0..ndim {
            let start = list_start[axis];
            let length = list_length[axis];
            if length != 0
                && (length % 8 != 0
                    || start < HEADER_LEN
                    || start.checked_add(length).is_none_or(|end| end > source_len))
            {
                return Err(ReadError::corrupt(
                    input_source.clone(),
                    format!("JEOL axis {} has an invalid coordinate list", axis + 1),
                ));
            }
        }
        Ok(Self {
            input_source,
            body_endian,
            precision,
            data_format,
            axis_types,
            axis_units,
            raw_axis_units,
            points_disk,
            offset_start,
            offset_stop,
            axis_start,
            axis_stop,
            data_start,
            data_length,
            title,
            axis_titles,
            base_frequency,
            param_start,
            param_length,
            list_start,
            list_length,
        })
    }

    pub(super) fn sections(&self) -> Result<usize, ReadError> {
        jeol_complex_dims(&self.axis_types, &self.input_source).and_then(|complex_dims| {
            1usize
                .checked_shl(complex_dims as u32)
                .ok_or(ReadError::SizeOverflow)
        })
    }
}

pub(crate) fn decode_axis_unit(
    bytes: &[u8],
    axis: usize,
    source: &InputSource,
) -> Result<(Option<AxisUnit>, RawAxisUnit), ReadError> {
    let offset = header_offset::AXIS_UNIT + axis * 2;
    let scaler = *bytes
        .get(offset)
        .ok_or_else(|| ReadError::truncated(source.clone(), offset + 1, bytes.len()))?;
    let base = *bytes
        .get(offset + 1)
        .ok_or_else(|| ReadError::truncated(source.clone(), offset + 2, bytes.len()))?;
    let raw = decode_raw_unit(scaler, base);
    let unit = (raw.power == 1).then(|| AxisUnit::decode(base)).flatten();
    Ok((unit, raw))
}
