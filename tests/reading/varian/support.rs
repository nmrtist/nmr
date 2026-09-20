use crate::raw_support::*;
pub(super) fn varian_fixture() -> Vec<u8> {
    let mut bytes = vec![0_u8; 32 + 28 + 16];
    put_be_u32(&mut bytes, 0, 1); // blocks
    put_be_u32(&mut bytes, 4, 1); // traces per block
    put_be_u32(&mut bytes, 8, 4); // R/I values
    put_be_u32(&mut bytes, 12, 4); // bytes per value
    put_be_u32(&mut bytes, 16, 16); // trace bytes
    put_be_u32(&mut bytes, 20, 44); // block bytes including header
    bytes[24..26].copy_from_slice(&1_u16.to_be_bytes()); // modern status-bit layout
    bytes[26..28].copy_from_slice(&149_u16.to_be_bytes()); // data + int32 + complex + acquisition
    put_be_u32(&mut bytes, 28, 1); // one block header
    bytes[32..34].copy_from_slice(&1_i16.to_be_bytes()); // scale = 1
    bytes[60..64].copy_from_slice(&1_i32.to_be_bytes());
    bytes[64..68].copy_from_slice(&2_i32.to_be_bytes());
    bytes[68..72].copy_from_slice(&3_i32.to_be_bytes());
    bytes[72..76].copy_from_slice(&4_i32.to_be_bytes());
    bytes
}

pub(super) fn varian_numbered_traces(trace_count: usize) -> Vec<u8> {
    let block_bytes = 28 + 8;
    let mut bytes = vec![0_u8; 32 + trace_count * block_bytes];
    put_be_u32(&mut bytes, 0, trace_count as u32);
    put_be_u32(&mut bytes, 4, 1);
    put_be_u32(&mut bytes, 8, 2);
    put_be_u32(&mut bytes, 12, 4);
    put_be_u32(&mut bytes, 16, 8);
    put_be_u32(&mut bytes, 20, block_bytes as u32);
    bytes[24..26].copy_from_slice(&1_u16.to_be_bytes());
    bytes[26..28].copy_from_slice(&149_u16.to_be_bytes());
    put_be_u32(&mut bytes, 28, 1);
    for trace in 0..trace_count {
        let block = 32 + trace * block_bytes;
        bytes[block + 4..block + 6].copy_from_slice(&(trace as i16).to_be_bytes());
        bytes[block + 8..block + 12].copy_from_slice(&(trace as i32 + 10).to_be_bytes());
        bytes[block + 28..block + 32].copy_from_slice(&(trace as i32).to_be_bytes());
        bytes[block + 32..block + 36].copy_from_slice(&(-(trace as i32)).to_be_bytes());
    }
    bytes
}

pub(super) fn procpar(records: &[(&str, &[&str])]) -> String {
    let mut text = String::new();
    for (name, values) in records {
        let real = values.iter().all(|value| value.parse::<f64>().is_ok());
        let basic_type = if real { 1 } else { 2 };
        text.push_str(&format!(
            "{name} {basic_type} {basic_type} 4 0 0 2 1 0 1 64\n"
        ));
        text.push_str(&values.len().to_string());
        for value in *values {
            text.push(' ');
            if value.is_empty() || value.contains(',') || value.chars().any(char::is_whitespace) {
                text.push('"');
                text.push_str(value);
                text.push('"');
            } else {
                text.push_str(value);
            }
        }
        text.push_str("\n0\n");
    }
    text
}
