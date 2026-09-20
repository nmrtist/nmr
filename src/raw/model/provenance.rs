use super::*;

/// Source-layer provenance kept separate from canonical acquisition semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct RawProvenance {
    pub(super) format: Option<RawFormat>,
    pub(super) sources: Vec<SourceFile>,
    pub(super) source_metadata: VendorMetadata,
    pub(super) sample_normalization: crate::provenance::SampleNormalization,
}

impl RawProvenance {
    pub(crate) fn reader(
        format: RawFormat,
        mut sources: Vec<SourceFile>,
        source_metadata: VendorMetadata,
    ) -> Self {
        let sample_normalization = source_metadata.sample_normalization();
        for (ordinal, source) in sources.iter_mut().enumerate() {
            source.assign_id(ordinal);
        }
        Self {
            format: Some(format),
            sources,
            source_metadata,
            sample_normalization,
        }
    }

    pub(super) fn user_constructed(mut sources: Vec<SourceFile>) -> Self {
        for (ordinal, source) in sources.iter_mut().enumerate() {
            source.assign_id(ordinal);
        }
        Self {
            format: None,
            sources,
            source_metadata: VendorMetadata::none(),
            sample_normalization: crate::provenance::SampleNormalization::identity(),
        }
    }

    /// Returns the source format for reader-created data.
    pub fn format(&self) -> Option<RawFormat> {
        self.format
    }

    /// Returns typed source files in stable role order.
    pub fn sources(&self) -> &[SourceFile] {
        &self.sources
    }

    /// Returns opaque format metadata for source inspection only.
    pub fn source_metadata(&self) -> &VendorMetadata {
        &self.source_metadata
    }

    /// Returns the typed numeric normalization applied by the source reader.
    pub fn sample_normalization(&self) -> &crate::provenance::SampleNormalization {
        &self.sample_normalization
    }

    pub(crate) fn verify_sources(&self) -> Result<(), ReadError> {
        for source in &self.sources {
            let path = source.path().to_path_buf();
            let result = if matches!(source.kind(), crate::provenance::SourceKind::Data) {
                source.verify_identity()
            } else {
                source.verify_digest()
            };
            result.map_err(|error| match error {
                ProvenanceError::SourceChanged => ReadError::source_changed(path.into()),
                other => ReadError::from(other),
            })?;
        }
        Ok(())
    }

    pub(crate) fn finalize_sources(
        &mut self,
        digest: Option<crate::provenance::SourceDigest>,
    ) -> Result<(), ReadError> {
        self.verify_sources()?;
        for source in &mut self.sources {
            if matches!(source.kind(), crate::provenance::SourceKind::Data) {
                let digest =
                    digest.ok_or_else(|| ReadError::source_changed(source.path().into()))?;
                source.set_consumed_digest(digest);
            }
        }
        Ok(())
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawProvenance {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Option<RawFormat>,
        &Vec<SourceFile>,
        &VendorMetadata,
        &crate::provenance::SampleNormalization,
    ) {
        (
            &self.format,
            &self.sources,
            &self.source_metadata,
            &self.sample_normalization,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Option<RawFormat>,
            Vec<SourceFile>,
            VendorMetadata,
            crate::provenance::SampleNormalization,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (format, sources, source_metadata, sample_normalization) = parts;
        let value = Self {
            format,
            sources,
            source_metadata,
            sample_normalization,
        };

        Ok(value)
    }
}
