use crate::ReadLimits;
use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisRole;
use crate::axis::FrequencyEvidence;
use crate::processed::ComponentBasis;
use crate::processed::ProcessedAxis;
use crate::processed::ProcessedData;
use crate::processed::ProcessedDataset;
use crate::processed::ProcessedDescriptor;
use crate::processed::ProcessedOrigin;
use crate::processed::ProcessedProvenance;
use crate::processed::SourceMetadata;
use crate::provenance::SourceFile;
use crate::read_error::ReadError;
use crate::read_error::ReadResource;
use crate::reading::processed::ProcessedRead;

use super::Jcamp;
use super::parameters::parameter_ranges;

pub(super) fn finish(
    control: &mut crate::ExecutionContext<'_>,
    parsed: Jcamp,
    mut sources: Vec<SourceFile>,
    text: &str,
    limits: ReadLimits,
) -> Result<ProcessedRead, ReadError> {
    control.check_cancelled()?;
    let result_bytes = parsed
        .samples
        .len()
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ReadError::SizeOverflow)?;
    if result_bytes > limits.materialized_bytes() {
        return Err(ReadError::limit(
            ReadResource::MaterializedBytes,
            limits.materialized_bytes(),
            result_bytes,
        ));
    }
    let points = parsed.samples.len();
    let coordinates = if points == 1 {
        AxisCoordinates::Explicit(vec![parsed.first_x])
    } else {
        AxisCoordinates::Uniform {
            start: parsed.first_x,
            step: parsed.delta_x,
        }
    };
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(parsed.unit),
        points,
        coordinates,
        ComponentBasis::Scalar,
    )?
    .with_label(parsed.nucleus.clone())
    .with_nucleus(parsed.nucleus.clone())?
    .with_frequency_evidence(Some(FrequencyEvidence::new(
        parsed.observe_frequency_mhz,
        None,
    )?))?;
    let descriptor = ProcessedDescriptor::new(vec![axis])?;
    let data = ProcessedData::new(vec![points], vec![1], parsed.samples)?;
    for (ordinal, source) in sources.iter_mut().enumerate() {
        source.assign_id(ordinal);
    }
    let source = sources[0].id().unwrap();
    let record = crate::provenance::ProcessedReadRecord::jcamp(
        source,
        parsed.x_factor,
        parsed.y_factor,
        crate::canonical_digest::processed_digests(
            &descriptor,
            &data,
            &crate::processing::contracts::state::PlanState::from_descriptor(&descriptor),
        ),
    );
    let records = parameter_ranges(text, source);
    let provenance =
        ProcessedProvenance::new(ProcessedOrigin::Imported, sources)?.with_read_record(record);
    let dataset = ProcessedDataset::new_imported(
        descriptor,
        data,
        provenance,
        SourceMetadata::jcamp(parsed.labels, records),
    )?;
    Ok(ProcessedRead {
        dataset,
        acquisition: None,
        warnings: Vec::new(),
    })
}
