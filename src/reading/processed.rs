use crate::formats::{bruker::processed as bruker, jcamp_dx::decoding as jcamp_dx};

use crate::processed::{self, ProcessedDataset};
use crate::reading::resolver::ResolvedCandidate;
use crate::{ReadError, ReadLimits, ReadWarning};

pub(crate) struct ProcessedRead {
    pub(crate) dataset: ProcessedDataset,
    pub(crate) acquisition: Option<String>,
    pub(crate) warnings: Vec<ReadWarning>,
}

pub(crate) fn read(
    control: &mut crate::ExecutionContext<'_>,
    candidate: &ResolvedCandidate,
    format: processed::Format,
    limits: ReadLimits,
    experimental_vendor_semantics: bool,
) -> Result<ProcessedRead, ReadError> {
    match format {
        processed::Format::BrukerTopSpin => bruker::read(control, candidate, limits),
        processed::Format::JcampDx => jcamp_dx::read(control, candidate, limits),
        processed::Format::JeolDelta => crate::formats::jeol::require_experimental_opt_in(
            experimental_vendor_semantics,
            candidate.primary.clone().into(),
        )
        .and_then(|()| crate::formats::jeol::read_processed(control, &candidate.primary, limits)),
    }
    .map_err(|error| error.with_format(format))
}
