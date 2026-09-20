use crate::processed::model::ProcessedDescriptor;
use crate::provenance::CanonicalDatasetDigests;
use crate::provenance::SampleNormalization;
use crate::provenance::SourceFile;

use self::validation::output_lineage;
use super::{ExecutionSegment, ProcessingRecord};

/// Immutable baseline descriptor and ordered processing records.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessingHistory {
    initial_descriptor: ProcessedDescriptor,
    inputs: Vec<HistoryInput>,
    axis_lineage: Vec<crate::provenance::InputAxisRef>,
    records: Vec<ProcessingRecord>,
    segments: Vec<ExecutionSegment>,
}

/// Immutable identity of the original processing input.
///
/// Library-produced bindings may gain additional identity facts. Use accessors
/// or include `..` in a variant match.
///
/// ```compile_fail,E0638
/// use nmr::processing::HistoryInput;
/// fn inspect(input: &HistoryInput) {
///     if let HistoryInput::Processed { digests, sources, read_record } = input {}
/// }
/// ```
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum HistoryInput {
    /// A raw acquisition and its applied decoding normalization.
    #[non_exhaustive]
    Raw {
        /// Canonical acquisition identity.
        digests: CanonicalDatasetDigests,
        /// Applied sample normalization.
        normalization: SampleNormalization,
        /// Reader format, or None for a caller-created memory input.
        format: Option<crate::raw::RawFormat>,
        /// Ordered original source contents; locators do not define identity.
        sources: Vec<SourceFile>,
    },
    /// A processed import or caller-created starting point.
    #[non_exhaustive]
    Processed {
        /// Canonical descriptor and sample identity.
        digests: CanonicalDatasetDigests,
        /// Original source records; locators are excluded from strict comparison.
        sources: Vec<SourceFile>,
        /// Recorded reader decoding, absent for caller-created memory inputs.
        read_record: Option<crate::provenance::ProcessedReadRecord>,
    },
}

impl HistoryInput {
    /// Returns the canonical identity of the original input.
    pub fn digests(&self) -> CanonicalDatasetDigests {
        match self {
            Self::Raw { digests, .. } | Self::Processed { digests, .. } => *digests,
        }
    }

    /// Returns the original ordered source records.
    pub fn sources(&self) -> &[SourceFile] {
        match self {
            Self::Raw { sources, .. } | Self::Processed { sources, .. } => sources,
        }
    }
}
impl ProcessingHistory {
    pub(crate) fn accept_archived_history(&mut self) {
        for segment in &mut self.segments {
            segment.accept_archive();
        }
    }
    /// Returns the descriptor before the first processing operation.
    pub fn initial_descriptor(&self) -> &ProcessedDescriptor {
        &self.initial_descriptor
    }

    /// Returns processing records in application order.
    pub fn records(&self) -> &[ProcessingRecord] {
        &self.records
    }

    /// Completed execution segments in order.
    pub fn segments(&self) -> &[ExecutionSegment] {
        &self.segments
    }

    pub(crate) fn inherit_segments(&mut self, previous: Option<&Self>) {
        self.segments = previous.map_or_else(Vec::new, |history| history.segments.clone());
    }

    pub(crate) fn finish_segment(&mut self, output: CanonicalDatasetDigests) {
        self.segments
            .push(ExecutionSegment::new(self.records.len(), output));
    }

    pub(crate) fn prefix(&self, end: usize) -> Self {
        Self {
            initial_descriptor: self.initial_descriptor.clone(),
            inputs: self.inputs.clone(),
            axis_lineage: self.axis_lineage.clone(),
            records: self.records[..end].to_vec(),
            segments: Vec::new(),
        }
    }

    /// Returns the canonical raw identity required for numeric replay.
    pub fn raw_input_digests(&self) -> Option<CanonicalDatasetDigests> {
        match self.input() {
            HistoryInput::Raw { digests, .. } => Some(*digests),
            _ => None,
        }
    }

    /// Returns reader scaling and stored-imaginary sign evidence for raw replay.
    pub fn raw_input_normalization(&self) -> Option<&SampleNormalization> {
        match self.input() {
            HistoryInput::Raw { normalization, .. } => Some(normalization),
            _ => None,
        }
    }

    /// Returns the original input binding, retained across additional processing.
    pub fn input(&self) -> &HistoryInput {
        &self.inputs[0]
    }

    /// Returns the ordered external starting points. Current execution supports one.
    pub fn inputs(&self) -> &[HistoryInput] {
        &self.inputs
    }

    /// Returns output-axis ancestry in the ordered external-input namespace.
    /// This identifies axes, not their FFT bins or coordinate transformations.
    pub fn axis_lineage(&self) -> &[crate::provenance::InputAxisRef] {
        &self.axis_lineage
    }

    pub(crate) fn new(
        initial_descriptor: ProcessedDescriptor,
        input: HistoryInput,
        records: Vec<ProcessingRecord>,
    ) -> Self {
        Self {
            axis_lineage: output_lineage(initial_descriptor.axes().len(), &records),
            initial_descriptor,
            inputs: vec![input],
            records,
            segments: Vec::new(),
        }
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ProcessingHistory {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &ProcessedDescriptor,
        &Vec<HistoryInput>,
        &Vec<crate::provenance::InputAxisRef>,
        &Vec<ProcessingRecord>,
        &Vec<ExecutionSegment>,
    ) {
        (
            &self.initial_descriptor,
            &self.inputs,
            &self.axis_lineage,
            &self.records,
            &self.segments,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            ProcessedDescriptor,
            Vec<HistoryInput>,
            Vec<crate::provenance::InputAxisRef>,
            Vec<ProcessingRecord>,
            Vec<ExecutionSegment>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (initial_descriptor, inputs, axis_lineage, records, segments) = parts;
        let value = Self {
            initial_descriptor,
            inputs,
            axis_lineage,
            records,
            segments,
        };

        Ok(value)
    }
}

#[cfg(test)]
mod tests;
pub(super) mod validation;
