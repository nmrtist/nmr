use super::context::ErrorContext;
use super::context::parameter_error;
use super::context::parse_finite;

use crate::raw::{InputSource, ParameterError, ParameterErrorKind, RawFormat};
use std::path::Path;

/// Raw scalar and array-valued entries from a Bruker JCAMP parameter file.
///
/// Values are retained as text so applications can inspect vendor parameters
/// that this crate does not interpret yet. Array ranges and element types are
/// intentionally not validated or converted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParameterFile {
    pub(super) entries: Vec<(String, String)>,
    pub(super) title: Option<String>,
    pub(super) raw_text: std::sync::Arc<String>,
}

#[cfg(test)]
thread_local! {
    pub(super) static PARAMETER_PARSER_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Raw Bruker status parameter files in acquisition-dimension order.
///
/// The direct file (`acqus`) is first, followed by `acqu2s`, `acqu3s`, and so
/// on in acquisition-dimension order. This is deliberately distinct from
/// normalized axis order, where the slowest indirect axis is first and the
/// direct axis last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameters {
    pub(super) files: Vec<ParameterFile>,
}

impl Parameters {
    pub(super) fn new(files: Vec<ParameterFile>) -> Self {
        debug_assert!(!files.is_empty());
        Self { files }
    }

    /// Returns the direct acquisition status parameters from `acqus`.
    pub fn direct(&self) -> &ParameterFile {
        &self.files[0]
    }

    /// Returns an indirect status file, where index zero denotes `acqu2s`.
    pub fn indirect(&self, index: usize) -> Option<&ParameterFile> {
        self.files.get(index + 1)
    }

    /// Returns all files in direct-first acquisition-dimension order.
    pub fn files(&self) -> &[ParameterFile] {
        &self.files
    }
}

impl ParameterFile {
    /// Parses all `##$NAME=` records in a Bruker parameter file.
    pub fn parse(text: &str) -> Result<Self, ParameterError> {
        Self::parse_at_source(text, InputSource::memory("acqus"))
    }

    pub(super) fn parse_at(text: &str, path: &Path) -> Result<Self, ParameterError> {
        Self::parse_at_source(text, path.into())
    }

    pub(super) fn parse_at_source(text: &str, source: InputSource) -> Result<Self, ParameterError> {
        #[cfg(test)]
        PARAMETER_PARSER_CALLS.with(|calls| calls.set(calls.get() + 1));
        let record_count = text.lines().filter(|line| line.starts_with("##$")).count();
        let requested_bytes = record_count
            .checked_mul(std::mem::size_of::<(String, String)>())
            .filter(|&bytes| bytes <= isize::MAX as usize)
            .ok_or_else(|| parameter_storage_error(&source, ParameterErrorKind::SizeOverflow))?;
        let mut entries = Vec::new();
        entries.try_reserve_exact(record_count).map_err(|_| {
            parameter_storage_error(&source, ParameterErrorKind::Allocation { requested_bytes })
        })?;
        let mut title = None;
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            if let Some(value) = line.strip_prefix("##TITLE=") {
                let value = value.trim();
                if !value.is_empty() {
                    title = Some(parameter_string(value, &source)?);
                }
            }
            let Some(record) = line.strip_prefix("##$") else {
                continue;
            };
            let Some((name, first)) = record
                .split_once('=')
                .filter(|(name, _)| !name.trim().is_empty())
            else {
                return Err(ParameterError::new(
                    RawFormat::BrukerRaw,
                    source.clone(),
                    None,
                    ParameterErrorKind::Malformed,
                    format!("malformed record: {line}"),
                ));
            };
            let first = first.trim();
            // The cloned line iterator borrows text; size the complete value
            // before one allocation, then consume those same continuation lines.
            let capacity = lines
                .clone()
                .take_while(|line| !line.starts_with("##"))
                .filter_map(parameter_continuation)
                .try_fold(first.len(), |length, value| {
                    length
                        .checked_add(usize::from(length != 0))?
                        .checked_add(value.len())
                })
                .ok_or_else(|| {
                    parameter_storage_error(&source, ParameterErrorKind::SizeOverflow)
                })?;
            let mut value = parameter_buffer(capacity, &source)?;
            value.push_str(first);
            while let Some(line) = lines.next_if(|line| !line.starts_with("##")) {
                if let Some(continuation) = parameter_continuation(line) {
                    if !value.is_empty() {
                        value.push('\n');
                    }
                    value.push_str(continuation);
                }
            }
            entries.push((parameter_string(name.trim(), &source)?, value));
        }
        entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        if let Some(pair) = entries.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(ParameterError::new(
                RawFormat::BrukerRaw,
                source,
                Some(pair[0].0.clone()),
                ParameterErrorKind::Duplicate,
                "parameter occurs more than once",
            ));
        }
        Ok(Self {
            entries,
            title,
            raw_text: std::sync::Arc::new(parameter_string(text, &source)?),
        })
    }

    /// Returns complete original text, preserving unknown/global records and comments.
    pub fn raw_text(&self) -> &str {
        &self.raw_text
    }

    /// Returns the global JCAMP title, when present.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Returns a raw parameter value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .binary_search_by(|(candidate, _)| candidate.as_str().cmp(name))
            .ok()
            .map(|index| self.entries[index].1.as_str())
    }

    /// Iterates over parameter names and raw values in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    pub(super) fn required<'a>(
        &'a self,
        name: &'static str,
        source: &(impl ErrorContext + ?Sized),
    ) -> Result<&'a str, ParameterError> {
        self.get(name).ok_or_else(|| {
            parameter_error(
                source,
                Some(name),
                ParameterErrorKind::Missing,
                "required parameter is absent",
            )
        })
    }

    pub(super) fn usize(
        &self,
        name: &'static str,
        source: &(impl ErrorContext + ?Sized),
    ) -> Result<usize, ParameterError> {
        let raw = self.required(name, source)?;
        raw.parse::<usize>().map_err(|_| {
            parameter_error(
                source,
                Some(name),
                ParameterErrorKind::Invalid,
                format!("value {raw:?}: expected a non-negative integer"),
            )
        })
    }

    pub(super) fn integer(
        &self,
        name: &'static str,
        source: &(impl ErrorContext + ?Sized),
    ) -> Result<i64, ParameterError> {
        let raw = self.required(name, source)?;
        raw.parse::<i64>().map_err(|_| {
            parameter_error(
                source,
                Some(name),
                ParameterErrorKind::Invalid,
                format!("value {raw:?}: expected an integer"),
            )
        })
    }

    pub(super) fn optional_integer(
        &self,
        name: &'static str,
        source: &(impl ErrorContext + ?Sized),
    ) -> Result<Option<i64>, ParameterError> {
        self.get(name)
            .map(|raw| {
                raw.parse::<i64>().map_err(|_| {
                    parameter_error(
                        source,
                        Some(name),
                        ParameterErrorKind::Invalid,
                        format!("value {raw:?}: expected an integer"),
                    )
                })
            })
            .transpose()
    }

    pub(super) fn float(
        &self,
        name: &'static str,
        source: &(impl ErrorContext + ?Sized),
    ) -> Result<f64, ParameterError> {
        let raw = self.required(name, source)?;
        parse_finite(raw).ok_or_else(|| {
            parameter_error(
                source,
                Some(name),
                ParameterErrorKind::Invalid,
                format!("value {raw:?}: expected a finite number"),
            )
        })
    }

    pub(super) fn optional_float(
        &self,
        name: &'static str,
        source: &(impl ErrorContext + ?Sized),
    ) -> Result<Option<f64>, ParameterError> {
        self.get(name)
            .map(|raw| {
                parse_finite(raw).ok_or_else(|| {
                    parameter_error(
                        source,
                        Some(name),
                        ParameterErrorKind::Invalid,
                        format!("value {raw:?}: expected a finite number"),
                    )
                })
            })
            .transpose()
    }

    pub(super) fn text(&self, name: &str) -> Option<String> {
        self.get(name)
            .map(|value| value.trim().trim_matches(['<', '>']).trim())
            .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("off"))
            .map(str::to_owned)
    }
}

pub(crate) fn parameter_continuation(line: &str) -> Option<&str> {
    let value = line.trim();
    (!value.is_empty() && !value.starts_with("$$")).then_some(value)
}

pub(crate) fn parameter_storage_error(
    source: &InputSource,
    kind: ParameterErrorKind,
) -> ParameterError {
    ParameterError::new(
        RawFormat::BrukerRaw,
        source.clone(),
        None,
        kind,
        "parameter storage could not be reserved",
    )
}

pub(crate) fn parameter_buffer(
    capacity: usize,
    source: &InputSource,
) -> Result<String, ParameterError> {
    if capacity > isize::MAX as usize {
        return Err(parameter_storage_error(
            source,
            ParameterErrorKind::SizeOverflow,
        ));
    }
    let mut value = String::new();
    value.try_reserve_exact(capacity).map_err(|_| {
        parameter_storage_error(
            source,
            ParameterErrorKind::Allocation {
                requested_bytes: capacity,
            },
        )
    })?;
    Ok(value)
}

pub(crate) fn parameter_string(text: &str, source: &InputSource) -> Result<String, ParameterError> {
    let mut value = parameter_buffer(text.len(), source)?;
    value.push_str(text);
    Ok(value)
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ParameterFile {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Vec<(String, String)>,
        &Option<String>,
        &std::sync::Arc<String>,
    ) {
        (&self.entries, &self.title, &self.raw_text)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Vec<(String, String)>,
            Option<String>,
            std::sync::Arc<String>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (entries, title, raw_text) = parts;
        let value = Self {
            entries,
            title,
            raw_text,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl Parameters {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Vec<ParameterFile>,) {
        (&self.files,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Vec<ParameterFile>,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (files,) = parts;
        let value = Self { files };

        Ok(value)
    }
}
