use crate::provenance::SourceFile;
use crate::provenance::SourceKind;
use crate::read_error::ReadError;
use std::path::Path;

use super::parameters::Parameters;
use super::reading::reserve_f64;

#[derive(Clone, Copy)]
pub(super) struct Storage {
    dtypp: i32,
    bytordp: i32,
    nc_proc: i32,
}

impl Storage {
    pub(super) fn from_parameters(
        parameters: &Parameters,
        source: &Path,
    ) -> Result<Self, ReadError> {
        let value = Self {
            dtypp: parameters.i32("DTYPP", source)?,
            bytordp: parameters.i32("BYTORDP", source)?,
            nc_proc: parameters.i32("NC_PROC", source)?,
        };
        if !matches!(value.dtypp, 0 | 2) {
            return Err(ReadError::unsupported(
                source.into(),
                format!("DTYPP={}", value.dtypp),
            ));
        }
        if !matches!(value.bytordp, 0 | 1) {
            return Err(ReadError::unsupported(
                source.into(),
                format!("BYTORDP={}", value.bytordp),
            ));
        }
        Ok(value)
    }

    pub(super) fn bytes(self) -> usize {
        if self.dtypp == 0 { 4 } else { 8 }
    }
}

pub(super) fn decode_file_recorded(
    control: &mut crate::ExecutionContext<'_>,
    path: &Path,
    count: usize,
    storage: Storage,
    role: &str,
) -> Result<(Vec<f64>, SourceFile), ReadError> {
    let expected = count
        .checked_mul(storage.bytes())
        .ok_or(ReadError::SizeOverflow)?;
    let scale = 2f64.powi(storage.nc_proc);
    if !scale.is_finite() {
        return Err(ReadError::corrupt(
            path.into(),
            "NC_proc produces a non-finite scale",
        ));
    }
    let bytes = crate::io::read_sized_file_controlled(control, path, expected)?;
    if bytes.len() < expected {
        return Err(ReadError::truncated(path.into(), expected, bytes.len()));
    }
    if bytes.len() > expected {
        return Err(ReadError::corrupt(
            path.into(),
            "bytes remain beyond the declared processed shape",
        ));
    }
    let mut values = reserve_f64(count)?;
    for (index, chunk) in bytes.chunks_exact(storage.bytes()).enumerate() {
        if index % 4096 == 0 {
            control.check_cancelled()?;
        }
        let raw = match (storage.dtypp, storage.bytordp) {
            (0, 0) => i32::from_le_bytes(chunk.try_into().unwrap()) as f64,
            (0, 1) => i32::from_be_bytes(chunk.try_into().unwrap()) as f64,
            (2, 0) => f64::from_le_bytes(chunk.try_into().unwrap()),
            (2, 1) => f64::from_be_bytes(chunk.try_into().unwrap()),
            _ => unreachable!(),
        };
        if !raw.is_finite() {
            return Err(ReadError::corrupt(
                path.into(),
                "non-finite processed sample",
            ));
        }
        let value = raw * scale;
        if !value.is_finite() || raw != 0.0 && value == 0.0 {
            return Err(ReadError::corrupt(
                path.into(),
                "processed scaling is not finite for a nonzero value",
            ));
        }
        values.push(value);
    }
    let source = SourceFile::from_consumed_bytes(SourceKind::Data, role, path, &bytes);
    Ok((values, source))
}

impl Storage {
    pub(super) fn nc_proc(self) -> i32 {
        self.nc_proc
    }
    pub(super) fn dtypp(self) -> i32 {
        self.dtypp
    }
    pub(super) fn bytordp(self) -> i32 {
        self.bytordp
    }
}
