use super::{
    SnapshotError,
    wire::{Budget, Value},
};
use crate::{Complex64, ExecutionContext};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    io::{Read, Write},
};

const BLOCK: usize = 4096;
const MAGIC: &[u8; 8] = b"NMRSS001";
const COMMIT: &[u8; 8] = b"NMRDONE1";

pub(super) fn size(
    value: &Value<'_>,
    control: &mut ExecutionContext<'_>,
    depth: usize,
    max_depth: usize,
) -> Result<u64, SnapshotError> {
    control.check_cancelled()?;
    if depth > max_depth.min(256) {
        return Err(SnapshotError::ResourceLimit);
    }
    let result = match value {
        Value::Record(tag, fields) => {
            fields
                .iter()
                .try_fold(17u64 + tag.len() as u64, |total, field| {
                    total
                        .checked_add(size(field, control, depth + 1, max_depth)?)
                        .ok_or(SnapshotError::SizeOverflow)
                })?
        }
        Value::Sequence(fields) => fields.iter().try_fold(9u64, |total, field| {
            total
                .checked_add(size(field, control, depth + 1, max_depth)?)
                .ok_or(SnapshotError::SizeOverflow)
        })?,
        Value::Text(v) => 9 + v.len() as u64,
        Value::Bytes(v) => 9 + v.len() as u64,
        Value::Unsigned(_) | Value::Signed(_) | Value::Float(_) => 9,
        Value::Boolean(_) => 2,
        Value::Floats(v) => numeric_size(v.len(), 8)?,
        Value::Complexes(v) => numeric_size(v.len(), 16)?,
    };
    Ok(result)
}
fn numeric_size(count: usize, width: u64) -> Result<u64, SnapshotError> {
    (count as u64)
        .checked_mul(width)
        .and_then(|n| n.checked_add(9 + 4 * count.div_ceil(BLOCK) as u64))
        .ok_or(SnapshotError::SizeOverflow)
}

pub(super) fn write(
    writer: &mut impl Write,
    value: &Value<'_>,
    length: u64,
    control: &mut ExecutionContext<'_>,
) -> Result<(), SnapshotError> {
    let mut header = Vec::from(MAGIC.as_slice());
    header.extend_from_slice(&1u32.to_le_bytes());
    header.extend_from_slice(&length.to_le_bytes());
    let mut out = Encoder {
        writer,
        control,
        digest: Sha256::new(),
    };
    out.bytes(&header)?;
    out.value(value)?;
    let hash = out.digest.clone().finalize();
    out.bytes(&hash)?;
    out.bytes(COMMIT)?;
    Ok(())
}
struct Encoder<'a, 'ctx, W> {
    writer: &'a mut W,
    control: &'a mut ExecutionContext<'ctx>,
    digest: Sha256,
}
impl<W: Write> Encoder<'_, '_, W> {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), SnapshotError> {
        for block in bytes.chunks(32768) {
            self.control.check_cancelled()?;
            self.writer.write_all(block)?;
            self.digest.update(block);
            self.control.io(block.len())?;
        }
        Ok(())
    }
    fn count(&mut self, n: usize) -> Result<(), SnapshotError> {
        self.bytes(&(n as u64).to_le_bytes())
    }
    fn text(&mut self, s: &str) -> Result<(), SnapshotError> {
        self.count(s.len())?;
        self.bytes(s.as_bytes())
    }
    #[inline(never)]
    fn floats(&mut self, v: &[f64]) -> Result<(), SnapshotError> {
        self.bytes(&[8])?;
        self.count(v.len())?;
        let mut buffer = [0u8; BLOCK * 8];
        for chunk in v.chunks(BLOCK) {
            self.bytes(&(chunk.len() as u32).to_le_bytes())?;
            for (value, bytes) in chunk.iter().zip(buffer.chunks_exact_mut(8)) {
                bytes.copy_from_slice(&value.to_bits().to_le_bytes());
            }
            self.bytes(&buffer[..chunk.len() * 8])?;
        }
        Ok(())
    }
    #[inline(never)]
    fn complexes(&mut self, v: &[Complex64]) -> Result<(), SnapshotError> {
        self.bytes(&[9])?;
        self.count(v.len())?;
        let mut buffer = [0u8; BLOCK * 16];
        for chunk in v.chunks(BLOCK) {
            self.bytes(&(chunk.len() as u32).to_le_bytes())?;
            for (value, bytes) in chunk.iter().zip(buffer.chunks_exact_mut(16)) {
                bytes[..8].copy_from_slice(&value.re.to_bits().to_le_bytes());
                bytes[8..].copy_from_slice(&value.im.to_bits().to_le_bytes());
            }
            self.bytes(&buffer[..chunk.len() * 16])?;
        }
        Ok(())
    }
    fn value(&mut self, value: &Value<'_>) -> Result<(), SnapshotError> {
        match value {
            Value::Record(tag, fields) => {
                self.bytes(&[0])?;
                self.text(tag)?;
                self.count(fields.len())?;
                for field in fields {
                    self.value(field)?;
                }
            }
            Value::Sequence(fields) => {
                self.bytes(&[1])?;
                self.count(fields.len())?;
                for field in fields {
                    self.value(field)?;
                }
            }
            Value::Text(v) => {
                self.bytes(&[2])?;
                self.text(v)?;
            }
            Value::Unsigned(v) => {
                self.bytes(&[3])?;
                self.bytes(&v.to_le_bytes())?;
            }
            Value::Signed(v) => {
                self.bytes(&[4])?;
                self.bytes(&v.to_le_bytes())?;
            }
            Value::Float(v) => {
                self.bytes(&[5])?;
                self.bytes(&v.to_le_bytes())?;
            }
            Value::Boolean(v) => self.bytes(&[6, u8::from(*v)])?,
            Value::Bytes(v) => {
                self.bytes(&[7])?;
                self.count(v.len())?;
                self.bytes(v)?;
            }
            Value::Floats(v) => self.floats(v)?,
            Value::Complexes(v) => self.complexes(v)?,
        };
        Ok(())
    }
}

pub(super) fn read(
    reader: &mut impl Read,
    budget: &mut Budget<'_, '_>,
) -> Result<Value<'static>, SnapshotError> {
    let mut header = [0u8; 20];
    read_exact(reader, &mut header, budget.control)?;
    if &header[..8] != MAGIC {
        return Err(SnapshotError::Structure);
    }
    let version = u32::from_le_bytes(header[8..12].try_into().expect("fixed header"));
    if version != 1 {
        return Err(SnapshotError::UnsupportedVersion(version));
    }
    let length = u64::from_le_bytes(header[12..20].try_into().expect("fixed header"));
    if length
        .checked_add(60)
        .is_none_or(|n| n > budget.limits.max_bytes)
    {
        return Err(SnapshotError::ResourceLimit);
    }
    let mut digest = Sha256::new();
    digest.update(header);
    let mut decoder = Decoder {
        reader,
        remaining: length,
        digest,
        budget,
    };
    let value = decoder.value(0)?;
    if decoder.remaining != 0 {
        return Err(SnapshotError::Structure);
    }
    let expected = decoder.digest.finalize();
    let mut trailer = [0u8; 40];
    read_exact(decoder.reader, &mut trailer, decoder.budget.control)?;
    if trailer[..32] != expected[..] || &trailer[32..] != COMMIT {
        return Err(SnapshotError::Integrity);
    }
    Ok(value)
}
fn read_exact(
    reader: &mut impl Read,
    buffer: &mut [u8],
    control: &mut ExecutionContext<'_>,
) -> Result<(), SnapshotError> {
    for block in buffer.chunks_mut(32768) {
        let mut filled = 0;
        while filled < block.len() {
            control.check_cancelled()?;
            match reader.read(&mut block[filled..]) {
                Ok(0) => return Err(SnapshotError::Truncated),
                Ok(n) => {
                    filled += n;
                    control.io(n)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(())
}
struct Decoder<'a, 'b, 'ctx, R> {
    reader: &'a mut R,
    remaining: u64,
    digest: Sha256,
    budget: &'a mut Budget<'b, 'ctx>,
}
impl<R: Read> Decoder<'_, '_, '_, R> {
    fn bytes(&mut self, bytes: &mut [u8]) -> Result<(), SnapshotError> {
        if bytes.len() as u64 > self.remaining {
            return Err(SnapshotError::Truncated);
        }
        read_exact(self.reader, bytes, self.budget.control)?;
        self.remaining -= bytes.len() as u64;
        self.digest.update(bytes);
        Ok(())
    }
    fn number(&mut self) -> Result<u64, SnapshotError> {
        let mut b = [0; 8];
        self.bytes(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }
    fn count(&mut self, width: usize) -> Result<usize, SnapshotError> {
        let n = usize::try_from(self.number()?).map_err(|_| SnapshotError::SizeOverflow)?;
        let bytes = n.checked_mul(width).ok_or(SnapshotError::SizeOverflow)?;
        if bytes as u64 > self.remaining {
            return Err(SnapshotError::Truncated);
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<String, SnapshotError> {
        let n = self.count(1)?;
        self.budget.reserve::<u8>(n)?;
        let mut b = allocated(n)?;
        b.resize(n, 0);
        self.bytes(&mut b)?;
        String::from_utf8(b).map_err(|_| SnapshotError::Structure)
    }
    fn children(&mut self, depth: usize) -> Result<Vec<Value<'static>>, SnapshotError> {
        let n = self.count(1)?;
        self.budget.reserve::<Value<'_>>(n)?;
        let mut values = allocated(n)?;
        for _ in 0..n {
            values.push(self.value(depth + 1)?);
        }
        Ok(values)
    }
    #[inline(never)]
    fn numeric(&mut self, width: usize) -> Result<Value<'static>, SnapshotError> {
        let n = self.count(width)?;
        self.budget
            .samples(n.checked_mul(width).ok_or(SnapshotError::SizeOverflow)?)?;
        let mut reals = if width == 8 {
            allocated(n)?
        } else {
            Vec::new()
        };
        let mut complexes = if width == 16 {
            allocated(n)?
        } else {
            Vec::new()
        };
        let mut remaining = n;
        let mut buffer = [0u8; BLOCK * 16];
        while remaining != 0 {
            let mut chunk = [0; 4];
            self.bytes(&mut chunk)?;
            let count = u32::from_le_bytes(chunk) as usize;
            if count == 0 || count > BLOCK || count > remaining {
                return Err(SnapshotError::Structure);
            }
            self.bytes(&mut buffer[..count * width])?;
            for point in buffer[..count * width].chunks_exact(width) {
                let re =
                    f64::from_bits(u64::from_le_bytes(point[..8].try_into().expect("binary64")));
                if width == 8 {
                    reals.push(re)
                } else {
                    let im = f64::from_bits(u64::from_le_bytes(
                        point[8..].try_into().expect("binary64"),
                    ));
                    complexes.push(Complex64::new(re, im));
                }
            }
            remaining -= count;
        }
        Ok(if width == 8 {
            Value::Floats(Cow::Owned(reals))
        } else {
            Value::Complexes(Cow::Owned(complexes))
        })
    }
    fn value(&mut self, depth: usize) -> Result<Value<'static>, SnapshotError> {
        self.budget.control.check_cancelled()?;
        if depth > self.budget.limits.max_depth.min(256) {
            return Err(SnapshotError::ResourceLimit);
        }
        let mut tag = [0];
        self.bytes(&mut tag)?;
        Ok(match tag[0] {
            0 => {
                let tag = self.string()?;
                Value::Record(Cow::Owned(tag), self.children(depth)?)
            }
            1 => Value::Sequence(self.children(depth)?),
            2 => Value::Text(Cow::Owned(self.string()?)),
            3 => Value::Unsigned(self.number()?),
            4 => Value::Signed(self.number()? as i64),
            5 => Value::Float(self.number()?),
            6 => {
                let mut b = [0];
                self.bytes(&mut b)?;
                if b[0] > 1 {
                    return Err(SnapshotError::Structure);
                }
                Value::Boolean(b[0] == 1)
            }
            7 => {
                let n = self.count(1)?;
                self.budget.reserve::<u8>(n)?;
                let mut b = allocated(n)?;
                b.resize(n, 0);
                self.bytes(&mut b)?;
                Value::Bytes(Cow::Owned(b))
            }
            8 | 9 => self.numeric(if tag[0] == 8 { 8 } else { 16 })?,

            _ => return Err(SnapshotError::Structure),
        })
    }
}
fn allocated<T>(n: usize) -> Result<Vec<T>, SnapshotError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| SnapshotError::Allocation)?;
    Ok(v)
}
