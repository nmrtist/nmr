use super::context::ErrorContext;
use super::context::parameter_error;

use crate::SamplingCoordinate;
use crate::raw::{ParameterError, ParameterErrorKind};

#[cfg(test)]
thread_local! {
    pub(super) static NUS_PARSER_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn parse_nuslist(
    text: &str,
    grid: usize,
    expected: usize,
    source: &(impl ErrorContext + ?Sized),
) -> Result<Vec<SamplingCoordinate>, ParameterError> {
    #[cfg(test)]
    NUS_PARSER_CALLS.with(|calls| calls.set(calls.get() + 1));
    let requested_bytes = expected
        .checked_mul(std::mem::size_of::<SamplingCoordinate>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(|| {
            parameter_error(
                source,
                Some("nuslist"),
                ParameterErrorKind::SizeOverflow,
                "coordinate table size overflow",
            )
        })?;
    let mut coordinates = Vec::new();
    coordinates.try_reserve_exact(expected).map_err(|_| {
        parameter_error(
            source,
            Some("nuslist"),
            ParameterErrorKind::Allocation { requested_bytes },
            "could not reserve coordinate table",
        )
    })?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let raw = fields.next().expect("non-empty line");
        if fields.next().is_some() {
            return Err(parameter_error(
                source,
                Some("nuslist"),
                ParameterErrorKind::Invalid,
                format!("value {line:?}: expected one zero-based coordinate per line for 2D data"),
            ));
        }
        let coordinate = raw.parse::<usize>().map_err(|_| {
            parameter_error(
                source,
                Some("nuslist"),
                ParameterErrorKind::Invalid,
                format!("value {raw:?}: expected a non-negative integer coordinate"),
            )
        })?;
        if coordinate >= grid {
            return Err(parameter_error(
                source,
                Some("nuslist"),
                ParameterErrorKind::Invalid,
                format!("value {raw:?}: coordinate exceeds the logical NusTD grid"),
            ));
        }
        if coordinates.len() < expected {
            coordinates.push(SamplingCoordinate::new(vec![coordinate]));
        }
    }
    if coordinates.is_empty() {
        return Err(parameter_error(
            source,
            Some("nuslist"),
            ParameterErrorKind::Invalid,
            "expected at least one coordinate",
        ));
    }
    if coordinates.len() != expected {
        return Err(parameter_error(
            source,
            Some("nuslist"),
            ParameterErrorKind::Invalid,
            format!(
                "length {} does not match the stored indirect trace count",
                coordinates.len()
            ),
        ));
    }
    Ok(coordinates)
}
