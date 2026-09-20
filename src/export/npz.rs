use super::*;

pub(super) const LOCAL_HEADER_BYTES: u64 = 30;
pub(super) const CENTRAL_HEADER_BYTES: u64 = 46;
pub(super) const EOCD_BYTES: u64 = 22;

#[derive(Clone, Copy)]
pub(super) enum Payload<'a> {
    F64(&'a [f64]),
    U64(&'a [u64]),
    Bytes(&'a [u8]),
}

impl Payload<'_> {
    fn byte_len(self) -> Result<u64, ExportError> {
        let (length, width) = match self {
            Self::F64(values) => (values.len(), 8_u64),
            Self::U64(values) => (values.len(), 8_u64),
            Self::Bytes(values) => (values.len(), 1_u64),
        };
        u64::try_from(length)
            .ok()
            .and_then(|value| value.checked_mul(width))
            .ok_or(ExportError::SizeOverflow)
    }

    fn chunks(
        self,
        buffer: &mut [u8],
        mut consume: impl FnMut(&[u8]) -> Result<(), ExportError>,
    ) -> Result<(), ExportError> {
        match self {
            Self::F64(values) => scalar_chunks(values, buffer, f64::to_le_bytes, consume),
            Self::U64(values) => scalar_chunks(values, buffer, u64::to_le_bytes, consume),
            Self::Bytes(values) => {
                for chunk in values.chunks(buffer.len()) {
                    consume(chunk)?;
                }
                Ok(())
            }
        }
    }
}

pub(super) fn scalar_chunks<T: Copy>(
    values: &[T],
    buffer: &mut [u8],
    encode: impl Fn(T) -> [u8; 8],
    mut consume: impl FnMut(&[u8]) -> Result<(), ExportError>,
) -> Result<(), ExportError> {
    let per_chunk = buffer.len() / 8;
    for values in values.chunks(per_chunk) {
        for (index, value) in values.iter().copied().enumerate() {
            buffer[index * 8..index * 8 + 8].copy_from_slice(&encode(value));
        }
        consume(&buffer[..values.len() * 8])?;
    }
    Ok(())
}

pub(super) struct Member<'a> {
    pub(super) name: String,
    pub(super) npy_header: Vec<u8>,
    pub(super) payload: Payload<'a>,
    pub(super) size: u32,
    pub(super) crc32: u32,
}

pub(super) fn build_members<'a>(
    plot: &'a PlotData,
    manifest: &'a [u8],
) -> Result<Vec<Member<'a>>, ExportError> {
    let mut members = Vec::new();
    members
        .try_reserve_exact(plot.axes().len() + 2)
        .map_err(|_| ExportError::AllocationFailure)?;
    members.push(member(
        "data.npy".to_owned(),
        "<f8",
        plot.shape(),
        Payload::F64(plot.data()),
    )?);
    for (index, axis) in plot.axes().iter().enumerate() {
        let name = format!("axis_{index}.npy");
        match axis.coordinates() {
            PlotCoordinates::Physical { values, .. } => {
                members.push(member(name, "<f8", &[values.len()], Payload::F64(values))?);
            }
            PlotCoordinates::LogicalIndex(values) => {
                members.push(member(name, "<u8", &[values.len()], Payload::U64(values))?);
            }
        }
    }
    members.push(member(
        "manifest.npy".to_owned(),
        "|u1",
        &[manifest.len()],
        Payload::Bytes(manifest),
    )?);
    Ok(members)
}

pub(super) fn member<'a>(
    name: String,
    descr: &str,
    shape: &[usize],
    payload: Payload<'a>,
) -> Result<Member<'a>, ExportError> {
    if !name.is_ascii() || name.len() > u16::MAX as usize {
        return Err(ExportError::SizeOverflow);
    }
    let npy_header = npy_header(descr, shape)?;
    let size = u64::try_from(npy_header.len())
        .ok()
        .and_then(|header| header.checked_add(payload.byte_len().ok()?))
        .ok_or(ExportError::SizeOverflow)?;
    let size = u32::try_from(size).map_err(|_| ExportError::Zip32Limit)?;
    Ok(Member {
        name,
        npy_header,
        payload,
        size,
        crc32: 0,
    })
}

pub(super) fn npy_header(descr: &str, shape: &[usize]) -> Result<Vec<u8>, ExportError> {
    let shape = shape_literal(shape);
    let dictionary = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {shape}, }}");
    if !dictionary.is_ascii() {
        return Err(ExportError::InvalidNpyHeader);
    }
    let unpadded = 10_u64
        .checked_add(u64::try_from(dictionary.len()).map_err(|_| ExportError::SizeOverflow)?)
        .and_then(|value| value.checked_add(1))
        .ok_or(ExportError::SizeOverflow)?;
    let padding = (64 - unpadded % 64) % 64;
    let header_len = u64::try_from(dictionary.len())
        .ok()
        .and_then(|value| value.checked_add(padding))
        .and_then(|value| value.checked_add(1))
        .ok_or(ExportError::SizeOverflow)?;
    let header_len = u16::try_from(header_len).map_err(|_| ExportError::NpyHeaderTooLarge)?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(10 + usize::from(header_len))
        .map_err(|_| ExportError::AllocationFailure)?;
    header.extend_from_slice(b"\x93NUMPY\x01\x00");
    header.extend_from_slice(&header_len.to_le_bytes());
    header.extend_from_slice(dictionary.as_bytes());
    header.resize(header.len() + padding as usize, b' ');
    header.push(b'\n');
    debug_assert_eq!(header.len() % 64, 0);
    Ok(header)
}

pub(super) fn shape_literal(shape: &[usize]) -> String {
    let mut literal = String::from("(");
    for (index, extent) in shape.iter().enumerate() {
        if index != 0 {
            literal.push_str(", ");
        }
        literal.push_str(&extent.to_string());
    }
    if shape.len() == 1 {
        literal.push(',');
    }
    literal.push(')');
    literal
}

pub(super) fn preflight_archive(
    members: &[Member<'_>],
    work: &mut ExecutionContext<'_>,
) -> Result<(), ExportError> {
    if members.len() > u16::MAX as usize {
        return Err(ExportError::Zip32Limit);
    }
    let mut local_bytes = 0_u64;
    let mut central_bytes = 0_u64;
    let mut member_bytes = 0_u128;
    for member in members {
        let name = u64::try_from(member.name.len()).map_err(|_| ExportError::SizeOverflow)?;
        local_bytes = local_bytes
            .checked_add(LOCAL_HEADER_BYTES)
            .and_then(|value| value.checked_add(name))
            .and_then(|value| value.checked_add(u64::from(member.size)))
            .ok_or(ExportError::SizeOverflow)?;
        central_bytes = central_bytes
            .checked_add(CENTRAL_HEADER_BYTES)
            .and_then(|value| value.checked_add(name))
            .ok_or(ExportError::SizeOverflow)?;
        member_bytes = member_bytes
            .checked_add(u128::from(member.size))
            .ok_or(ExportError::SizeOverflow)?;
    }
    let archive = local_bytes
        .checked_add(central_bytes)
        .and_then(|value| value.checked_add(EOCD_BYTES))
        .ok_or(ExportError::SizeOverflow)?;
    if archive > MAX_EXPORT_BYTES || archive > u64::from(u32::MAX) {
        return Err(ExportError::ArchiveLimit);
    }
    let charged = member_bytes
        .checked_mul(2)
        .ok_or(ExportError::SizeOverflow)?;
    work.ensure_work(charged)?;
    Ok(())
}

pub(super) fn member_crc(
    member: &Member<'_>,
    buffer: &mut [u8],
    control: &mut ExecutionContext<'_>,
) -> Result<u32, ExportError> {
    let mut crc = Crc32::new();
    control.charge_without_progress(member.npy_header.len() as u128)?;
    crc.update(&member.npy_header);
    member.payload.chunks(buffer, |chunk| {
        control.charge_without_progress(chunk.len() as u128)?;
        crc.update(chunk);
        Ok(())
    })?;
    Ok(crc.finish())
}

pub(super) fn write_archive(
    file: &mut impl Write,
    members: &[Member<'_>],
    buffer: &mut [u8],
    control: &mut ExecutionContext<'_>,
) -> Result<(), ExportError> {
    let mut buffered = BufWriter::with_capacity(BUFFER_BYTES, file);
    let mut writer = ControlledWriter {
        writer: &mut buffered,
        control,
        count_io: true,
    };
    let mut offsets = Vec::new();
    offsets
        .try_reserve_exact(members.len())
        .map_err(|_| ExportError::AllocationFailure)?;
    let mut offset = 0_u64;
    for member in members {
        offsets.push(u32::try_from(offset).map_err(|_| ExportError::Zip32Limit)?);
        write_local_header(&mut writer, member)?;
        writer
            .control
            .charge_without_progress(member.npy_header.len() as u128)?;
        writer.write_all(&member.npy_header)?;
        member.payload.chunks(buffer, |chunk| {
            writer
                .control
                .charge_without_progress(chunk.len() as u128)?;
            writer.write_all(chunk)?;
            Ok(())
        })?;
        offset = offset
            .checked_add(LOCAL_HEADER_BYTES)
            .and_then(|value| value.checked_add(member.name.len() as u64))
            .and_then(|value| value.checked_add(u64::from(member.size)))
            .ok_or(ExportError::SizeOverflow)?;
    }
    let central_offset = u32::try_from(offset).map_err(|_| ExportError::Zip32Limit)?;
    for (member, &local_offset) in members.iter().zip(&offsets) {
        write_central_header(&mut writer, member, local_offset)?;
        offset = offset
            .checked_add(CENTRAL_HEADER_BYTES)
            .and_then(|value| value.checked_add(member.name.len() as u64))
            .ok_or(ExportError::SizeOverflow)?;
    }
    let central_size =
        u32::try_from(offset - u64::from(central_offset)).map_err(|_| ExportError::Zip32Limit)?;
    write_eocd(
        &mut writer,
        u16::try_from(members.len()).map_err(|_| ExportError::Zip32Limit)?,
        central_size,
        central_offset,
    )?;
    writer.flush()?;
    Ok(())
}

pub(super) fn write_local_header(writer: &mut impl Write, member: &Member<'_>) -> io::Result<()> {
    write_u32(writer, 0x0403_4b50)?;
    write_u16(writer, 20)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u32(writer, member.crc32)?;
    write_u32(writer, member.size)?;
    write_u32(writer, member.size)?;
    write_u16(writer, member.name.len() as u16)?;
    write_u16(writer, 0)?;
    writer.write_all(member.name.as_bytes())
}

pub(super) fn write_central_header(
    writer: &mut impl Write,
    member: &Member<'_>,
    local_offset: u32,
) -> io::Result<()> {
    write_u32(writer, 0x0201_4b50)?;
    write_u16(writer, 20)?;
    write_u16(writer, 20)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u32(writer, member.crc32)?;
    write_u32(writer, member.size)?;
    write_u32(writer, member.size)?;
    write_u16(writer, member.name.len() as u16)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u32(writer, 0)?;
    write_u32(writer, local_offset)?;
    writer.write_all(member.name.as_bytes())
}

pub(super) fn write_eocd(
    writer: &mut impl Write,
    entries: u16,
    central_size: u32,
    central_offset: u32,
) -> io::Result<()> {
    write_u32(writer, 0x0605_4b50)?;
    write_u16(writer, 0)?;
    write_u16(writer, 0)?;
    write_u16(writer, entries)?;
    write_u16(writer, entries)?;
    write_u32(writer, central_size)?;
    write_u32(writer, central_offset)?;
    write_u16(writer, 0)
}

pub(super) fn write_u16(writer: &mut impl Write, value: u16) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

pub(super) fn write_u32(writer: &mut impl Write, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

pub(super) struct Crc32(u32);

impl Crc32 {
    fn new() -> Self {
        Self(u32::MAX)
    }

    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u32::from(byte);
            for _ in 0..8 {
                self.0 = (self.0 >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(self.0 & 1)));
            }
        }
    }

    fn finish(self) -> u32 {
        !self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        let mut crc = Crc32::new();
        crc.update(b"123456789");
        assert_eq!(crc.finish(), 0xcbf4_3926);
    }

    #[test]
    fn npy_v1_rejects_a_header_larger_than_u16() {
        let shape = vec![usize::MAX; 4_000];
        assert!(matches!(
            npy_header("<f8", &shape),
            Err(ExportError::NpyHeaderTooLarge)
        ));
    }

    #[test]
    fn archive_limit_is_checked_without_allocating_the_declared_payload() {
        let member = Member {
            name: "data.npy".to_owned(),
            npy_header: Vec::new(),
            payload: Payload::Bytes(&[]),
            size: (MAX_EXPORT_BYTES - LOCAL_HEADER_BYTES) as u32,
            crc32: 0,
        };
        let mut work = WorkLedger::new(u128::MAX);
        assert!(matches!(
            preflight_archive(&[member], &mut ExecutionContext::new(&mut work)),
            Err(ExportError::ArchiveLimit)
        ));
        assert_eq!(work.used(), 0);
    }
}
