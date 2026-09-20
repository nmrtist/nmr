use crate::ReadLimits;
use crate::processed::ProcessedDataset;
use crate::provenance::SourceFile;
use crate::provenance::SourceKind;
use crate::read_error::ReadError;
use crate::read_error::ReadResource;
use crate::reading::processed::ProcessedRead;
use crate::reading::resolver::ResolvedCandidate;
use std::path::Path;

use super::Jcamp;
use super::assembly::finish;
use super::parameters::enforce_metadata_limit;

pub(crate) fn read(
    control: &mut crate::ExecutionContext<'_>,
    candidate: &ResolvedCandidate,
    limits: ReadLimits,
) -> Result<ProcessedRead, ReadError> {
    let source_bytes = crate::ensure_file_size(
        &candidate.primary,
        limits.source_bytes(),
        ReadResource::SourceBytes,
    )?;
    enforce_working(source_bytes, limits.working_bytes())?;
    let bytes = crate::io::read_sized_file_controlled(control, &candidate.primary, source_bytes)?;
    if bytes.len() > limits.source_bytes() {
        return Err(ReadError::limit(
            ReadResource::SourceBytes,
            limits.source_bytes(),
            bytes.len(),
        ));
    }
    enforce_working(bytes.len(), limits.working_bytes())?;
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        ReadError::corrupt(candidate.primary.clone().into(), "JCAMP input is not UTF-8")
    })?;
    enforce_metadata_limit(control, text, limits.metadata_bytes())?;
    control.check_cancelled()?;
    let parsed = Jcamp::parse(control, text, &candidate.primary, &limits, bytes.len())?;
    finish(
        control,
        parsed,
        vec![SourceFile::from_consumed_bytes(
            SourceKind::Data,
            "jcamp_dx",
            &candidate.primary,
            &bytes,
        )],
        text,
        limits,
    )
}

pub(crate) fn read_parts(bytes: &[u8]) -> Result<ProcessedDataset, ReadError> {
    read_parts_with_limits(bytes, ReadLimits::default())
}

pub(crate) fn read_parts_with_limits(
    bytes: &[u8],
    limits: ReadLimits,
) -> Result<ProcessedDataset, ReadError> {
    let control = &mut crate::ExecutionContext::default();
    if bytes.len() > limits.source_bytes() {
        return Err(ReadError::limit(
            ReadResource::SourceBytes,
            limits.source_bytes(),
            bytes.len(),
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        ReadError::corrupt(
            crate::raw::InputSource::memory("jcamp_dx"),
            "JCAMP input is not UTF-8",
        )
    })?;
    enforce_metadata_limit(control, text, limits.metadata_bytes())?;
    let parsed = Jcamp::parse(control, text, Path::new("jcamp_dx"), &limits, 0)?;
    let source =
        SourceFile::from_consumed_bytes(SourceKind::Data, "jcamp_dx", Path::new(""), bytes);
    Ok(finish(control, parsed, vec![source], text, limits)?.dataset)
}

pub(super) fn enforce_working(required: usize, limit: usize) -> Result<(), ReadError> {
    if required > limit {
        return Err(ReadError::limit(
            ReadResource::WorkingBytes,
            limit,
            required,
        ));
    }
    Ok(())
}
