use super::options::VarianOptions;

use super::parameters::BlockHeader;
use super::parameters::FileHeader;
use super::parameters::ParameterRecord;
use super::parameters::ParameterType;
use super::parameters::ParameterValue;
use super::parameters::Parameters;

use crate::ReadError;
use crate::formats::varian::layout::LayoutPlan;
use crate::raw::RawFormat;
use crate::raw::{InputSource, ParameterError, ParameterErrorKind};
use std::collections::BTreeMap;

pub(super) struct ParsedBinary {
    pub(super) layout: LayoutPlan,
    pub(super) file_header: FileHeader,
    pub(super) block_headers: Vec<BlockHeader>,
}

pub(super) fn parse_binary<F>(
    header: &[u8],
    source_len: usize,
    options: &VarianOptions,
    source: &InputSource,
    mut read_block_header: F,
) -> Result<ParsedBinary, ReadError>
where
    F: FnMut(usize) -> Result<[u8; 28], ReadError>,
{
    if header.len() < 32 {
        return Err(ReadError::truncated(source.clone(), 32, header.len()));
    }
    let nblocks = u32::from_be_bytes(header[0..4].try_into().unwrap()) as usize;
    let ntraces = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
    let stored_values = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;
    let bytes_per_value = u32::from_be_bytes(header[12..16].try_into().unwrap()) as usize;
    let trace_bytes = u32::from_be_bytes(header[16..20].try_into().unwrap()) as usize;
    let block_bytes = u32::from_be_bytes(header[20..24].try_into().unwrap()) as usize;
    let version_id = u16::from_be_bytes(header[24..26].try_into().unwrap());
    let status = u16::from_be_bytes(header[26..28].try_into().unwrap());
    let raw_block_header_word = u32::from_be_bytes(header[28..32].try_into().unwrap());
    let version = version_id & 0x003f;
    let block_headers = if version == 0 {
        1
    } else {
        raw_block_header_word as usize & 0x000f
    };

    if nblocks == 0 || ntraces == 0 || stored_values == 0 || !matches!(bytes_per_value, 2 | 4) {
        return Err(ReadError::corrupt(
            source.clone(),
            "invalid Varian file header",
        ));
    }
    if status & 0x1 == 0 {
        return Err(ReadError::corrupt(
            source.clone(),
            "Varian file header does not mark data as present",
        ));
    }
    if block_headers == 0 {
        return Err(ReadError::corrupt(
            source.clone(),
            "Varian file declares no block headers",
        ));
    }
    if block_headers != 1 {
        return Err(ReadError::unsupported(
            source.clone(),
            format!("Varian raw data with {block_headers} block headers"),
        ));
    }
    if status & 0x2 != 0 {
        return Err(ReadError::unsupported(
            source.clone(),
            "processed Varian data",
        ));
    }
    let is_float = status & 0x8 != 0;
    let is_32_bit_integer = status & 0x4 != 0;
    if is_float && bytes_per_value != 4
        || !is_float && is_32_bit_integer && bytes_per_value != 4
        || !is_float && !is_32_bit_integer && bytes_per_value != 2
    {
        return Err(ReadError::corrupt(
            source.clone(),
            "status and sample width disagree",
        ));
    }

    let block_header_bytes = block_headers
        .checked_mul(28)
        .ok_or(ReadError::SizeOverflow)?;
    let expected_trace_bytes = stored_values
        .checked_mul(bytes_per_value)
        .ok_or(ReadError::SizeOverflow)?;
    if trace_bytes != expected_trace_bytes {
        return Err(ReadError::corrupt(
            source.clone(),
            "Varian trace length does not match np/ebytes",
        ));
    }
    let declared_source_len = 32usize
        .checked_add(
            nblocks
                .checked_mul(block_bytes)
                .ok_or(ReadError::SizeOverflow)?,
        )
        .ok_or(ReadError::SizeOverflow)?;
    if source_len < declared_source_len {
        return Err(ReadError::truncated(
            source.clone(),
            declared_source_len,
            source_len,
        ));
    }
    if source_len > declared_source_len {
        return Err(ReadError::corrupt(
            source.clone(),
            "Varian fid contains bytes after the declared blocks",
        ));
    }
    let block_payload = ntraces
        .checked_mul(trace_bytes)
        .ok_or(ReadError::SizeOverflow)?;
    if block_bytes
        .checked_sub(block_header_bytes)
        .is_none_or(|bytes| bytes != block_payload)
    {
        return Err(ReadError::corrupt(
            source.clone(),
            "Varian block length does not match np/ntraces",
        ));
    }

    let complex = if version == 0 {
        status & 0x40 != 0
    } else {
        status & 0x10 != 0
    };
    if complex && stored_values % 2 != 0 {
        return Err(ReadError::corrupt(
            source.clone(),
            "complex Varian trace has an odd number of stored values",
        ));
    }

    let block_metadata_bytes = nblocks
        .checked_mul(
            std::mem::size_of::<BlockHeader>()
                .checked_add(std::mem::size_of::<f64>())
                .ok_or(ReadError::SizeOverflow)?,
        )
        .ok_or(ReadError::SizeOverflow)?;
    if block_metadata_bytes > options.max_metadata_bytes {
        return Err(ReadError::limit(
            crate::raw::ReadResource::MetadataBytes,
            options.max_metadata_bytes,
            block_metadata_bytes,
        ));
    }

    let parsed_header_allocation_bytes = nblocks
        .checked_mul(std::mem::size_of::<BlockHeader>())
        .ok_or(ReadError::SizeOverflow)?;
    let scale_factor_bytes = nblocks
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut parsed_block_headers = Vec::new();
    parsed_block_headers
        .try_reserve_exact(nblocks)
        .map_err(|_| ReadError::allocation(parsed_header_allocation_bytes))?;
    let mut scale_factors = Vec::new();
    scale_factors
        .try_reserve_exact(nblocks)
        .map_err(|_| ReadError::allocation(scale_factor_bytes))?;
    for block_index in 0..nblocks {
        let offset = 32usize
            .checked_add(
                block_index
                    .checked_mul(block_bytes)
                    .ok_or(ReadError::SizeOverflow)?,
            )
            .ok_or(ReadError::SizeOverflow)?;
        let common = read_block_header(offset)?;
        let scale = i16::from_be_bytes(common[0..2].try_into().unwrap());
        let metadata = [
            f32::from_be_bytes(common[12..16].try_into().unwrap()),
            f32::from_be_bytes(common[16..20].try_into().unwrap()),
            f32::from_be_bytes(common[20..24].try_into().unwrap()),
            f32::from_be_bytes(common[24..28].try_into().unwrap()),
        ];
        if metadata.iter().any(|value| !value.is_finite()) {
            return Err(ReadError::corrupt(
                source.clone(),
                "Varian block header contains non-finite metadata",
            ));
        }
        let factor = 2f64.powi(i32::from(scale));
        if !factor.is_finite() || factor == 0.0 {
            return Err(ReadError::corrupt(
                source.clone(),
                "Varian block scale is out of range",
            ));
        }
        scale_factors.push(factor);
        parsed_block_headers.push(BlockHeader {
            raw_bytes: common,
            scale,
            status: u16::from_be_bytes(common[2..4].try_into().unwrap()),
            index: i16::from_be_bytes(common[4..6].try_into().unwrap()),
            mode: u16::from_be_bytes(common[6..8].try_into().unwrap()),
            ctcount: i32::from_be_bytes(common[8..12].try_into().unwrap()),
            left_phase: metadata[0],
            right_phase: metadata[1],
            level: metadata[2],
            tilt: metadata[3],
        });
    }
    let trace_count = nblocks
        .checked_mul(ntraces)
        .ok_or(ReadError::SizeOverflow)?;

    Ok(ParsedBinary {
        layout: LayoutPlan {
            traces_per_block: ntraces,
            stored_values,
            bytes_per_value,
            trace_bytes,
            block_bytes,
            block_header_bytes,
            direct_points: stored_values / if complex { 2 } else { 1 },
            trace_count,
            is_float,
            is_32_bit_integer,
            complex,
            scale_factors,
        },
        file_header: FileHeader {
            raw_bytes: header[..32].try_into().unwrap(),
            nblocks,
            ntraces,
            stored_values_per_trace: stored_values,
            bytes_per_value,
            trace_bytes,
            block_bytes,
            version_id,
            status,
            block_headers,
            raw_block_header_word,
        },
        block_headers: parsed_block_headers,
    })
}

pub(super) fn parse_procpar(text: &str, source: InputSource) -> Result<Parameters, ParameterError> {
    let mut records = BTreeMap::new();
    let mut lines = text.lines();
    while let Some(header) = lines.next() {
        if header.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = header.split_whitespace().collect();
        if fields.len() != 11 {
            return Err(procpar_error(
                &source,
                fields.first().copied(),
                ParameterErrorKind::Malformed,
                format!("header must contain 11 fields: {header}"),
            ));
        }
        let name = fields[0];
        let subtype = parse_header_field::<i16>(fields[1], name, "subtype", &source)?;
        let parameter_type =
            match parse_header_field::<i16>(fields[2], name, "basic type", &source)? {
                1 => ParameterType::Real,
                2 => ParameterType::String,
                value => {
                    return Err(procpar_error(
                        &source,
                        Some(name),
                        ParameterErrorKind::Invalid,
                        format!("unsupported basic type {value}"),
                    ));
                }
            };
        let maximum = parse_finite_header_f64(fields[3], name, "maximum", &source)?;
        let minimum = parse_finite_header_f64(fields[4], name, "minimum", &source)?;
        let step = parse_finite_header_f64(fields[5], name, "step", &source)?;
        let group = parse_header_field::<i16>(fields[6], name, "group", &source)?;
        let display_group = parse_header_field::<i16>(fields[7], name, "display group", &source)?;
        let protection = parse_header_field::<i32>(fields[8], name, "protection", &source)?;
        let active = match parse_header_field::<i16>(fields[9], name, "active flag", &source)? {
            0 => false,
            1 => true,
            value => {
                return Err(procpar_error(
                    &source,
                    Some(name),
                    ParameterErrorKind::Invalid,
                    format!("invalid active flag {value}"),
                ));
            }
        };
        u32::from_str_radix(fields[10].trim_start_matches("0x"), 16).map_err(|_| {
            procpar_error(
                &source,
                Some(name),
                ParameterErrorKind::Malformed,
                "invalid auxiliary hex field",
            )
        })?;
        let values = parse_parameter_values(
            read_counted_record(&mut lines, name, &source)?,
            parameter_type,
            name,
            &source,
        )?;
        let enumerated_values = parse_parameter_values(
            read_counted_record(&mut lines, name, &source)?,
            parameter_type,
            name,
            &source,
        )?;
        let record = ParameterRecord {
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
        if records.insert(name.to_string(), record).is_some() {
            return Err(procpar_error(
                &source,
                Some(name),
                ParameterErrorKind::Duplicate,
                "parameter occurs more than once",
            ));
        }
    }
    Ok(Parameters {
        records,
        raw_text: text.to_owned(),
        file_header: None,
        block_headers: Vec::new(),
        input_source: source,
    })
}

fn procpar_error(
    source: &InputSource,
    parameter: Option<&str>,
    kind: ParameterErrorKind,
    detail: impl Into<String>,
) -> ParameterError {
    ParameterError::new(
        RawFormat::VarianRaw,
        source.clone(),
        parameter.map(str::to_owned),
        kind,
        detail,
    )
}

fn parse_header_field<T>(
    raw: &str,
    name: &str,
    field: &str,
    source: &InputSource,
) -> Result<T, ParameterError>
where
    T: std::str::FromStr,
{
    raw.parse().map_err(|_| {
        procpar_error(
            source,
            Some(name),
            ParameterErrorKind::Malformed,
            format!("invalid {field}"),
        )
    })
}

fn parse_finite_header_f64(
    raw: &str,
    name: &str,
    field: &str,
    source: &InputSource,
) -> Result<f64, ParameterError> {
    let value = parse_header_field::<f64>(raw, name, field, source)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(procpar_error(
            source,
            Some(name),
            ParameterErrorKind::Invalid,
            format!("non-finite {field}"),
        ))
    }
}

fn parse_parameter_values(
    values: Vec<String>,
    parameter_type: ParameterType,
    name: &str,
    source: &InputSource,
) -> Result<Vec<ParameterValue>, ParameterError> {
    values
        .into_iter()
        .map(|value| match parameter_type {
            ParameterType::String => Ok(ParameterValue::String(value)),
            ParameterType::Real => {
                let parsed = value.parse::<f64>().map_err(|_| {
                    procpar_error(
                        source,
                        Some(name),
                        ParameterErrorKind::Malformed,
                        "contains a non-real value",
                    )
                })?;
                if parsed.is_finite() {
                    Ok(ParameterValue::Real(parsed))
                } else {
                    Err(procpar_error(
                        source,
                        Some(name),
                        ParameterErrorKind::Invalid,
                        "contains a non-finite value",
                    ))
                }
            }
        })
        .collect()
}

fn read_counted_record(
    lines: &mut std::str::Lines<'_>,
    name: &str,
    source: &InputSource,
) -> Result<Vec<String>, ParameterError> {
    let first = lines.next().ok_or_else(|| {
        procpar_error(
            source,
            Some(name),
            ParameterErrorKind::Malformed,
            "truncated value record",
        )
    })?;
    let mut tokens = procpar_tokens(first, name, source)?;
    let count = tokens
        .first()
        .ok_or_else(|| {
            procpar_error(
                source,
                Some(name),
                ParameterErrorKind::Malformed,
                "empty value record",
            )
        })?
        .parse::<usize>()
        .map_err(|_| {
            procpar_error(
                source,
                Some(name),
                ParameterErrorKind::Malformed,
                "invalid value count",
            )
        })?;
    tokens.remove(0);
    while tokens.len() < count {
        let continuation = lines.next().ok_or_else(|| {
            procpar_error(
                source,
                Some(name),
                ParameterErrorKind::Malformed,
                "truncated value array",
            )
        })?;
        tokens.extend(procpar_tokens(continuation, name, source)?);
    }
    if tokens.len() != count {
        return Err(procpar_error(
            source,
            Some(name),
            ParameterErrorKind::Malformed,
            "value count mismatch",
        ));
    }
    Ok(tokens)
}

fn procpar_tokens(
    line: &str,
    name: &str,
    source: &InputSource,
) -> Result<Vec<String>, ParameterError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut token_started = false;
    let mut quoted = false;
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            current.push(character);
            token_started = true;
            escaped = false;
        } else if quoted && character == '\\' {
            escaped = true;
        } else if character == '"' {
            token_started = true;
            quoted = !quoted;
        } else if character.is_whitespace() && !quoted {
            if token_started {
                tokens.push(std::mem::take(&mut current));
                token_started = false;
            }
        } else {
            current.push(character);
            token_started = true;
        }
    }
    if quoted || escaped {
        return Err(procpar_error(
            source,
            Some(name),
            ParameterErrorKind::Malformed,
            "unterminated string",
        ));
    }
    if token_started {
        tokens.push(current);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_header() -> [u8; 32] {
        let mut header = [0; 32];
        header[0..4].copy_from_slice(&1_u32.to_be_bytes());
        header[4..8].copy_from_slice(&1_u32.to_be_bytes());
        header[8..12].copy_from_slice(&2_u32.to_be_bytes());
        header[12..16].copy_from_slice(&2_u32.to_be_bytes());
        header[16..20].copy_from_slice(&4_u32.to_be_bytes());
        header[20..24].copy_from_slice(&32_u32.to_be_bytes());
        header[24..26].copy_from_slice(&1_u16.to_be_bytes());
        header[26..28].copy_from_slice(&0x11_u16.to_be_bytes());
        header[28..32].copy_from_slice(&1_u32.to_be_bytes());
        header
    }

    #[test]
    fn every_short_binary_header_boundary_is_typed_and_does_not_panic() {
        let header = valid_header();
        let source = InputSource::memory("fid");
        for length in 0..32 {
            let result = std::panic::catch_unwind(|| {
                parse_binary(
                    &header[..length],
                    length,
                    &VarianOptions::default(),
                    &source,
                    |_| Ok([0; 28]),
                )
            });
            let error = result
                .expect("short header parser must not panic")
                .err()
                .expect("short header must fail");
            assert!(matches!(
                error.reason(),
                crate::read_error::ReadErrorReason::Truncated {
                    expected: 32,
                    actual,
                    ..
                } if *actual == length
            ));
        }
    }

    #[test]
    fn declared_binary_length_must_match_exactly() {
        let header = valid_header();
        let source = InputSource::memory("fid");
        let truncated = parse_binary(&header, 63, &VarianOptions::default(), &source, |_| {
            Ok([0; 28])
        })
        .err()
        .expect("short declared source must fail");
        assert!(matches!(
            truncated.reason(),
            crate::read_error::ReadErrorReason::Truncated {
                expected: 64,
                actual: 63,
                ..
            }
        ));

        let trailing = parse_binary(&header, 65, &VarianOptions::default(), &source, |_| {
            Ok([0; 28])
        })
        .err()
        .expect("trailing source bytes must fail");
        assert!(matches!(
            trailing.reason(),
            crate::read_error::ReadErrorReason::Corrupt { .. }
        ));

        let parsed = parse_binary(&header, 64, &VarianOptions::default(), &source, |_| {
            Ok([0; 28])
        })
        .unwrap();
        assert_eq!(parsed.layout.direct_points, 1);
        assert_eq!(parsed.layout.block_header_bytes, 28);
        assert!(parsed.layout.complex);
    }

    #[test]
    fn block_metadata_limit_is_checked_before_allocation_or_header_reads() {
        let nblocks = 17_000_000u32;
        let mut header = valid_header();
        header[0..4].copy_from_slice(&nblocks.to_be_bytes());
        let source_len = 32 + nblocks as usize * 32;
        let mut header_reads = 0usize;
        let source = InputSource::memory("fid");

        let error = parse_binary(
            &header,
            source_len,
            &VarianOptions::default(),
            &source,
            |_| {
                header_reads += 1;
                Ok([0; 28])
            },
        )
        .err()
        .expect("oversized block metadata must fail");

        assert!(matches!(
            error.reason(),
            crate::read_error::ReadErrorReason::LimitExceeded { .. }
        ));
        assert_eq!(header_reads, 0);
    }
}
