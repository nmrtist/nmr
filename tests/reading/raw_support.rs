use nmr::raw::{RawDataset as Acquisition, RawFormat as Format, ReadError, ReadErrorKind};

pub(crate) enum DatasetParts<'a> {
    Jeol {
        jdf: &'a [u8],
    },
    Varian {
        fid: &'a [u8],
        procpar: &'a str,
        schedule: Option<&'a str>,
    },
}

pub(crate) fn read_from_parts(parts: DatasetParts<'_>) -> Result<Acquisition, ReadError> {
    match parts {
        DatasetParts::Jeol { jdf } => nmr::formats::jeol::read_parts(
            nmr::formats::jeol::Parts::new(jdf).allow_experimental_vendor_semantics(true),
        ),
        DatasetParts::Varian {
            fid,
            procpar,
            schedule,
        } => {
            let parts = nmr::formats::varian::Parts::new(fid, procpar);
            nmr::formats::varian::read_parts(match schedule {
                Some(value) => parts.sampling_schedule(value),
                None => parts,
            })
        }
    }
}

pub(crate) fn put_be_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

pub(crate) fn put_be_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
}

pub(crate) fn put_le_f64(bytes: &mut [u8], offset: usize, value: f64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn put_le_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn assert_vendor_error(error: &ReadError, format: Format, category: ReadErrorKind) {
    assert_eq!(
        error.format(),
        Some(nmr::Format::Raw(format)),
        "unexpected error: {error:?}"
    );
    assert_eq!(error.kind(), category, "unexpected error: {error:?}");
}
