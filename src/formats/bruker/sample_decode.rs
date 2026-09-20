use super::context::ErrorContext;
use super::storage::DirectStorage;

use crate::Complex64;
use crate::raw::ReadError;

pub(super) fn decode_complex_pairs(
    bytes: &[u8],
    storage: DirectStorage,
    role: &'static str,
    source: &(impl ErrorContext + ?Sized),
    capacity: usize,
) -> Result<Vec<Complex64>, ReadError> {
    let requested_bytes = capacity
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| ReadError::allocation(requested_bytes))?;
    output.resize(capacity, Complex64::new(0.0, 0.0));
    decode_complex_pairs_into(bytes, storage, role, source, &mut output)?;
    Ok(output)
}

pub(super) fn decode_complex_pairs_into(
    bytes: &[u8],
    storage: DirectStorage,
    role: &'static str,
    source: &(impl ErrorContext + ?Sized),
    output: &mut [Complex64],
) -> Result<(), ReadError> {
    let sample_bytes = storage.sample.bytes();
    let expected = output
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_mul(sample_bytes))
        .ok_or(ReadError::SizeOverflow)?;
    if bytes.len() != expected {
        return Err(ReadError::corrupt(
            source.input_source(),
            format!("{role} byte length does not match its declared sample count"),
        ));
    }
    for (pair, output) in bytes.chunks_exact(2 * sample_bytes).zip(output) {
        let value = Complex64::new(
            storage.sample.read(&pair[..sample_bytes], storage.endian),
            storage.sample.read(&pair[sample_bytes..], storage.endian),
        );
        if !value.re.is_finite() || !value.im.is_finite() {
            return Err(ReadError::corrupt(
                source.input_source(),
                format!("{role} binary sample is not finite"),
            ));
        }
        *output = value;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::bruker::storage::{Endian, SampleFormat};
    use crate::raw::InputSource;

    #[test]
    fn numeric_and_endian_matrix_decodes_public_complex_order() {
        let source = InputSource::memory("fid");
        let cases = [
            (
                SampleFormat::I32,
                Endian::Little,
                [1_i32.to_le_bytes(), (-2_i32).to_le_bytes()].concat(),
            ),
            (
                SampleFormat::I32,
                Endian::Big,
                [1_i32.to_be_bytes(), (-2_i32).to_be_bytes()].concat(),
            ),
        ];
        for (sample, endian, bytes) in cases {
            let decoded = decode_complex_pairs(
                &bytes,
                DirectStorage {
                    td: 2,
                    endian,
                    sample,
                },
                "fid",
                &source,
                1,
            )
            .unwrap();
            assert_eq!(decoded, [Complex64::new(1.0, -2.0)]);
        }

        for (endian, bytes) in [
            (
                Endian::Little,
                [1.5_f64.to_le_bytes(), (-2.5_f64).to_le_bytes()].concat(),
            ),
            (
                Endian::Big,
                [1.5_f64.to_be_bytes(), (-2.5_f64).to_be_bytes()].concat(),
            ),
        ] {
            let decoded = decode_complex_pairs(
                &bytes,
                DirectStorage {
                    td: 2,
                    endian,
                    sample: SampleFormat::F64,
                },
                "fid",
                &source,
                1,
            )
            .unwrap();
            assert_eq!(decoded, [Complex64::new(1.5, -2.5)]);
        }
    }

    #[test]
    fn non_finite_binary_sample_is_structural_corruption_not_a_parameter_error() {
        let source = InputSource::memory("fid");
        let bytes = [f64::NAN.to_le_bytes(), 0.0_f64.to_le_bytes()].concat();
        let error = decode_complex_pairs(
            &bytes,
            DirectStorage {
                td: 2,
                endian: Endian::Little,
                sample: SampleFormat::F64,
            },
            "fid",
            &source,
            1,
        )
        .unwrap_err();
        assert!(matches!(
            error.reason(),
            crate::raw::ReadErrorReason::Corrupt { input, .. }
                if input.role() == Some("fid")
        ));
    }
}
