use super::*;

pub(super) const BUFFER_BYTES: usize = 64 * 1024;

/// Exports checked plot data as deterministic uncompressed ZIP32 NPZ V1.
pub fn export_npz(
    plot: &PlotData,
    target: impl AsRef<Path>,
    work: &mut WorkLedger,
) -> Result<(), ExportError> {
    export_npz_with_context(plot, target, &mut ExecutionContext::new(work))
}
/// Publishes NPZ through a temporary file; cancellation before commit leaves no target.
pub fn export_npz_with_context(
    plot: &PlotData,
    target: impl AsRef<Path>,
    control: &mut ExecutionContext<'_>,
) -> Result<(), ExportError> {
    control.begin(ExecutionStage::Export, None, None)?;
    let target = target.as_ref();
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let manifest = manifest_json(plot);
    let mut members = build_members(plot, &manifest)?;
    preflight_archive(&members, control)?;
    let mut buffer = [0_u8; BUFFER_BYTES];
    for member in &mut members {
        member.crc32 = member_crc(member, &mut buffer, control)?;
    }

    let temporary = NamedTempFile::new_in(parent).map_err(ExportError::CreateTemporary)?;
    write_archive(&mut temporary.as_file(), &members, &mut buffer, control)?;
    control.check_cancelled()?;
    publish(temporary, target)
}
/// Writes NPZ to a host-owned stream. Failure can leave a prefix; the host owns commit.
pub fn write_npz_with_context(
    plot: &PlotData,
    writer: &mut impl Write,
    control: &mut ExecutionContext<'_>,
) -> Result<(), ExportError> {
    control.begin(ExecutionStage::Export, None, None)?;
    let manifest = manifest_json(plot);
    let mut members = build_members(plot, &manifest)?;
    preflight_archive(&members, control)?;
    let mut buffer = [0u8; BUFFER_BYTES];
    for member in &mut members {
        member.crc32 = member_crc(member, &mut buffer, control)?;
    }
    write_archive(writer, &members, &mut buffer, control)
}
/// Writes NPZ with default execution controls to a host-owned stream.
pub fn write_npz(plot: &PlotData, writer: &mut impl Write) -> Result<(), ExportError> {
    write_npz_with_context(plot, writer, &mut ExecutionContext::default())
}
