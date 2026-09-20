use super::context::ErrorContext;
use super::context::parameter_error;
use super::parameters::ParameterFile;
use super::sample_decode::decode_complex_pairs_into;
use crate::Complex64;
use crate::ReadError;
use crate::raw::ParameterErrorKind;
use crate::raw::ReadLimits;
use std::fs;
use std::io::SeekFrom;
use std::io::{Read, Seek};
use std::path::Path;

#[derive(Clone, Copy)]
pub(crate) struct DirectStorage {
    pub(super) td: usize,
    pub(super) endian: Endian,
    pub(super) sample: SampleFormat,
}

pub(crate) fn direct_storage(
    parameters: &ParameterFile,
    source: &(impl ErrorContext + ?Sized),
) -> Result<DirectStorage, ReadError> {
    let aq_mod = parameters.integer("AQ_mod", source)?;
    if aq_mod != 3 {
        return Err(ReadError::unsupported(
            source.input_source(),
            format!("AQ_mod={aq_mod}; validated direct acquisition mode is 3 (DQD)"),
        ));
    }
    let td = parameters.usize("TD", source)?;
    if td < 2 || td % 2 != 0 {
        return Err(parameter_error(
            source,
            Some("TD"),
            ParameterErrorKind::Invalid,
            format!("value {td}: expected an even value of at least 2 for complex data"),
        )
        .into());
    }
    let endian = match parameters.integer("BYTORDA", source)? {
        0 => Endian::Little,
        1 => Endian::Big,
        value => {
            return Err(ReadError::unsupported(
                source.input_source(),
                format!("BYTORDA={value}; expected 0 (little endian) or 1 (big endian)"),
            ));
        }
    };
    let sample = match parameters.integer("DTYPA", source)? {
        0 => SampleFormat::I32,
        2 => SampleFormat::F64,
        value => {
            return Err(ReadError::unsupported(
                source.input_source(),
                format!("DTYPA={value}; v1 supports int32 and float64 samples"),
            ));
        }
    };
    Ok(DirectStorage { td, endian, sample })
}

#[derive(Clone, Copy)]
pub(crate) enum Endian {
    Little,
    Big,
}

#[derive(Clone, Copy)]
pub(crate) enum SampleFormat {
    I32,
    F64,
}

impl SampleFormat {
    pub(super) const fn bytes(self) -> usize {
        match self {
            Self::I32 => 4,
            Self::F64 => 8,
        }
    }

    pub(super) fn read(self, bytes: &[u8], endian: Endian) -> f64 {
        match self {
            Self::I32 => {
                let bytes: [u8; 4] = bytes.try_into().expect("i32 sample width is fixed");
                match endian {
                    Endian::Little => i32::from_le_bytes(bytes) as f64,
                    Endian::Big => i32::from_be_bytes(bytes) as f64,
                }
            }
            Self::F64 => {
                let bytes: [u8; 8] = bytes.try_into().expect("f64 sample width is fixed");
                match endian {
                    Endian::Little => f64::from_le_bytes(bytes),
                    Endian::Big => f64::from_be_bytes(bytes),
                }
            }
        }
    }
}

pub(crate) fn parameter_storage_bound(text: &str) -> Result<usize, ReadError> {
    let count = text.lines().filter(|line| line.starts_with("##$")).count();
    // Parsed names/values and every replacement title occupy disjoint source
    // spans (normalized separators cannot exceed original line separators).
    // Add one complete raw-text copy and the fixed Arc<String> allocation.
    count
        .checked_mul(std::mem::size_of::<(String, String)>())
        .and_then(|bytes| bytes.checked_add(text.len()))
        .and_then(|bytes| bytes.checked_add(text.len()))
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>())
        })
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<ParameterFile>()))
        .ok_or(ReadError::SizeOverflow)
}

pub(crate) fn ensure_parameter_storage<'a>(
    mut texts: impl Iterator<Item = &'a str>,
    owned_input_bytes: usize,
    options: &ReadLimits,
) -> Result<usize, ReadError> {
    let parsed = texts.try_fold(0usize, |sum, text| {
        sum.checked_add(parameter_storage_bound(text)?)
            .ok_or(ReadError::SizeOverflow)
    })?;
    let required = parsed
        .checked_add(owned_input_bytes)
        .ok_or(ReadError::SizeOverflow)?;
    ensure_decode_limit(required, options)?;
    Ok(parsed)
}

pub(crate) fn reserve_parameter_files(count: usize) -> Result<Vec<ParameterFile>, ReadError> {
    let requested_bytes = count
        .checked_mul(std::mem::size_of::<ParameterFile>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut files = Vec::new();
    files
        .try_reserve_exact(count)
        .map_err(|_| ReadError::allocation(requested_bytes))?;
    Ok(files)
}

pub(crate) fn ensure_decode_limit(bytes: usize, options: &ReadLimits) -> Result<(), ReadError> {
    options.check_working(crate::raw::ReadResource::WorkingBytes, bytes)
}

pub(crate) fn trace_stride(
    direct: &ParameterFile,
    payload_bytes: usize,
    rows: usize,
    actual_bytes: usize,
    source: &(impl ErrorContext + ?Sized),
) -> Result<usize, ReadError> {
    let standard = payload_bytes
        .checked_add(1023)
        .map(|value| value / 1024 * 1024)
        .ok_or(ReadError::SizeOverflow)?;
    match direct.text("GO_block_size") {
        Some(value) if value.eq_ignore_ascii_case("continuous") => Ok(payload_bytes),
        Some(value) if value.eq_ignore_ascii_case("Standard_KBlock_Format") => Ok(standard),
        None => {
            let continuous_bytes = payload_bytes
                .checked_mul(rows)
                .ok_or(ReadError::SizeOverflow)?;
            let standard_bytes = standard.checked_mul(rows).ok_or(ReadError::SizeOverflow)?;
            if continuous_bytes == standard_bytes {
                return Ok(standard);
            }
            match (
                actual_bytes == continuous_bytes,
                actual_bytes == standard_bytes,
            ) {
                (true, false) => Ok(payload_bytes),
                (false, true) | (true, true) => Ok(standard),
                (false, false) => Err(ReadError::unsupported(
                    source.input_source(),
                    "GO_block_size is absent and ser length matches neither continuous nor standard K-block rows",
                )),
            }
        }
        Some(value) => Err(parameter_error(
            source,
            Some("GO_block_size"),
            ParameterErrorKind::Invalid,
            format!("value {value:?}: expected continuous or Standard_KBlock_Format"),
        )
        .into()),
    }
}

pub(crate) fn decode_ser_row(
    ser: &[u8],
    row: usize,
    stride: usize,
    payload_bytes: usize,
    storage: DirectStorage,
    source: &(impl ErrorContext + ?Sized),
    output: &mut [Complex64],
) -> Result<(), ReadError> {
    let start = row.checked_mul(stride).ok_or(ReadError::SizeOverflow)?;
    let end = start
        .checked_add(payload_bytes)
        .ok_or(ReadError::SizeOverflow)?;
    decode_complex_pairs_into(&ser[start..end], storage, "ser", source, output)
}

pub(crate) fn validate_zero_padding(
    path: &Path,
    payload_bytes: usize,
    source_len: usize,
) -> Result<(), ReadError> {
    if payload_bytes == source_len {
        return Ok(());
    }
    let mut file = fs::File::open(path).map_err(|error| ReadError::io(path, error))?;
    file.seek(SeekFrom::Start(
        u64::try_from(payload_bytes).map_err(|_| ReadError::SizeOverflow)?,
    ))
    .map_err(|error| ReadError::io(path, error))?;
    let mut remaining = source_len - payload_bytes;
    let mut buffer = [0u8; 8192];
    while remaining != 0 {
        let length = remaining.min(buffer.len());
        file.read_exact(&mut buffer[..length])
            .map_err(|error| ReadError::io(path, error))?;
        if buffer[..length].iter().any(|&byte| byte != 0) {
            return Err(ReadError::corrupt(
                path.into(),
                "nonzero bytes remain beyond the declared Bruker payload",
            ));
        }
        remaining -= length;
    }
    Ok(())
}

pub(crate) fn enforce_lazy_trace_limit(
    payload_bytes: usize,
    direct_points: usize,
    component_count: usize,
    options: &ReadLimits,
) -> Result<(), ReadError> {
    let decoded = direct_points
        .checked_mul(component_count)
        .and_then(|value| value.checked_mul(std::mem::size_of::<Complex64>()))
        .ok_or(ReadError::SizeOverflow)?;
    let working = payload_bytes
        .checked_add(decoded)
        .ok_or(ReadError::SizeOverflow)?;
    options.check_working(crate::raw::ReadResource::TraceBytes, working)
}
