use crate::raw_support::*;
pub(super) fn jeol_parameter_record(
    name: &str,
    value: nmr::formats::jeol::ParameterValue,
    base_unit: u8,
) -> Vec<u8> {
    let mut raw_units = [0_u8; 10];
    raw_units[0] = 1;
    raw_units[1] = base_unit;
    jeol_parameter_record_with_units(name, value, raw_units)
}

pub(super) fn jeol_parameter_record_with_units(
    name: &str,
    value: nmr::formats::jeol::ParameterValue,
    raw_units: [u8; 10],
) -> Vec<u8> {
    let mut record = vec![0_u8; 64];
    record[6..16].copy_from_slice(&raw_units);
    let type_code = match value {
        nmr::formats::jeol::ParameterValue::String(value) => {
            record[16..16 + value.len()].copy_from_slice(value.as_bytes());
            0_i32
        }
        nmr::formats::jeol::ParameterValue::Integer(value) => {
            record[16..20].copy_from_slice(&value.to_le_bytes());
            1
        }
        nmr::formats::jeol::ParameterValue::Float(value) => {
            record[16..24].copy_from_slice(&value.to_le_bytes());
            2
        }
        _ => panic!("unsupported synthetic JEOL parameter value"),
    };
    record[32..36].copy_from_slice(&type_code.to_le_bytes());
    record[36..36 + name.len()].copy_from_slice(name.as_bytes());
    record
}

pub(super) fn jeol_with_parameter_records(original: &[u8], records: &[Vec<u8>]) -> Vec<u8> {
    let old_data_start = u32::from_be_bytes(original[1284..1288].try_into().unwrap()) as usize;
    let payload = &original[old_data_start..];
    let parameter_length = 16 + records.len() * 64;
    let data_start = 1360 + parameter_length;
    let mut bytes = vec![0_u8; data_start + payload.len()];
    bytes[..1360].copy_from_slice(&original[..1360]);
    put_be_u32(&mut bytes, 1212, 1360);
    put_be_u32(&mut bytes, 1216, parameter_length as u32);
    put_be_u32(&mut bytes, 1284, data_start as u32);
    put_le_u32(&mut bytes, 1360, 64);
    put_le_u32(&mut bytes, 1364, 0);
    put_le_u32(&mut bytes, 1368, records.len() as u32);
    put_le_u32(&mut bytes, 1372, parameter_length as u32);
    for (index, record) in records.iter().enumerate() {
        let start = 1376 + index * 64;
        bytes[start..start + 64].copy_from_slice(record);
    }
    bytes[data_start..].copy_from_slice(payload);
    bytes
}

pub(super) fn jeol_fixture_with_parameters() -> Vec<u8> {
    let mut original = jeol_fixture_f64();
    let records = [
        jeol_parameter_record(
            "x_domain",
            nmr::formats::jeol::ParameterValue::String("Proton".into()),
            0,
        ),
        jeol_parameter_record(
            "x_sweep",
            nmr::formats::jeol::ParameterValue::Float(4000.0),
            13,
        ),
        jeol_parameter_record(
            "x_freq",
            nmr::formats::jeol::ParameterValue::Float(400_000_000.0),
            13,
        ),
        jeol_parameter_record(
            "x_offset",
            nmr::formats::jeol::ParameterValue::Float(5.0),
            26,
        ),
        jeol_parameter_record(
            "solvent",
            nmr::formats::jeol::ParameterValue::String("D2O".into()),
            0,
        ),
        jeol_parameter_record("scans", nmr::formats::jeol::ParameterValue::Integer(16), 0),
        jeol_parameter_record(
            "experiment",
            nmr::formats::jeol::ParameterValue::String("proton.jxp".into()),
            0,
        ),
    ];
    original[48..61].copy_from_slice(b"Synthetic FID");
    jeol_with_parameter_records(&original, &records)
}

pub(super) fn jeol_array_parameter_fixture(parameter_units: [u8; 10]) -> Vec<u8> {
    let mut original = jeol_fixture_small_2d();
    original[34] = 0x11; // milli, first power
    original[35] = 31; // tesla base unit in the abbreviated axis header
    original[280..288].copy_from_slice(&20.0_f64.to_be_bytes());
    original[344..352].copy_from_slice(&80.0_f64.to_be_bytes());
    let records = [
        jeol_parameter_record(
            "x_domain",
            nmr::formats::jeol::ParameterValue::String("Fluorine19".into()),
            0,
        ),
        jeol_parameter_record(
            "y_acq",
            nmr::formats::jeol::ParameterValue::String("g".into()),
            0,
        ),
        jeol_parameter_record_with_units(
            "g",
            nmr::formats::jeol::ParameterValue::Float(20.0),
            parameter_units,
        ),
        jeol_parameter_record_with_units(
            "delta",
            nmr::formats::jeol::ParameterValue::Float(5.0),
            [0x11, 28, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        jeol_parameter_record(
            "delta_large",
            nmr::formats::jeol::ParameterValue::Float(0.1),
            28,
        ),
        jeol_parameter_record_with_units(
            "tau",
            nmr::formats::jeol::ParameterValue::Float(2.0),
            [0x11, 28, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
        jeol_parameter_record(
            "grad_shape",
            nmr::formats::jeol::ParameterValue::String("SQUARE".into()),
            0,
        ),
    ];
    jeol_with_parameter_records(&original, &records)
}

pub(super) fn jeol_gradient_array_fixture() -> Vec<u8> {
    jeol_array_parameter_fixture([0x11, 31, 0x0f, 19, 0, 0, 0, 0, 0, 0])
}

pub(super) fn jeol_time_array_fixture() -> Vec<u8> {
    let mut original = jeol_fixture_small_2d();
    original[34] = 0x11; // milli, first power
    original[35] = 28; // seconds
    original[280..288].copy_from_slice(&20.0_f64.to_be_bytes());
    original[344..352].copy_from_slice(&80.0_f64.to_be_bytes());
    jeol_with_parameter_records(
        &original,
        &[
            jeol_parameter_record(
                "y_acq",
                nmr::formats::jeol::ParameterValue::String("total_echo".into()),
                0,
            ),
            jeol_parameter_record_with_units(
                "total_echo",
                nmr::formats::jeol::ParameterValue::Float(20.0),
                [0x11, 28, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            jeol_parameter_record(
                "experiment",
                nmr::formats::jeol::ParameterValue::String("spin_echo_t2.jxp".into()),
                0,
            ),
        ],
    )
}

pub(super) fn jeol_fixture_f64() -> Vec<u8> {
    let mut bytes = vec![0_u8; 1360 + 4 * 2 * 8 * 2];
    bytes[..8].copy_from_slice(b"JEOL.NMR");
    bytes[8] = 1; // little-endian payload
    bytes[9] = 1;
    bytes[10..12].copy_from_slice(&2_u16.to_be_bytes());
    bytes[12] = 1;
    bytes[13] = 1; // direct axis exists
    bytes[14] = 1; // float64, one_d
    bytes[24] = 3; // complex direct axis
    put_be_u32(&mut bytes, 176, 4);
    put_be_u32(&mut bytes, 240, 3);
    put_be_u32(&mut bytes, 1284, 1360);
    let real = [1.0_f64, 2.0, 3.0, 4.0];
    let imag = [10.0_f64, 20.0, 30.0, 40.0];
    for (i, value) in real.into_iter().chain(imag).enumerate() {
        let start = 1360 + i * 8;
        bytes[start..start + 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub(super) fn jeol_fixture_small_2d() -> Vec<u8> {
    let mut bytes = vec![0_u8; 1360 + 8 * 4 * 2 * 8];
    bytes[..8].copy_from_slice(b"JEOL.NMR");
    bytes[8] = 1; // little-endian payload
    bytes[9] = 1;
    bytes[10..12].copy_from_slice(&2_u16.to_be_bytes());
    bytes[12] = 2;
    bytes[13] = 3; // both axes exist
    bytes[14] = 12; // float64, small_two_d
    bytes[24] = 3; // complex direct axis
    bytes[25] = 1; // real array-parameter axis
    put_be_u32(&mut bytes, 176, 8);
    put_be_u32(&mut bytes, 180, 4);
    put_be_u32(&mut bytes, 240, 5); // direct valid points: 0..5
    put_be_u32(&mut bytes, 244, 2); // indirect valid points: 0..2
    put_be_u32(&mut bytes, 1284, 1360);

    // JDF stores each 4x4 tile in row-major tile order. The second tile
    // covers columns 4..7 of the canonical 4x8 output.
    let mut raw = Vec::new();
    for section in 0..2 {
        for tile_column in 0..2 {
            for row in 0..4 {
                for local_column in 0..4 {
                    let column = tile_column * 4 + local_column;
                    let value = section as f64 * 1000.0 + row as f64 * 10.0 + column as f64;
                    raw.push(value);
                }
            }
        }
    }
    for (index, value) in raw.into_iter().enumerate() {
        put_le_f64(&mut bytes, 1360 + index * 8, value);
    }
    bytes
}

pub(super) fn jeol_fixture_hypercomplex_2d() -> Vec<u8> {
    let mut bytes = vec![0_u8; 1360 + 4 * 4 * 4 * 8];
    bytes[..8].copy_from_slice(b"JEOL.NMR");
    bytes[8] = 1; // little-endian payload
    bytes[9] = 1;
    bytes[10..12].copy_from_slice(&2_u16.to_be_bytes());
    bytes[12] = 2;
    bytes[13] = 3; // both axes exist
    bytes[14] = 12; // float64, small_two_d
    bytes[24] = 3; // complex direct axis
    bytes[25] = 3; // complex indirect axis
    put_be_u32(&mut bytes, 176, 4);
    put_be_u32(&mut bytes, 180, 4);
    put_be_u32(&mut bytes, 240, 2); // direct valid points: 0..2
    put_be_u32(&mut bytes, 244, 1); // indirect valid points: 0..1
    put_be_u32(&mut bytes, 1284, 1360);

    let mut index = 0;
    for plane in 0..4 {
        for row in 0..4 {
            for column in 0..4 {
                let value = plane as f64 * 100.0 + row as f64 * 10.0 + column as f64;
                put_le_f64(&mut bytes, 1360 + index * 8, value);
                index += 1;
            }
        }
    }
    bytes
}

pub(super) fn jeol_fixture_hypercomplex_nus() -> Vec<u8> {
    let original = jeol_fixture_hypercomplex_2d();
    let records = [
        jeol_parameter_record(
            "y_orig_points",
            nmr::formats::jeol::ParameterValue::Integer(8),
            0,
        ),
        jeol_parameter_record(
            "y_sweep",
            nmr::formats::jeol::ParameterValue::Float(1000.0),
            13,
        ),
    ];
    let parameterized = jeol_with_parameter_records(&original, &records);
    let old_data_start = u32::from_be_bytes(parameterized[1284..1288].try_into().unwrap()) as usize;
    let mut bytes = vec![0_u8; parameterized.len() + 32];
    bytes[..old_data_start].copy_from_slice(&parameterized[..old_data_start]);
    bytes[old_data_start + 32..].copy_from_slice(&parameterized[old_data_start..]);
    bytes[34] = 1;
    bytes[35] = 28;
    put_be_u32(&mut bytes, 244, 3);
    put_be_u32(&mut bytes, 1224, old_data_start as u32);
    put_be_u32(&mut bytes, 1256, 32);
    put_be_u32(&mut bytes, 1284, (old_data_start + 32) as u32);
    for (index, value) in [0.0_f64, 0.001, 0.003, 0.007].into_iter().enumerate() {
        let start = old_data_start + index * 8;
        bytes[start..start + 8].copy_from_slice(&value.to_be_bytes());
    }
    bytes
}
