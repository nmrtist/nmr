use crate::raw::InputSource;
use crate::raw::ParameterError;
use crate::raw::ParameterErrorKind;
use crate::raw::RawFormat;
use std::path::Path;
use std::path::PathBuf;

pub(crate) trait ErrorContext {
    fn input_source(&self) -> InputSource;
}

impl ErrorContext for Path {
    fn input_source(&self) -> InputSource {
        self.into()
    }
}

impl ErrorContext for PathBuf {
    fn input_source(&self) -> InputSource {
        self.as_path().into()
    }
}

impl ErrorContext for InputSource {
    fn input_source(&self) -> InputSource {
        self.clone()
    }
}

pub(crate) fn parse_finite(value: &str) -> Option<f64> {
    value
        .trim()
        .parse()
        .ok()
        .filter(|value: &f64| value.is_finite())
}

pub(crate) fn parameter_error(
    source: &(impl ErrorContext + ?Sized),
    parameter: Option<&str>,
    kind: ParameterErrorKind,
    detail: impl Into<String>,
) -> ParameterError {
    ParameterError::new(
        RawFormat::BrukerRaw,
        source.input_source(),
        parameter.map(str::to_owned),
        kind,
        detail,
    )
}

pub(crate) fn positive(
    name: &'static str,
    value: f64,
    source: &(impl ErrorContext + ?Sized),
) -> Result<f64, ParameterError> {
    if value > 0.0 {
        Ok(value)
    } else {
        Err(parameter_error(
            source,
            Some(name),
            ParameterErrorKind::Invalid,
            format!("value {value}: expected a positive value"),
        ))
    }
}
