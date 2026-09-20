use crate::Dataset;
use crate::processed::ProcessedValidationError;
use crate::raw::{RawDataset, RawFormat};
use crate::read_error::{ReadError, ReadErrorReason};
use crate::reading::resolver::{ResolvedCandidate, resolve_and_select};
use crate::{ReadLimits, processed};
use std::path::{Path, PathBuf};

/// Whether a loaded dataset contains acquisition or processed samples.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetKind {
    /// A raw acquisition.
    Raw,
    /// An already-processed spectrum.
    Processed,
}

/// Vendor family shared by raw and processed representations.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FormatFamily {
    /// Bruker TopSpin.
    BrukerTopSpin,
    /// Varian or Agilent.
    Varian,
    /// JEOL Delta.
    JeolDelta,
    /// JCAMP-DX.
    JcampDx,
}

/// A precise supported input format.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Format {
    /// Raw acquisition format.
    Raw(RawFormat),
    /// Already-processed spectrum format.
    Processed(processed::Format),
}

impl Format {
    /// Returns whether the format is raw or processed.
    pub fn kind(self) -> DatasetKind {
        match self {
            Self::Raw(_) => DatasetKind::Raw,
            Self::Processed(_) => DatasetKind::Processed,
        }
    }

    /// Returns the vendor or interchange family.
    pub fn family(self) -> FormatFamily {
        match self {
            Self::Raw(RawFormat::BrukerRaw) | Self::Processed(processed::Format::BrukerTopSpin) => {
                FormatFamily::BrukerTopSpin
            }
            Self::Raw(RawFormat::VarianRaw) => FormatFamily::Varian,
            Self::Raw(RawFormat::JeolDelta) | Self::Processed(processed::Format::JeolDelta) => {
                FormatFamily::JeolDelta
            }
            Self::Processed(processed::Format::JcampDx) => FormatFamily::JcampDx,
        }
    }
}

impl From<RawFormat> for Format {
    fn from(value: RawFormat) -> Self {
        Self::Raw(value)
    }
}

impl From<processed::Format> for Format {
    fn from(value: processed::Format) -> Self {
        Self::Processed(value)
    }
}

/// Portable identity fields established without sequence-name guessing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DatasetIdentity {
    subject: Option<String>,
    acquisition: Option<String>,
    source_label: Option<String>,
}

impl DatasetIdentity {
    pub(crate) fn new(
        subject: Option<String>,
        acquisition: Option<String>,
        source_label: Option<String>,
    ) -> Self {
        Self {
            subject: clean(subject),
            acquisition: clean(acquisition),
            source_label: clean(source_label),
        }
    }
    /// Returns the subject label when established.
    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }
    /// Returns the explicit acquisition or experiment label when established.
    pub fn acquisition(&self) -> Option<&str> {
        self.acquisition.as_deref()
    }
    /// Returns the source-local label when established.
    pub fn source_label(&self) -> Option<&str> {
        self.source_label.as_deref()
    }
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Portable metadata field implicated by a warning.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataField {
    /// Subject identity.
    Subject,
    /// Explicit acquisition or experiment identity.
    Acquisition,
    /// Source-local identity label.
    SourceLabel,
    /// Nucleus label.
    Nucleus,
    /// Spectral width.
    SpectralWidth,
    /// Observe frequency.
    ObserveFrequency,
    /// Spectrum reference frequency used for ppm-to-Hz intervals.
    ReferenceFrequency,
    /// Referencing offset.
    Offset,
}

/// Scientific effect of omitted optional information.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WarningImpact {
    /// Portable dataset identity.
    Identity,
    /// Numeric axis calibration.
    AxisCalibration,
    /// Descriptive axis annotation.
    AxisAnnotation,
}

/// Non-fatal issue retained by a successful read.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadWarning {
    /// Reading explicitly accepted vendor interpretations with incomplete independent evidence.
    #[non_exhaustive]
    ExperimentalVendorSemantics {
        /// Format whose decoding rules remain experimental.
        format: Format,
        /// Layout-specific evidence gaps, stored explicitly in the snapshot record.
        details: Vec<String>,
    },
    /// An optional source was absent.
    #[non_exhaustive]
    MissingOptionalSource {
        /// Expected source classification.
        kind: crate::provenance::SourceKind,
        /// Expected vendor-defined role.
        role: String,
        /// Expected source path.
        path: PathBuf,
        /// Scientific effect of its absence.
        impact: WarningImpact,
    },
    /// Portable metadata was absent.
    #[non_exhaustive]
    MissingMetadata {
        /// Missing portable field.
        field: MetadataField,
        /// Axis index when the field is axis-specific.
        axis: Option<usize>,
        /// Scientific effect of its absence.
        impact: WarningImpact,
    },
}

/// Policy for choosing between raw and processed candidates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReadPreference {
    /// Prefer a processed candidate when both kinds have structurally complete candidates.
    ///
    /// When any structurally complete candidate exists, candidates missing required
    /// primary or companion files are excluded before this preference is applied.
    PreferProcessed,
    /// Prefer a raw candidate when both kinds have structurally complete candidates.
    ///
    /// When any structurally complete candidate exists, candidates missing required
    /// primary or companion files are excluded before this preference is applied.
    PreferRaw,
    /// Reject raw-versus-processed ambiguity among structurally complete candidates.
    #[default]
    RequireUnique,
}

/// Builder for unified detection and materialized reads.
#[derive(Clone, Debug, Default)]
pub struct ReadOptions {
    format: Option<Format>,
    preference: ReadPreference,
    limits: ReadLimits,
    varian: Option<crate::formats::varian::ReadAssertions>,
    experimental_vendor_semantics: bool,
    sampling: Option<std::sync::Arc<crate::SamplingDeclaration>>,
}

impl ReadOptions {
    /// Creates bounded defaults with [`ReadPreference::RequireUnique`].
    pub fn new() -> Self {
        Self::default()
    }
    /// Opts into vendor interpretations with incomplete independent evidence.
    ///
    /// Required for Bruker NUS and all raw and processed JEOL imports. Accepted
    /// Bruker NUS schedules do not emit a blanket experimental warning. All JEOL
    /// imports retain an [`ReadWarning::ExperimentalVendorSemantics`] warning
    /// with layout-specific evidence gaps, including raw NUS.
    /// Experimental interpretations may change independently of stable decoding
    /// rules. Unsupported layouts remain rejected.
    pub fn allow_experimental_vendor_semantics(mut self, value: bool) -> Self {
        self.experimental_vendor_semantics = value;
        self
    }
    /// Adds an exact format assertion.
    pub fn format(mut self, value: Format) -> Self {
        self.format = Some(value);
        self
    }
    /// Sets raw-versus-processed selection policy.
    pub fn preference(mut self, value: ReadPreference) -> Self {
        self.preference = value;
        self
    }
    /// Replaces resource limits.
    pub fn limits(mut self, value: ReadLimits) -> Self {
        self.limits = value;
        self
    }
    /// Supplies complete Varian acquisition assertions.
    pub fn varian_assertions(mut self, value: crate::formats::varian::ReadAssertions) -> Self {
        self.varian = Some(value);
        self
    }
    /// Supplies an explicit sampling table for a supported raw Bruker or JEOL
    /// NUS acquisition. Conflicts and unsupported formats are errors. The
    /// original vendor files and caller declaration are retained independently.
    pub fn sampling_declaration(mut self, value: crate::SamplingDeclaration) -> Self {
        self.sampling = Some(std::sync::Arc::new(value));
        self
    }
    /// Detects and selects a format without decoding all samples.
    ///
    /// Recognition is not a support probe. A successful result does not promise
    /// that [`Self::read`] supports the selected scientific layout.
    pub fn detect(&self, path: impl AsRef<Path>) -> Result<Format, ReadError> {
        Ok(resolve_and_select(path.as_ref(), self.format, self.preference)?.format)
    }
    /// Detects, selects, and materializes one dataset.
    pub fn read(&self, path: impl AsRef<Path>) -> Result<Dataset, ReadError> {
        self.read_with_context(path, &mut crate::ExecutionContext::default())
    }

    /// Detects, selects and reads with cooperative execution control.
    pub fn read_with_context(
        &self,
        path: impl AsRef<Path>,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<Dataset, ReadError> {
        control.begin(crate::execution::ExecutionStage::Reading, None, None)?;
        let selected_path = path.as_ref().to_path_buf();
        let candidate = resolve_and_select(path.as_ref(), self.format, self.preference)?;
        self.read_candidate(candidate, selected_path, control)
    }

    fn read_candidate(
        &self,
        candidate: ResolvedCandidate,
        selected_path: PathBuf,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<Dataset, ReadError> {
        control.check_cancelled()?;
        candidate.validate_required()?;
        let limits = if let Some(sampling) = &self.sampling {
            if !matches!(
                candidate.format,
                Format::Raw(RawFormat::BrukerRaw | RawFormat::JeolDelta)
            ) {
                return Err(ReadError::sampling_declaration(
                    "sampling declarations require supported raw Bruker or JEOL NUS input",
                ));
            }
            super::sampling::limits(sampling, self.limits)?
        } else {
            self.limits
        };
        let (dataset, identity, mut warnings) = match candidate.format {
            Format::Raw(_) => {
                let opened = candidate.open_raw(
                    &limits,
                    self.varian.as_ref(),
                    self.experimental_vendor_semantics,
                    self.sampling.as_ref(),
                    control.cancellation(),
                )?;
                let value = opened.into_dataset_with_context(control)?;
                let identity = raw_identity(&candidate, &value);
                let warnings = Vec::new();
                (
                    Dataset::from_raw_with_context(value, control)?,
                    identity,
                    warnings,
                )
            }
            Format::Processed(format) => {
                let result = crate::reading::processed::read(
                    control,
                    &candidate,
                    format,
                    self.limits,
                    self.experimental_vendor_semantics,
                )?;
                (
                    Dataset::from_processed(result.dataset),
                    candidate.identity(result.acquisition),
                    result.warnings,
                )
            }
        };
        if candidate.format.family() == FormatFamily::JeolDelta {
            let parameters = if let Some(raw) = dataset.as_raw() {
                raw.provenance().source_metadata().as_jeol()
            } else {
                dataset.as_processed().and_then(|processed| {
                    processed
                        .provenance()
                        .source_metadata()
                        .jeol_delta()
                        .map(|metadata| metadata.parameters())
                })
            };
            warnings.push(ReadWarning::ExperimentalVendorSemantics {
                format: candidate.format,
                details: crate::formats::jeol::experimental_details(
                    parameters.expect("JEOL readers retain vendor metadata"),
                    candidate.format.kind(),
                    dataset
                        .as_raw()
                        .is_some_and(|raw| raw.sampling_schedule().is_some()),
                ),
            });
        }
        control.check_cancelled()?;
        dataset.attach_read_context(candidate.format, identity, selected_path, warnings)
    }
}

fn raw_identity(candidate: &ResolvedCandidate, dataset: &RawDataset) -> DatasetIdentity {
    match candidate.format {
        Format::Raw(RawFormat::BrukerRaw) => {
            let acquisition = dataset
                .provenance()
                .source_metadata()
                .as_bruker()
                .and_then(|parameters| parameters.direct().get("EXP"))
                .map(clean_vendor_text);
            candidate.identity(acquisition)
        }
        Format::Raw(RawFormat::VarianRaw) => {
            let subject = dataset
                .provenance()
                .source_metadata()
                .as_varian()
                .and_then(|parameters| {
                    ["samplename", "sample", "name", "filename"]
                        .into_iter()
                        .find_map(|name| {
                            parameters
                                .get(name)
                                .and_then(|record| record.values().first())
                                .and_then(crate::formats::varian::ParameterValue::as_str)
                                .map(str::to_owned)
                        })
                })
                .or_else(|| {
                    candidate
                        .root
                        .extension()
                        .is_some_and(|value| value.eq_ignore_ascii_case("fid"))
                        .then(|| {
                            candidate
                                .root
                                .file_stem()
                                .map(|value| value.to_string_lossy().into_owned())
                        })
                        .flatten()
                });
            DatasetIdentity::new(subject, None, None)
        }
        Format::Raw(RawFormat::JeolDelta) => {
            let acquisition =
                dataset
                    .provenance()
                    .source_metadata()
                    .as_jeol()
                    .and_then(|parameters| {
                        ["experiment", "content"].into_iter().find_map(|name| {
                            parameters
                                .get(name)
                                .and_then(|record| match record.value() {
                                    crate::formats::jeol::ParameterValue::String(value) => {
                                        Some(clean_jxp(value))
                                    }
                                    _ => None,
                                })
                        })
                    });
            DatasetIdentity::new(None, acquisition, None)
        }
        Format::Processed(_) => unreachable!(),
    }
}

fn clean_jxp(value: &str) -> String {
    let value = value.trim();
    value
        .get(..value.len().saturating_sub(4))
        .filter(|_| {
            value
                .get(value.len().saturating_sub(4)..)
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".jxp"))
        })
        .unwrap_or(value)
        .to_owned()
}

fn clean_vendor_text(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| character == '<' || character == '>')
        .trim()
        .to_owned()
}

/// Detects and selects a path using default selection policy.
///
/// Recognition is not a support probe. A successful result does not promise
/// that [`read`] supports the selected scientific layout.
pub fn detect(path: impl AsRef<Path>) -> Result<Format, ReadError> {
    ReadOptions::new().detect(path)
}

/// Materializes a path using default selection policy.
///
/// Recognized but unimplemented layouts return [`crate::ReadErrorKind::UnsupportedFeature`].
/// Corrupt, truncated, invalid, allocation, and limit failures remain fatal and
/// must not be treated as permission to reinterpret the input with another reader.
pub fn read(path: impl AsRef<Path>) -> Result<Dataset, ReadError> {
    ReadOptions::new().read(path)
}

impl From<ProcessedValidationError> for ReadError {
    fn from(error: ProcessedValidationError) -> Self {
        ReadError::new(None, ReadErrorReason::ProcessedModel(error))
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl DatasetIdentity {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Option<String>, &Option<String>, &Option<String>) {
        (&self.subject, &self.acquisition, &self.source_label)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Option<String>, Option<String>, Option<String>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (subject, acquisition, source_label) = parts;
        let value = Self {
            subject,
            acquisition,
            source_label,
        };

        Ok(value)
    }
}

/// Reads with default selection policy and shared execution control.
pub fn read_with_context(
    path: impl AsRef<Path>,
    control: &mut crate::ExecutionContext<'_>,
) -> Result<Dataset, ReadError> {
    ReadOptions::new().read_with_context(path, control)
}
