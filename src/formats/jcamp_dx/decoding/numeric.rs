use crate::read_error::ReadError;
use std::path::Path;

use super::parameters::invalid;

#[derive(Clone, Copy)]
pub(crate) enum Encoded {
    Absolute(f64),
    Difference(f64),
    Duplicate(usize),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn decode_xydata(
    control: &mut crate::ExecutionContext<'_>,
    lines: &[&str],
    first_x: f64,
    delta_x: f64,
    npoints: usize,
    x_factor: f64,
    y_factor: f64,
    source: &Path,
) -> Result<Vec<f64>, ReadError> {
    if lines.is_empty() {
        return Err(ReadError::incomplete(
            source.into(),
            "XYDATA contains no data lines",
        ));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(npoints)
        .map_err(|_| ReadError::allocation(npoints.saturating_mul(8)))?;
    let mut previous: Option<f64> = None;
    let mut previous_difference: Option<f64> = None;
    let mut duplicate_difference = false;
    for line in lines {
        control.check_cancelled()?;
        let requires_y_check = duplicate_difference;
        let trimmed = line.trim_start();
        let checkpoint_end = numeric_prefix_end(trimmed);
        if checkpoint_end == 0 {
            return Err(ReadError::corrupt(
                source.into(),
                "XYDATA line has no X checkpoint",
            ));
        }
        let raw_x = trimmed[..checkpoint_end]
            .parse::<f64>()
            .map_err(|_| invalid(source, "XYDATA X checkpoint"))?;
        let x = raw_x * x_factor;
        let checkpoint_index = if requires_y_check {
            output.len().checked_sub(1).ok_or_else(|| {
                ReadError::corrupt(source.into(), "DIF checkpoint has no previous sample")
            })?
        } else {
            output.len()
        };
        let expected_x = first_x + delta_x * checkpoint_index as f64;
        let tolerance = f64::EPSILON * 64.0 * x.abs().max(expected_x.abs()).max(1.0);
        if !x.is_finite() || (x - expected_x).abs() > tolerance {
            return Err(ReadError::corrupt(
                source.into(),
                "XYDATA line X checkpoint is inconsistent",
            ));
        }
        let mut tokens = scan_asdf(
            &trimmed[checkpoint_end..],
            npoints
                .saturating_sub(output.len())
                .saturating_add(usize::from(requires_y_check)),
            source,
        );
        if requires_y_check {
            let check = match tokens.next().transpose()? {
                Some(Encoded::Absolute(raw)) => raw * y_factor,
                _ => {
                    return Err(ReadError::corrupt(
                        source.into(),
                        "DIF line is missing its absolute Y checkpoint",
                    ));
                }
            };
            let expected = previous.ok_or_else(|| {
                ReadError::corrupt(source.into(), "DIF checkpoint has no previous sample")
            })?;
            let tolerance = f64::EPSILON * 64.0 * check.abs().max(expected.abs()).max(1.0);
            if !check.is_finite() || (check - expected).abs() > tolerance {
                return Err(ReadError::corrupt(
                    source.into(),
                    "DIF Y checkpoint is inconsistent",
                ));
            }
            duplicate_difference = false;
        }
        for token in tokens {
            match token? {
                Encoded::Absolute(raw) => {
                    let value = raw * y_factor;
                    push_finite(&mut output, value, npoints, source)?;
                    previous_difference = previous.map(|old| value - old);
                    previous = Some(value);
                    duplicate_difference = false;
                }
                Encoded::Difference(raw) => {
                    let difference = raw * y_factor;
                    let value = previous.ok_or_else(|| {
                        ReadError::corrupt(source.into(), "DIF has no previous sample")
                    })? + difference;
                    push_finite(&mut output, value, npoints, source)?;
                    previous = Some(value);
                    previous_difference = Some(difference);
                    duplicate_difference = true;
                }
                Encoded::Duplicate(count) => {
                    let additional = count.checked_sub(1).ok_or_else(|| {
                        ReadError::corrupt(source.into(), "DUP count must be positive")
                    })?;
                    if additional > npoints.saturating_sub(output.len()) {
                        return Err(ReadError::corrupt(
                            source.into(),
                            "XYDATA expands beyond NPOINTS",
                        ));
                    }
                    for _ in 0..additional {
                        let value = match (previous, previous_difference, duplicate_difference) {
                            (Some(value), Some(difference), true) => value + difference,
                            (Some(value), _, false) => value,
                            _ => {
                                return Err(ReadError::corrupt(
                                    source.into(),
                                    "DUP has no previous decoder state",
                                ));
                            }
                        };
                        push_finite(&mut output, value, npoints, source)?;
                        previous = Some(value);
                    }
                }
            }
            if output.len() > npoints {
                return Err(ReadError::corrupt(
                    source.into(),
                    "XYDATA expands beyond NPOINTS",
                ));
            }
        }
    }
    if output.len() != npoints {
        return Err(ReadError::truncated(source.into(), npoints, output.len()));
    }
    Ok(output)
}

pub(super) fn push_finite(
    output: &mut Vec<f64>,
    value: f64,
    max_points: usize,
    source: &Path,
) -> Result<(), ReadError> {
    if !value.is_finite() {
        return Err(ReadError::corrupt(source.into(), "non-finite JCAMP sample"));
    }
    if output.len() == max_points {
        return Err(ReadError::corrupt(
            source.into(),
            "XYDATA expands beyond NPOINTS",
        ));
    }
    output.push(value);
    Ok(())
}

pub(super) fn numeric_prefix_end(value: &str) -> usize {
    affn_token_end(value)
}

pub(super) fn scan_asdf<'a>(
    mut text: &'a str,
    max_tokens: usize,
    source: &'a Path,
) -> impl Iterator<Item = Result<Encoded, ReadError>> + 'a {
    let mut count = 0;
    std::iter::from_fn(move || {
        if text.trim_start().is_empty() {
            return None;
        }
        let result = if count == max_tokens {
            Err(ReadError::corrupt(
                source.into(),
                "XYDATA expands beyond NPOINTS",
            ))
        } else {
            count += 1;
            next_asdf(&mut text, source)
        };
        if result.is_err() {
            text = "";
        }
        Some(result)
    })
}

pub(super) fn next_asdf(text: &mut &str, source: &Path) -> Result<Encoded, ReadError> {
    *text = text.trim_start();
    let first = text.as_bytes()[0] as char;
    let (kind, leading) = decode_lead(first).ok_or_else(|| {
        ReadError::corrupt(
            source.into(),
            format!("invalid JCAMP compressed character {first:?}"),
        )
    })?;
    if kind == 3 {
        let mut end = 1;
        while end < text.len() && text.as_bytes()[end].is_ascii_digit() {
            end += 1;
        }
        let tail = &text[1..end];
        let places = 10usize
            .checked_pow(u32::try_from(tail.len()).map_err(|_| ReadError::SizeOverflow)?)
            .ok_or(ReadError::SizeOverflow)?;
        let trailing = if tail.is_empty() {
            0
        } else {
            tail.parse::<usize>()
                .map_err(|_| invalid(source, "DUP count"))?
        };
        let count = (leading as usize)
            .checked_mul(places)
            .and_then(|value| value.checked_add(trailing))
            .ok_or(ReadError::SizeOverflow)?;
        *text = &text[end..];
        return Ok(Encoded::Duplicate(count));
    }
    if matches!(first, '+' | '-' | '.' | '0'..='9') {
        let end = affn_token_end(text);
        let value = text[..end]
            .parse::<f64>()
            .map_err(|_| invalid(source, "PACKED value"))?;
        *text = &text[end..];
        return Ok(Encoded::Absolute(value));
    }
    let mut end = 1;
    while end < text.len() && text.as_bytes()[end].is_ascii_digit() {
        end += 1;
    }
    let tail = &text[1..end];
    let places = 10f64.powi(tail.len() as i32);
    let trailing = if tail.is_empty() {
        0.0
    } else {
        tail.parse::<f64>()
            .map_err(|_| invalid(source, "compressed value"))?
    };
    let value = leading as f64 * places + leading.signum() as f64 * trailing;
    *text = &text[end..];
    Ok(if kind == 2 {
        Encoded::Difference(value)
    } else {
        Encoded::Absolute(value)
    })
}

pub(super) fn affn_token_end(value: &str) -> usize {
    let bytes = value.as_bytes();
    let mut end = usize::from(
        bytes
            .first()
            .is_some_and(|byte| matches!(byte, b'+' | b'-')),
    );
    let mut exponent = false;
    let mut exponent_sign = false;
    while end < bytes.len() {
        match bytes[end] {
            b'0'..=b'9' | b'.' => end += 1,
            b'e' | b'E' if !exponent && end > 0 => {
                exponent = true;
                exponent_sign = true;
                end += 1;
            }
            b'+' | b'-' if exponent_sign => {
                exponent_sign = false;
                end += 1;
            }
            _ => break,
        }
    }
    end
}

pub(super) fn decode_lead(character: char) -> Option<(u8, i32)> {
    match character {
        '@' => Some((1, 0)),
        'A'..='I' => Some((1, character as i32 - 'A' as i32 + 1)),
        'a'..='i' => Some((1, -(character as i32 - 'a' as i32 + 1))),
        '%' => Some((2, 0)),
        'J'..='R' => Some((2, character as i32 - 'J' as i32 + 1)),
        'j'..='r' => Some((2, -(character as i32 - 'j' as i32 + 1))),
        'S'..='Z' => Some((3, character as i32 - 'S' as i32 + 1)),
        's' => Some((3, 9)),
        '+' | '-' | '.' | '0'..='9' => Some((1, 0)),
        _ => None,
    }
}
