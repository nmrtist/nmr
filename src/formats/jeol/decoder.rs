use super::header::BodyEndian;
use super::header::Precision;

use crate::ReadError;
use crate::raw::InputSource;

pub(super) fn decode_value(
    bytes: &[u8],
    precision: Precision,
    endian: BodyEndian,
    source: &InputSource,
) -> Result<f64, ReadError> {
    if bytes.len() != precision.size() {
        return Err(ReadError::corrupt(
            source.clone(),
            "JEOL sample width disagrees with the validated layout",
        ));
    }
    let value = match (precision, endian) {
        (Precision::F32, BodyEndian::Little) => {
            f32::from_le_bytes(bytes.try_into().unwrap()) as f64
        }
        (Precision::F32, BodyEndian::Big) => f32::from_be_bytes(bytes.try_into().unwrap()) as f64,
        (Precision::F64, BodyEndian::Little) => f64::from_le_bytes(bytes.try_into().unwrap()),
        (Precision::F64, BodyEndian::Big) => f64::from_be_bytes(bytes.try_into().unwrap()),
    };
    if !value.is_finite() {
        return Err(ReadError::corrupt(source.clone(), "non-finite JEOL sample"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_and_endian_matrix_decodes_values() {
        let source = InputSource::memory("jdf");
        let cases: [(Precision, BodyEndian, Vec<u8>, f64); 4] = [
            (
                Precision::F32,
                BodyEndian::Little,
                1.5_f32.to_le_bytes().to_vec(),
                1.5,
            ),
            (
                Precision::F32,
                BodyEndian::Big,
                1.5_f32.to_be_bytes().to_vec(),
                1.5,
            ),
            (
                Precision::F64,
                BodyEndian::Little,
                (-2.5_f64).to_le_bytes().to_vec(),
                -2.5,
            ),
            (
                Precision::F64,
                BodyEndian::Big,
                (-2.5_f64).to_be_bytes().to_vec(),
                -2.5,
            ),
        ];
        for (precision, endian, bytes, expected) in cases {
            assert_eq!(
                decode_value(&bytes, precision, endian, &source).unwrap(),
                expected
            );
        }
    }
}
