use nmr::formats::bruker::{Parts, read_parts};
use nmr::raw::{GroupDelayState, RawAxis, ReadError};
use std::fs;
use std::path::Path;

pub(super) fn parameter_text(byte_order: i64, data_type: i64, td: usize) -> String {
    format!(
        "##TITLE= synthetic\n\
         ##$TD= {td}\n\
         ##$PARMODE= 0\n\
         ##$AQ_mod= 3\n\
         ##$BYTORDA= {byte_order}\n\
         ##$DTYPA= {data_type}\n\
         ##$SW_h= 8000\n\
         ##$SFO1= 400.13\n\
         ##$BF1= 400.0\n\
         ##$O1= 1880\n\
         ##$NUC1= <1H>\n\
         ##$SOLVENT= <CDCl3>\n\
         ##$TE= 298.15\n\
         ##$NS= 16\n\
         ##$PULPROG= <zg30>\n\
         ##$GRPDLY= 44.75\n\
         ##END=\n"
    )
}

pub(super) fn write_dataset(directory: &Path, parameters: &str, bytes: &[u8]) {
    fs::create_dir_all(directory).unwrap();
    fs::write(directory.join("acqus"), parameters).unwrap();
    fs::write(directory.join("fid"), bytes).unwrap();
}

pub(super) fn read_from_parts(
    data: &[u8],
    acquisition_parameters: &[&str],
    nuslist: Option<&str>,
) -> Result<nmr::raw::RawDataset, ReadError> {
    let parts = Parts::new(data, acquisition_parameters).allow_experimental_vendor_semantics(true);
    read_parts(match nuslist {
        Some(value) => parts.nuslist(value),
        None => parts,
    })
}

pub(super) fn pending_delay(axis: &RawAxis) -> Option<f64> {
    match axis.group_delay() {
        GroupDelayState::Pending(delay) => Some(delay.delay_points()),
        _ => None,
    }
}

pub(super) fn two_dimensional_parameters(
    fn_type: i64,
    fn_mode: i64,
    indirect_td: usize,
    nus_td: usize,
    continuous: bool,
) -> (String, String) {
    let block = if continuous {
        "##$GO_block_size= <continuous>\n"
    } else {
        ""
    };
    let acqus = format!(
        "##TITLE= synthetic 2D\n\
         ##$PARMODE= 1\n\
         ##$AQSEQ= 0\n\
         ##$FnTYPE= {fn_type}\n\
         ##$AQ_mod= 3\n\
         ##$TD= 4\n\
         ##$BYTORDA= 0\n\
         ##$DTYPA= 0\n\
         {block}\
         ##$SW_h= 8000\n\
         ##$SFO1= 400.13\n\
         ##$BF1= 400.0\n\
         ##$O1= 1880\n\
         ##$NUC1= <1H>\n\
         ##$NS= 8\n\
         ##$PULPROG= <synthetic2d>\n\
         ##END=\n"
    );
    let acqu2s = format!(
        "##TITLE= synthetic indirect\n\
         ##$TD= {indirect_td}\n\
         ##$FnMODE= {fn_mode}\n\
         ##$NusTD= {nus_td}\n\
         ##$SW_h= 1000\n\
         ##$SFO1= 100.62\n\
         ##$BF1= 100.0\n\
         ##$O1= 2200\n\
         ##$NUC1= <13C>\n\
         ##END=\n"
    );
    (acqus, acqu2s)
}

pub(super) fn encoded_ser(rows: usize, continuous: bool) -> Vec<u8> {
    let stride = if continuous { 16 } else { 1024 };
    let mut bytes = vec![0; rows * stride];
    for row in 0..rows {
        for scalar in 0..4 {
            let value = (row as i32 * 10 + scalar + 1).to_le_bytes();
            let start = row * stride + scalar as usize * 4;
            bytes[start..start + 4].copy_from_slice(&value);
        }
    }
    bytes
}

pub(super) fn three_dimensional_parameters(aqseq: i64) -> (String, String, String) {
    let (mut acqus, acqu2s) = two_dimensional_parameters(0, 6, 4, 4, true);
    acqus = acqus
        .replace("##$PARMODE= 1", "##$PARMODE= 2")
        .replace("##$AQSEQ= 0", &format!("##$AQSEQ= {aqseq}"));
    let acqu3s = "##TITLE= synthetic slow indirect\n\
                   ##$TD= 4\n\
                   ##$FnMODE= 5\n\
                   ##$SW_h= 2000\n\
                   ##$SFO1= 50.68\n\
                   ##$BF1= 50.0\n\
                   ##$O1= 1000\n\
                   ##$NUC1= <15N>\n\
                   ##END=\n"
        .to_owned();
    (acqus, acqu2s, acqu3s)
}
