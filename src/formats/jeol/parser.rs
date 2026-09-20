use super::header::BodyEndian;
use super::header::JdfHeader;
use super::parameters::ParameterRecord;
use super::parameters::ParameterValue;

use crate::{Complex64, ReadError};
use std::collections::BTreeMap;

pub(super) fn parse_parameter_table(
    bytes: &[u8],
    header: &JdfHeader,
    data_end: usize,
) -> Result<BTreeMap<String, ParameterRecord>, ReadError> {
    if header.param_start == 0 || header.param_length == 0 {
        return Ok(BTreeMap::new());
    }
    let table_end = header
        .param_start
        .checked_add(header.param_length)
        .ok_or(ReadError::SizeOverflow)?;
    if header.param_start < data_end && table_end > header.data_start {
        return Err(ReadError::corrupt(
            header.input_source.clone(),
            "JEOL parameter table overlaps the sample payload",
        ));
    }
    let table = bytes
        .get(header.param_start..table_end)
        .ok_or_else(|| ReadError::truncated(header.input_source.clone(), table_end, bytes.len()))?;
    parse_parameter_records(table, header, header.param_start)
}

pub(super) fn parse_parameter_records(
    table: &[u8],
    header: &JdfHeader,
    table_start: usize,
) -> Result<BTreeMap<String, ParameterRecord>, ReadError> {
    if table.len() < 16 {
        return Err(ReadError::truncated(
            header.input_source.clone(),
            table_start + 16,
            table_start + table.len(),
        ));
    }
    let read_u32 = |slice: &[u8]| match header.body_endian {
        BodyEndian::Big => u32::from_be_bytes(slice.try_into().unwrap()),
        BodyEndian::Little => u32::from_le_bytes(slice.try_into().unwrap()),
    } as usize;
    let parameter_size = read_u32(&table[0..4]);
    let high_index = read_u32(&table[8..12]);
    let declared_total = read_u32(&table[12..16]);
    if parameter_size < 64 {
        return Err(ReadError::corrupt(
            header.input_source.clone(),
            format!("JEOL parameter record size {parameter_size} is smaller than 64"),
        ));
    }
    let expected = high_index
        .checked_mul(parameter_size)
        .and_then(|value| value.checked_add(16))
        .ok_or(ReadError::SizeOverflow)?;
    if expected > table.len() || declared_total > table.len() {
        let expected_absolute = table_start
            .checked_add(expected.max(declared_total))
            .ok_or(ReadError::SizeOverflow)?;
        return Err(ReadError::truncated(
            header.input_source.clone(),
            expected_absolute,
            table_start + table.len(),
        ));
    }
    let mut parameters = BTreeMap::new();
    for index in 0..high_index {
        let start = 16 + index * parameter_size;
        let record = &table[start..start + parameter_size];
        let mut class = [0; 4];
        class.copy_from_slice(&record[..4]);
        let unit_scaler = match header.body_endian {
            BodyEndian::Big => i16::from_be_bytes(record[4..6].try_into().unwrap()),
            BodyEndian::Little => i16::from_le_bytes(record[4..6].try_into().unwrap()),
        };
        let mut raw_units = [0; 10];
        raw_units.copy_from_slice(&record[6..16]);
        let mut raw_value = [0; 16];
        raw_value.copy_from_slice(&record[16..32]);
        let type_code = match header.body_endian {
            BodyEndian::Big => i32::from_be_bytes(record[32..36].try_into().unwrap()),
            BodyEndian::Little => i32::from_le_bytes(record[32..36].try_into().unwrap()),
        };
        let name = std::str::from_utf8(&record[36..64])
            .map_err(|_| {
                ReadError::corrupt(
                    header.input_source.clone(),
                    "JEOL parameter name is not UTF-8",
                )
            })?
            .trim_matches(['\0', ' '])
            .to_owned();
        if name.is_empty() {
            return Err(ReadError::corrupt(
                header.input_source.clone(),
                "JEOL parameter has an empty name",
            ));
        }
        let value = match type_code {
            0 => ParameterValue::String(
                std::str::from_utf8(&raw_value)
                    .map_err(|_| {
                        ReadError::corrupt(
                            header.input_source.clone(),
                            format!("JEOL parameter {name} is not UTF-8"),
                        )
                    })?
                    .trim_matches(['\0', ' '])
                    .to_owned(),
            ),
            1 => ParameterValue::Integer(match header.body_endian {
                BodyEndian::Big => i32::from_be_bytes(raw_value[..4].try_into().unwrap()),
                BodyEndian::Little => i32::from_le_bytes(raw_value[..4].try_into().unwrap()),
            }),
            2 => ParameterValue::Float(match header.body_endian {
                BodyEndian::Big => f64::from_be_bytes(raw_value[..8].try_into().unwrap()),
                BodyEndian::Little => f64::from_le_bytes(raw_value[..8].try_into().unwrap()),
            }),
            3 => {
                let read = |value: &[u8]| match header.body_endian {
                    BodyEndian::Big => f64::from_be_bytes(value.try_into().unwrap()),
                    BodyEndian::Little => f64::from_le_bytes(value.try_into().unwrap()),
                };
                ParameterValue::Complex(Complex64::new(
                    read(&raw_value[..8]),
                    read(&raw_value[8..]),
                ))
            }
            4 => ParameterValue::Infinity(match header.body_endian {
                BodyEndian::Big => i32::from_be_bytes(raw_value[..4].try_into().unwrap()),
                BodyEndian::Little => i32::from_le_bytes(raw_value[..4].try_into().unwrap()),
            }),
            _ => ParameterValue::Unknown {
                type_code,
                bytes: raw_value,
            },
        };
        if matches!(value, ParameterValue::Float(value) if !value.is_finite())
            || matches!(value, ParameterValue::Complex(value) if !value.re.is_finite() || !value.im.is_finite())
        {
            return Err(ReadError::corrupt(
                header.input_source.clone(),
                format!("JEOL parameter {name} is non-finite"),
            ));
        }
        let key = name.to_ascii_lowercase().replace(' ', "");
        if parameters
            .insert(
                key,
                ParameterRecord {
                    class,
                    unit_scaler,
                    raw_units,
                    value,
                    name,
                },
            )
            .is_some()
        {
            return Err(ReadError::corrupt(
                header.input_source.clone(),
                "duplicate JEOL parameter name",
            ));
        }
    }
    Ok(parameters)
}
