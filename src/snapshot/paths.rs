use super::{
    SnapshotError,
    wire::{Budget, Codec, Value, fields, next, record},
};
use std::{borrow::Cow, path::PathBuf};

impl Codec for PathBuf {
    fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
        if let Some(path) = self.to_str() {
            return record(
                "path.utf8.v1",
                vec![Value::Text(Cow::Borrowed(path))],
                budget,
            );
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let count = self.as_os_str().encode_wide().count();
            budget.reserve::<u16>(count)?;
            budget.reserve::<u16>(count)?;
            let units: Vec<u16> = self.as_os_str().encode_wide().collect();
            record(
                "path.windows-utf16.v1",
                vec![Value::Bytes(Cow::Owned(
                    units.into_iter().flat_map(u16::to_le_bytes).collect(),
                ))],
                budget,
            )
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            record(
                "path.unix-bytes.v1",
                vec![Value::Bytes(Cow::Borrowed(self.as_os_str().as_bytes()))],
                budget,
            )
        }
        #[cfg(not(any(windows, unix)))]
        {
            Err(SnapshotError::PathEncoding)
        }
    }
    fn from_wire(
        value: Value<'static>,
        budget: &mut Budget<'_, '_>,
    ) -> Result<Self, SnapshotError> {
        let Value::Record(tag, values) = value else {
            return Err(SnapshotError::Structure);
        };
        let tag = tag.into_owned();
        let mut f = fields(Value::Record(Cow::Owned(tag.clone()), values), &tag, 1)?;
        match tag.as_str() {
            "path.utf8.v1" => {
                let path: String = next(&mut f, budget)?;
                Ok(PathBuf::from(path))
            }
            #[cfg(windows)]
            "path.windows-utf16.v1" => {
                use std::os::windows::ffi::OsStringExt;
                let bytes: Vec<u8> = next(&mut f, budget)?;
                if bytes.len() % 2 != 0 {
                    return Err(SnapshotError::Structure);
                }
                budget.reserve::<u16>(bytes.len() / 2)?;
                budget.reserve::<u8>(bytes.len())?;
                let units: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect();
                Ok(PathBuf::from(std::ffi::OsString::from_wide(&units)))
            }
            #[cfg(unix)]
            "path.unix-bytes.v1" => {
                use std::os::unix::ffi::OsStringExt;
                let bytes: Vec<u8> = next(&mut f, budget)?;
                Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
            }
            #[cfg(not(windows))]
            "path.windows-utf16.v1" => Err(SnapshotError::PathEncoding),
            #[cfg(not(unix))]
            "path.unix-bytes.v1" => Err(SnapshotError::PathEncoding),
            _ => Err(SnapshotError::Structure),
        }
    }
}
