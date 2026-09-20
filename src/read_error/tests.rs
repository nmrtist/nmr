use super::*;
use crate::raw::RawFormat;
use std::error::Error as StdError;

#[test]
fn parameter_error_preserves_format_source_and_error_chain() {
    let parameter = ParameterError::new(
        RawFormat::VarianRaw,
        InputSource::memory("procpar"),
        Some("np".into()),
        ParameterErrorKind::Missing,
        "required parameter is absent",
    );
    let error = ReadError::from(parameter).with_format(RawFormat::BrukerRaw);

    assert_eq!(
        error.format(),
        Some(crate::Format::Raw(RawFormat::VarianRaw))
    );
    assert_eq!(error.kind(), ReadErrorKind::InvalidMetadata);
    let ReadErrorReason::InvalidMetadata {
        parameter: Some(parameter),
        ..
    } = error.reason()
    else {
        panic!("parameter error must retain its structured reason");
    };
    assert_eq!(parameter.input_source().path(), None);
    assert_eq!(parameter.input_source().role(), Some("procpar"));
    assert_eq!(parameter.parameter(), Some("np"));

    let reason = StdError::source(&error).expect("read error must expose its reason");
    assert!(reason.downcast_ref::<ReadErrorReason>().is_some());
    let parameter_source = StdError::source(error.reason())
        .expect("invalid metadata reason must expose ParameterError");
    assert!(parameter_source.downcast_ref::<ParameterError>().is_some());
}

#[test]
fn parameter_storage_failures_keep_resource_classification() {
    for (parameter_kind, expected_kind) in [
        (
            ParameterErrorKind::SizeOverflow,
            ReadErrorKind::SizeOverflow,
        ),
        (
            ParameterErrorKind::Allocation {
                requested_bytes: 123,
            },
            ReadErrorKind::Allocation,
        ),
    ] {
        let error = ReadError::from(ParameterError::new(
            RawFormat::BrukerRaw,
            InputSource::memory("acqus"),
            None,
            parameter_kind,
            "storage failure",
        ));
        assert_eq!(error.kind(), expected_kind);
        assert_eq!(
            error.format(),
            Some(crate::Format::Raw(RawFormat::BrukerRaw))
        );
        if expected_kind == ReadErrorKind::Allocation {
            assert!(matches!(
                error.reason(),
                ReadErrorReason::Allocation {
                    requested_bytes: 123
                }
            ));
        }
    }
}

#[test]
fn all_parameter_semantic_errors_map_to_invalid_metadata() {
    for kind in [
        ParameterErrorKind::Duplicate,
        ParameterErrorKind::Malformed,
        ParameterErrorKind::Invalid,
    ] {
        let error = ReadError::from(ParameterError::new(
            RawFormat::BrukerRaw,
            InputSource::memory("acqus"),
            None,
            kind,
            "invalid parameter record",
        ));
        assert_eq!(error.kind(), ReadErrorKind::InvalidMetadata);
    }
}

#[test]
fn sampling_model_failures_preserve_allocation_model_and_execution_categories() {
    use crate::execution::ExecutionError;
    use crate::raw::model::{ValidationError, sampling_declaration::SamplingDeclarationError as S};
    let error = ReadError::from(S::Model(ValidationError::Allocation {
        requested_bytes: 123,
    }));
    assert_eq!(error.kind(), ReadErrorKind::Allocation);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::Allocation {
            requested_bytes: 123
        }
    ));
    let error = ReadError::from(S::Model(ValidationError::SizeOverflow));
    assert_eq!(error.kind(), ReadErrorKind::Model);
    assert!(matches!(
        error.reason(),
        ReadErrorReason::Model(ValidationError::SizeOverflow)
    ));
    for (cause, kind) in [
        (ExecutionError::Cancelled, ReadErrorKind::Cancelled),
        (ExecutionError::WorkLimit, ReadErrorKind::LimitExceeded),
        (ExecutionError::SizeOverflow, ReadErrorKind::SizeOverflow),
    ] {
        let error = ReadError::from(S::Execution(cause));
        assert_eq!(error.kind(), kind);
        assert!(matches!(error.reason(), ReadErrorReason::Execution(found) if *found == cause));
        assert_eq!(error.format(), None);
    }
}
