use nmr::{ReadError, ReadErrorKind, ReadErrorReason, ReadResource};
use std::fs;

pub(super) fn processed_parameters(si: usize) -> String {
    processed_parameters_with_storage(si, 0, 0, 1)
}

pub(super) fn processed_parameters_with_storage(
    si: usize,
    dtypp: i32,
    bytordp: i32,
    nc_proc: i32,
) -> String {
    format!(
        "##TITLE= fixture\n\
         ##$SI= {si}\n\
         ##$DTYPP= {dtypp}\n\
         ##$BYTORDP= {bytordp}\n\
         ##$NC_proc= {nc_proc}\n\
         ##$SW_p= 1200\n\
         ##$SF= 400\n\
         ##$OFFSET= 10\n\
         ##$AXNUC= <1H>\n\
         ##$EXP= <exp-name>\n"
    )
}

pub(super) fn write_i32(path: &std::path::Path, values: &[i32]) {
    let bytes: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    fs::write(path, bytes).unwrap();
}

pub(super) fn bruker_raw_1d_parameters() -> &'static str {
    "##TITLE= raw 1D\n\
     ##$TD= 4\n\
     ##$PARMODE= 0\n\
     ##$AQ_mod= 3\n\
     ##$BYTORDA= 0\n\
     ##$DTYPA= 0\n\
     ##$SW_h= 8000\n\
     ##$SFO1= 400.13\n\
     ##$NUC1= <1H>\n\
     ##END=\n"
}

pub(super) fn bruker_raw_2d_parameters() -> (String, String) {
    let direct = "##TITLE= raw 2D\n\
                  ##$PARMODE= 1\n\
                  ##$AQSEQ= 0\n\
                  ##$FnTYPE= 0\n\
                  ##$AQ_mod= 3\n\
                  ##$TD= 4\n\
                  ##$BYTORDA= 0\n\
                  ##$DTYPA= 0\n\
                  ##$GO_block_size= <continuous>\n\
                  ##$SW_h= 8000\n\
                  ##$SFO1= 400.13\n\
                  ##$NUC1= <1H>\n\
                  ##END=\n"
        .to_owned();
    let indirect = "##TITLE= raw indirect\n\
                    ##$TD= 2\n\
                    ##$FnMODE= 5\n\
                    ##$SW_h= 1000\n\
                    ##$SFO1= 100.62\n\
                    ##$NUC1= <13C>\n\
                    ##END=\n"
        .to_owned();
    (direct, indirect)
}

pub(super) fn write_complete_bruker_raw_1d(root: &std::path::Path) {
    fs::write(root.join("acqus"), bruker_raw_1d_parameters()).unwrap();
    write_i32(&root.join("fid"), &[1, 2, 3, 4]);
}

pub(super) fn write_complete_bruker_raw_2d(root: &std::path::Path) {
    let (direct, indirect) = bruker_raw_2d_parameters();
    fs::write(root.join("acqus"), direct).unwrap();
    fs::write(root.join("acqu2s"), indirect).unwrap();
    write_i32(&root.join("ser"), &[1, 2, 3, 4, 5, 6, 7, 8]);
}

pub(super) fn encoded_processed(values: &[f64], dtypp: i32, bytordp: i32) -> Vec<u8> {
    values
        .iter()
        .flat_map(|&value| match (dtypp, bytordp) {
            (0, 0) => (value as i32).to_le_bytes().to_vec(),
            (0, 1) => (value as i32).to_be_bytes().to_vec(),
            (2, 0) => value.to_le_bytes().to_vec(),
            (2, 1) => value.to_be_bytes().to_vec(),
            _ => unreachable!(),
        })
        .collect()
}

pub(super) fn put_be_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

pub(super) fn put_le_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn jeol_string_parameter(name: &str, value: &str) -> Vec<u8> {
    let mut record = vec![0_u8; 64];
    record[6] = 1;
    record[16..16 + value.len()].copy_from_slice(value.as_bytes());
    record[32..36].copy_from_slice(&0_i32.to_le_bytes());
    record[36..36 + name.len()].copy_from_slice(name.as_bytes());
    record
}

pub(super) fn jeol_float_parameter(name: &str, value: f64, base_unit: u8) -> Vec<u8> {
    let mut record = vec![0_u8; 64];
    record[6] = 1;
    record[7] = base_unit;
    record[16..24].copy_from_slice(&value.to_le_bytes());
    record[32..36].copy_from_slice(&2_i32.to_le_bytes());
    record[36..36 + name.len()].copy_from_slice(name.as_bytes());
    record
}

pub(super) fn processed_jeol(axis_type: u8, values: &[f64]) -> Vec<u8> {
    let mut bytes = vec![0_u8; 1360 + values.len() * 8];
    bytes[..8].copy_from_slice(b"JEOL.NMR");
    bytes[8] = 1;
    bytes[9] = 1;
    bytes[10..12].copy_from_slice(&2_u16.to_be_bytes());
    bytes[12] = 1;
    bytes[13] = 1;
    bytes[14] = 1;
    bytes[24] = axis_type;
    bytes[32] = 1;
    bytes[33] = 26;
    put_be_u32(&mut bytes, 176, 4);
    put_be_u32(&mut bytes, 208, 1);
    put_be_u32(&mut bytes, 240, 2);
    bytes[272..280].copy_from_slice(&10.0_f64.to_be_bytes());
    bytes[336..344].copy_from_slice(&7.0_f64.to_be_bytes());
    put_be_u32(&mut bytes, 1284, 1360);
    for (index, value) in values.iter().copied().enumerate() {
        let start = 1360 + index * 8;
        bytes[start..start + 8].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub(super) fn processed_jeol_with_axis_evidence(axis_type: u8, values: &[f64]) -> Vec<u8> {
    let original = processed_jeol(axis_type, values);
    let records = [
        jeol_string_parameter("x_domain", "Proton"),
        jeol_float_parameter("x_sweep", 4000.0, 13),
        jeol_float_parameter("x_freq", 400_000_000.0, 13),
    ];
    let parameter_length = 16 + records.len() * 64;
    let data_start = 1360 + parameter_length;
    let payload = &original[1360..];
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

pub(super) fn assert_limit(error: ReadError, expected: ReadResource) {
    assert_eq!(error.kind(), ReadErrorKind::LimitExceeded);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::LimitExceeded { resource, .. } if *resource == expected
    ));
}

pub(super) fn compressed_jcamp(encoded: &str, points: usize) -> String {
    format!(
        "##TITLE=Compressed\n\
         ##JCAMP-DX=5.00\n\
         ##DATA TYPE=NMR SPECTRUM\n\
         ##XUNITS=HZ\n\
         ##YUNITS=ARBITRARY UNITS\n\
         ##XFACTOR=1\n\
         ##YFACTOR=1\n\
         ##FIRSTX=0\n\
         ##LASTX={}\n\
         ##NPOINTS={points}\n\
         ##DELTAX=1\n\
         ##.OBSERVE FREQUENCY=400\n\
         ##.OBSERVE NUCLEUS=<1H>\n\
         ##XYDATA=(X++(Y..Y))\n\
         0 {encoded}\n\
         ##END=\n",
        points - 1
    )
}
