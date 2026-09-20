use super::{
    ReportError,
    json::{Encode, Json},
    mapping::Report,
};
use crate::execution::{ExecutionContext, ExecutionStage};
use crate::io::ControlledWriter;
use crate::processed::ProcessedDataset;
use std::io::Write;

/// Frozen JSON field and tag protocol used by this module.
pub const REPORT_SCHEMA: &str = "nmr.execution-report.v1";

/// A caller-authored algorithm statement, kept separate from library execution history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAlgorithmDeclaration {
    pub(super) algorithm: String,
    pub(super) version: String,
    pub(super) statement: String,
    pub(super) parameter_format: String,
    pub(super) parameters: Vec<u8>,
}
impl ExternalAlgorithmDeclaration {
    /// Creates a declaration; it does not establish library execution or result quality.
    pub fn new(
        algorithm: impl Into<String>,
        version: impl Into<String>,
        statement: impl Into<String>,
    ) -> Result<Self, ReportError> {
        let value = Self {
            algorithm: algorithm.into(),
            version: version.into(),
            statement: statement.into(),
            parameter_format: "application/octet-stream".into(),
            parameters: Vec::new(),
        };
        if value.algorithm.trim().is_empty()
            || value.version.trim().is_empty()
            || value.statement.trim().is_empty()
        {
            return Err(ReportError::Invalid(
                "external declaration fields must be nonempty",
            ));
        }
        Ok(value)
    }
    /// Attaches exact host parameters with an explicit format/version identifier.
    /// Random algorithms should include their seed in this payload.
    pub fn with_parameters(
        mut self,
        format: impl Into<String>,
        parameters: Vec<u8>,
    ) -> Result<Self, ReportError> {
        self.parameter_format = format.into();
        if self.parameter_format.trim().is_empty() {
            return Err(ReportError::Invalid("empty external parameter format"));
        }
        self.parameters = parameters;
        Ok(self)
    }
    /// Host algorithm identifier.
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }
    /// Host algorithm version.
    pub fn version(&self) -> &str {
        &self.version
    }
    /// Human explanation supplied by the host.
    pub fn statement(&self) -> &str {
        &self.statement
    }
    /// Format identifier for the opaque parameter bytes.
    pub fn parameter_format(&self) -> &str {
        &self.parameter_format
    }
    /// Exact parameters, interpreted only by the host algorithm.
    pub fn parameters(&self) -> &[u8] {
        &self.parameters
    }
}

/// Writes a deterministic JSON report after validating the entire report and its byte budget.
///
/// This makes two streaming passes without building a JSON tree or copying samples.
/// The first pass rejects invalid numbers and limits before touching the supplied
/// writer. I/O failures during the second pass may leave a prefix; callers should
/// use a temporary file when publishing a report. `max_bytes` bounds UTF-8 output,
/// not OS buffers. No samples are included; preserve original input data separately.
/// Declarations remain explicitly caller-authored and cannot forge library history.
pub fn write_json(
    dataset: &ProcessedDataset,
    declarations: &[ExternalAlgorithmDeclaration],
    writer: &mut impl Write,
    max_bytes: usize,
) -> Result<(), ReportError> {
    write_json_with_context(
        dataset,
        declarations,
        writer,
        max_bytes,
        &mut ExecutionContext::default(),
    )
}
/// Writes a report with cooperative cancellation during validation and encoding.
pub fn write_json_with_context(
    dataset: &ProcessedDataset,
    declarations: &[ExternalAlgorithmDeclaration],
    writer: &mut impl Write,
    max_bytes: usize,
    control: &mut ExecutionContext<'_>,
) -> Result<(), ReportError> {
    control.begin(ExecutionStage::Report, None, None)?;
    for block in dataset.data().samples().chunks(4096) {
        control.check_cancelled()?;
        if block.iter().any(|v| !v.is_finite()) {
            return Err(ReportError::NonFinite);
        }
    }
    let report = Report {
        dataset,
        declarations,
    };
    report.encode(&mut Json {
        writer: &mut ControlledWriter {
            writer: &mut std::io::sink(),
            control,
            count_io: false,
        },
        written: 0,
        limit: max_bytes,
    })?;
    control.begin(ExecutionStage::Report, None, None)?;
    report.encode(&mut Json {
        writer: &mut ControlledWriter {
            writer,
            control,
            count_io: true,
        },
        written: 0,
        limit: max_bytes,
    })
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ExternalAlgorithmDeclaration {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&String, &String, &String, &String, &Vec<u8>) {
        (
            &self.algorithm,
            &self.version,
            &self.statement,
            &self.parameter_format,
            &self.parameters,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (String, String, String, String, Vec<u8>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (algorithm, version, statement, parameter_format, parameters) = parts;
        let value = Self {
            algorithm,
            version,
            statement,
            parameter_format,
            parameters,
        };

        if value.algorithm.trim().is_empty()
            || value.version.trim().is_empty()
            || value.statement.trim().is_empty()
            || value.parameter_format.trim().is_empty()
        {
            return Err(crate::internal::ModelError::Structure);
        }

        Ok(value)
    }
}
