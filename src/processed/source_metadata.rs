/// Opaque vendor metadata retained by processed-format readers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceMetadata(SourceMetadataInner);

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum SourceMetadataInner {
    #[default]
    Empty,
    BrukerTopSpin(BrukerSourceMetadata),
    JcampDx(JcampSourceMetadata),
    JeolDelta(JeolSourceMetadata),
}

impl SourceMetadata {
    /// Returns Bruker processed parameters when this metadata came from TopSpin.
    pub fn bruker_topspin(&self) -> Option<&BrukerSourceMetadata> {
        match &self.0 {
            SourceMetadataInner::BrukerTopSpin(value) => Some(value),
            _ => None,
        }
    }

    /// Returns JCAMP-DX labels when this metadata came from a JCAMP spectrum.
    pub fn jcamp_dx(&self) -> Option<&JcampSourceMetadata> {
        match &self.0 {
            SourceMetadataInner::JcampDx(value) => Some(value),
            _ => None,
        }
    }

    /// Returns JEOL parameters when this metadata came from a Delta spectrum.
    pub fn jeol_delta(&self) -> Option<&JeolSourceMetadata> {
        match &self.0 {
            SourceMetadataInner::JeolDelta(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn bruker(
        parameters: Vec<(String, String)>,
        documents: Vec<SourceParameterText>,
    ) -> Self {
        Self(SourceMetadataInner::BrukerTopSpin(BrukerSourceMetadata {
            parameters,
            documents,
        }))
    }

    pub(crate) fn jcamp(labels: Vec<(String, String)>, records: Vec<SourceParameterText>) -> Self {
        Self(SourceMetadataInner::JcampDx(JcampSourceMetadata {
            labels,
            records,
        }))
    }

    pub(crate) fn jeol(
        parameters: crate::formats::jeol::parameters::Parameters,
        source: crate::provenance::SourceId,
        payload_end: usize,
    ) -> Self {
        Self(SourceMetadataInner::JeolDelta(JeolSourceMetadata {
            parameters: std::sync::Arc::new(parameters),
            source,
            payload_end,
        }))
    }
}

/// Retained Bruker processed parameter records.
#[derive(Clone, Debug, PartialEq)]
pub struct BrukerSourceMetadata {
    parameters: Vec<(String, String)>,
    documents: Vec<SourceParameterText>,
}

/// Exact UTF-8 parameter range, including unknown records and comments.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceParameterText {
    source: crate::provenance::SourceId,
    text: std::sync::Arc<str>,
    byte_offset: usize,
}

impl SourceParameterText {
    /// Dataset-local source containing the retained parameter bytes.
    pub fn source(&self) -> crate::provenance::SourceId {
        self.source
    }
    /// Exact original UTF-8 bytes, preserving line endings and record order.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Start byte offset in the original source; text.len() is its byte length.
    pub fn byte_offset(&self) -> usize {
        self.byte_offset
    }
    /// Version of the full-document retention rule.
    pub fn retention_version(&self) -> &str {
        "exact-utf8-parameter-range.v1"
    }
    pub(crate) fn new(source: crate::provenance::SourceId, text: String) -> Self {
        Self {
            source,
            text: text.into(),
            byte_offset: 0,
        }
    }
    pub(crate) fn at_offset(
        source: crate::provenance::SourceId,
        byte_offset: usize,
        text: String,
    ) -> Self {
        Self {
            source,
            byte_offset,
            text: text.into(),
        }
    }
}

impl BrukerSourceMetadata {
    /// Complete procs and, for 2D, proc2s documents in source order.
    /// The existing metadata input-byte limit covers their combined byte length;
    /// it does not bound all parsed object allocations.
    pub fn documents(&self) -> &[SourceParameterText] {
        &self.documents
    }
    /// Returns retained parameter name/value pairs in deterministic order.
    pub fn parameters(&self) -> &[(String, String)] {
        &self.parameters
    }
}

/// Retained JCAMP-DX labels.
#[derive(Clone, Debug, PartialEq)]
pub struct JcampSourceMetadata {
    labels: Vec<(String, String)>,
    records: Vec<SourceParameterText>,
}

impl JcampSourceMetadata {
    /// Exact parameter ranges outside XYDATA sample rows, in source order.
    /// Includes the XYDATA declaration and original newline bytes.
    pub fn records(&self) -> &[SourceParameterText] {
        &self.records
    }
    /// Returns retained label/value pairs in source order.
    pub fn labels(&self) -> &[(String, String)] {
        &self.labels
    }
}

/// Retained JEOL processed parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct JeolSourceMetadata {
    parameters: std::sync::Arc<crate::formats::jeol::parameters::Parameters>,
    source: crate::provenance::SourceId,
    payload_end: usize,
}

impl JeolSourceMetadata {
    /// Returns original typed records, raw units/classes, and non-sample byte areas.
    pub fn parameters(&self) -> &crate::formats::jeol::parameters::Parameters {
        &self.parameters
    }
    /// Source containing the retained header, parameter areas and sample sections.
    pub fn source(&self) -> crate::provenance::SourceId {
        self.source
    }
    /// Original byte offset of the retained trailing records.
    pub fn trailing_byte_offset(&self) -> usize {
        self.payload_end
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl BrukerSourceMetadata {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Vec<(String, String)>, &Vec<SourceParameterText>) {
        (&self.parameters, &self.documents)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Vec<(String, String)>, Vec<SourceParameterText>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (parameters, documents) = parts;
        let value = Self {
            parameters,
            documents,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl JcampSourceMetadata {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Vec<(String, String)>, &Vec<SourceParameterText>) {
        (&self.labels, &self.records)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Vec<(String, String)>, Vec<SourceParameterText>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (labels, records) = parts;
        let value = Self { labels, records };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl JeolSourceMetadata {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &std::sync::Arc<crate::formats::jeol::parameters::Parameters>,
        &crate::provenance::SourceId,
        &usize,
    ) {
        (&self.parameters, &self.source, &self.payload_end)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            std::sync::Arc<crate::formats::jeol::parameters::Parameters>,
            crate::provenance::SourceId,
            usize,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (parameters, source, payload_end) = parts;
        let value = Self {
            parameters,
            source,
            payload_end,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SourceMetadata {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&SourceMetadataInner,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (SourceMetadataInner,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl SourceParameterText {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (&crate::provenance::SourceId, &std::sync::Arc<str>, &usize) {
        (&self.source, &self.text, &self.byte_offset)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (crate::provenance::SourceId, std::sync::Arc<str>, usize),
    ) -> Result<Self, crate::internal::ModelError> {
        let (source, text, byte_offset) = parts;
        let value = Self {
            source,
            text,
            byte_offset,
        };

        Ok(value)
    }
}
