use super::layout::LayoutPlan;

use crate::raw::InputSource;
use crate::{Complex64, ReadError};

fn decode_value(
    bytes: &[u8],
    layout: &LayoutPlan,
    factor: f64,
    source: &impl Fn() -> InputSource,
) -> Result<f64, ReadError> {
    let base = if layout.is_float {
        f32::from_be_bytes(bytes.try_into().unwrap()) as f64
    } else if layout.is_32_bit_integer {
        i32::from_be_bytes(bytes.try_into().unwrap()) as f64
    } else {
        i16::from_be_bytes(bytes.try_into().unwrap()) as f64
    };
    let value = base * factor;
    if !value.is_finite() || base != 0.0 && value == 0.0 {
        return Err(ReadError::corrupt(
            source(),
            "scaled Varian sample is non-finite or underflowed",
        ));
    }
    Ok(value)
}

pub(super) fn decode_trace(
    bytes: &[u8],
    layout: &LayoutPlan,
    block: usize,
    source: &impl Fn() -> InputSource,
) -> Result<Vec<Complex64>, ReadError> {
    if bytes.len() != layout.trace_bytes || block >= layout.scale_factors.len() {
        return Err(ReadError::corrupt(
            source(),
            "Varian trace bytes disagree with the validated layout",
        ));
    }
    let factor = layout.scale_factors[block];
    let value_stride = if layout.complex { 2usize } else { 1usize };
    if layout
        .direct_points
        .checked_mul(value_stride)
        .is_none_or(|values| values != layout.stored_values)
    {
        return Err(ReadError::corrupt(
            source(),
            "Varian direct points disagree with the stored-value count",
        ));
    }
    let point_stride = value_stride
        .checked_mul(layout.bytes_per_value)
        .ok_or(ReadError::SizeOverflow)?;
    let sample_bytes = layout
        .direct_points
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut samples = Vec::new();
    samples
        .try_reserve_exact(layout.direct_points)
        .map_err(|_| ReadError::allocation(sample_bytes))?;
    for point in 0..layout.direct_points {
        let offset = point
            .checked_mul(point_stride)
            .ok_or(ReadError::SizeOverflow)?;
        let real = decode_value(
            &bytes[offset..offset + layout.bytes_per_value],
            layout,
            factor,
            source,
        )?;
        let imaginary = if layout.complex {
            let offset = offset
                .checked_add(layout.bytes_per_value)
                .ok_or(ReadError::SizeOverflow)?;
            let stored = decode_value(
                &bytes[offset..offset + layout.bytes_per_value],
                layout,
                factor,
                source,
            )?;
            // Varian stores the quadrature lane with the opposite sign from
            // the crate's ascending-frequency, negative-exponent convention.
            if stored == 0.0 { 0.0 } else { -stored }
        } else {
            0.0
        };
        samples.push(Complex64::new(real, imaginary));
    }
    Ok(samples)
}

pub(super) fn decode_all(
    fid: &[u8],
    layout: &LayoutPlan,
    max_decode_bytes: usize,
    source: &InputSource,
) -> Result<Vec<Complex64>, ReadError> {
    let sample_count = layout
        .trace_count
        .checked_mul(layout.direct_points)
        .ok_or(ReadError::SizeOverflow)?;
    let output_bytes = sample_count
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let scratch_bytes = layout
        .direct_points
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    if output_bytes
        .checked_add(scratch_bytes)
        .is_none_or(|bytes| bytes > max_decode_bytes)
    {
        let required = output_bytes
            .checked_add(scratch_bytes)
            .ok_or(ReadError::SizeOverflow)?;
        return Err(ReadError::limit(
            crate::raw::ReadResource::WorkingBytes,
            max_decode_bytes,
            required,
        ));
    }

    let mut samples = Vec::new();
    samples
        .try_reserve_exact(sample_count)
        .map_err(|_| ReadError::allocation(output_bytes))?;
    for disk_trace in 0..layout.trace_count {
        let block = disk_trace / layout.traces_per_block;
        let trace_in_block = disk_trace % layout.traces_per_block;
        let offset = 32usize
            .checked_add(
                block
                    .checked_mul(layout.block_bytes)
                    .ok_or(ReadError::SizeOverflow)?,
            )
            .and_then(|value| value.checked_add(layout.block_header_bytes))
            .and_then(|value| value.checked_add(trace_in_block.checked_mul(layout.trace_bytes)?))
            .ok_or(ReadError::SizeOverflow)?;
        let end = offset
            .checked_add(layout.trace_bytes)
            .ok_or(ReadError::SizeOverflow)?;
        let bytes = fid
            .get(offset..end)
            .ok_or_else(|| ReadError::truncated(source.clone(), end, fid.len()))?;
        samples.extend(decode_trace(bytes, layout, block, &|| source.clone())?);
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(bytes_per_value: usize, is_float: bool, is_32_bit_integer: bool) -> LayoutPlan {
        LayoutPlan {
            traces_per_block: 1,
            stored_values: 2,
            bytes_per_value,
            trace_bytes: bytes_per_value * 2,
            block_bytes: 28 + bytes_per_value * 2,
            block_header_bytes: 28,
            direct_points: 1,
            trace_count: 1,
            is_float,
            is_32_bit_integer,
            complex: true,
            scale_factors: vec![2.0],
        }
    }

    #[test]
    fn error_sources_are_constructed_only_when_decoding_fails() {
        let layout = layout(4, true, false);
        let valid = [1_f32.to_be_bytes(), 2_f32.to_be_bytes()].concat();
        let nonfinite = [f32::NAN.to_be_bytes(), 2_f32.to_be_bytes()].concat();
        for source in [
            InputSource::memory("fid"),
            std::path::Path::new("synthetic/fid").into(),
        ] {
            let calls = std::cell::Cell::new(0usize);
            let context = || {
                calls.set(calls.get() + 1);
                source.clone()
            };
            assert_eq!(
                decode_trace(&valid, &layout, 0, &context).unwrap(),
                [Complex64::new(2.0, -4.0)]
            );
            assert_eq!(calls.get(), 0);
            for (bytes, block) in [
                (&valid[..7], 0),
                (valid.as_slice(), 1),
                (nonfinite.as_slice(), 0),
            ] {
                calls.set(0);
                let error = decode_trace(bytes, &layout, block, &context).unwrap_err();
                assert_eq!(calls.get(), 1);
                assert_eq!(error.reason().input_source(), Some(&source));
                assert!(matches!(
                    error.reason(),
                    crate::ReadErrorReason::Corrupt { .. }
                ));
            }
        }
    }

    #[test]
    fn numeric_storage_matrix_applies_scale_and_canonical_complex_sign() {
        let source = InputSource::memory("fid");
        let cases = [
            (
                layout(2, false, false),
                [1_i16.to_be_bytes(), (-2_i16).to_be_bytes()].concat(),
                Complex64::new(2.0, 4.0),
            ),
            (
                layout(4, false, true),
                [1_i32.to_be_bytes(), (-2_i32).to_be_bytes()].concat(),
                Complex64::new(2.0, 4.0),
            ),
            (
                layout(4, true, false),
                [1.5_f32.to_be_bytes(), (-2.5_f32).to_be_bytes()].concat(),
                Complex64::new(3.0, 5.0),
            ),
        ];
        for (layout, bytes, expected) in cases {
            assert_eq!(
                decode_trace(&bytes, &layout, 0, &|| source.clone()).unwrap(),
                [expected]
            );
        }
    }
}
