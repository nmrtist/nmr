use super::ReportError;
use std::io::Write;

pub(super) struct Json<'a> {
    pub(super) writer: &'a mut dyn Write,
    pub(super) written: usize,
    pub(super) limit: usize,
}
pub(super) trait Encode {
    fn encode(&self, json: &mut Json<'_>) -> Result<(), ReportError>;
}
impl Json<'_> {
    pub(super) fn raw(&mut self, value: &str) -> Result<(), ReportError> {
        let required = self
            .written
            .checked_add(value.len())
            .ok_or(ReportError::Invalid("report size overflow"))?;
        if required > self.limit {
            return Err(ReportError::LimitExceeded {
                required,
                limit: self.limit,
            });
        }
        self.writer.write_all(value.as_bytes())?;
        self.written = required;
        Ok(())
    }
    pub(super) fn object(&mut self, fields: &[(&str, &dyn Encode)]) -> Result<(), ReportError> {
        self.raw("{")?;
        for (index, (key, value)) in fields.iter().enumerate() {
            if index > 0 {
                self.raw(",")?;
            }
            key.encode(self)?;
            self.raw(":")?;
            value.encode(self)?;
        }
        self.raw("}")
    }
}
macro_rules! object { ($j:expr, $($key:literal => $value:expr),* $(,)?) => { $j.object(&[$(($key, &$value as &dyn Encode)),*]) }; }
impl<T: Encode + ?Sized> Encode for &T {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        (*self).encode(j)
    }
}
impl Encode for str {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        j.raw("\"")?;
        for character in self.chars() {
            match character {
                '"' => j.raw("\\\"")?,
                '\\' => j.raw("\\\\")?,
                '\n' => j.raw("\\n")?,
                '\r' => j.raw("\\r")?,
                '\t' => j.raw("\\t")?,
                value if value <= '\u{1f}' => j.raw(&format!("\\u{:04x}", value as u32))?,
                value => j.raw(value.encode_utf8(&mut [0; 4]))?,
            }
        }
        j.raw("\"")
    }
}
impl Encode for String {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        self.as_str().encode(j)
    }
}
impl Encode for bool {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        j.raw(if *self { "true" } else { "false" })
    }
}
macro_rules! integers { ($($ty:ty),*) => { $(impl Encode for $ty { fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> { j.raw(&self.to_string()) } })* }; }
integers!(usize, u64, i64, u32, i32, i8);
impl Encode for f64 {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        if !self.is_finite() {
            return Err(ReportError::NonFinite);
        }
        j.raw(&self.to_string())
    }
}
impl<T: Encode> Encode for Option<T> {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Some(v) => v.encode(j),
            None => j.raw("null"),
        }
    }
}
impl<T: Encode> Encode for [T] {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        j.raw("[")?;
        for (i, v) in self.iter().enumerate() {
            if i > 0 {
                j.raw(",")?;
            }
            v.encode(j)?;
        }
        j.raw("]")
    }
}
impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        self.as_slice().encode(j)
    }
}
impl Encode for u8 {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        j.raw(&self.to_string())
    }
}
impl Encode for std::borrow::Cow<'_, str> {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        self.as_ref().encode(j)
    }
}
