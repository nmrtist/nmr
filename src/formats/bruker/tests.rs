use super::metadata;
use super::opening::lazy_nus_storage;
use super::opening::open_resolved_acquisition;
use super::parameters::PARAMETER_PARSER_CALLS;
use super::parameters::ParameterFile;
use super::parser;
use super::parts::Parts;
use super::parts::parts_nus_storage;
use super::parts::read_parts_with_limits;
use super::reader::TraceReader;
use super::storage::parameter_storage_bound;
use crate::Complex64;
use crate::SamplingCoordinate;
use crate::SourceFile;
use crate::SparseTrace;
use crate::raw::ReadLimits;

use super::semantics::bruker_group_delay_fallback;

#[test]
fn large_nus_grid_is_budgeted_before_coordinate_parsing() {
    let acqus = "##$TD= 4\n##$PARMODE= 1\n##$AQ_mod= 3\n##$BYTORDA= 0\n##$DTYPA= 0\n##$SW_h= 8000\n##$SFO1= 400\n##$FnTYPE= 2\n##$GO_block_size= <continuous>\n";
    let acqu2s = "##$TD= 4\n##$NusTD= 200000\n##$FnMODE= 6\n##$SW_h= 1000\n##$SFO1= 100\n";
    let nuslist = "0\n99999\n";
    let directory = tempfile::tempdir().unwrap();
    let ser = directory.path().join("ser");
    let parameters = [
        directory.path().join("acqus"),
        directory.path().join("acqu2s"),
    ];
    let schedule = directory.path().join("nuslist");
    let mut bytes = Vec::new();
    for value in 1_i32..=16 {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(&ser, &bytes).unwrap();
    std::fs::write(&parameters[0], acqus).unwrap();
    std::fs::write(&parameters[1], acqu2s).unwrap();
    std::fs::write(&schedule, nuslist).unwrap();
    let parameter_bytes =
        parameter_storage_bound(acqus).unwrap() + parameter_storage_bound(acqu2s).unwrap();
    let owned = acqus.len()
        + acqu2s.len()
        + nuslist.len()
        + 2 * std::mem::size_of::<String>()
        + 3 * std::mem::size_of::<usize>();
    let retained = 100_000 * std::mem::size_of::<Option<usize>>()
        + 2 * (std::mem::size_of::<SamplingCoordinate>() + std::mem::size_of::<usize>())
        + std::mem::size_of::<usize>();
    let peak = retained + 2 * std::mem::size_of::<&SamplingCoordinate>();
    assert_eq!(lazy_nus_storage(100_000, 2).unwrap(), (retained, peak));
    let source_bytes = 4 * std::mem::size_of::<SourceFile>()
        + std::mem::size_of::<TraceReader>()
        + "seracqusacqu2ssampling_schedule".len()
        + ser.as_os_str().as_encoded_bytes().len()
        + [&ser, &parameters[0], &parameters[1], &schedule]
            .iter()
            .map(|path| path.as_os_str().as_encoded_bytes().len())
            .sum::<usize>();
    let descriptor_bytes = metadata::descriptor_storage_bound(&[
        ParameterFile::parse(acqus).unwrap(),
        ParameterFile::parse(acqu2s).unwrap(),
    ])
    .unwrap();
    let required = parameter_bytes + source_bytes + descriptor_bytes + owned + retained + 80;
    parser::NUS_PARSER_CALLS.with(|calls| calls.set(0));
    let error = open_resolved_acquisition(
        &ser,
        &parameters,
        Some(&schedule),
        &ReadLimits::new().max_working_bytes(10_000),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, ..
        } if *actual == parameter_bytes + source_bytes + descriptor_bytes + owned + peak)
    );
    let error = open_resolved_acquisition(
        &ser,
        &parameters,
        Some(&schedule),
        &ReadLimits::new().max_working_bytes(required - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::TraceBytes, required: actual, ..
        } if *actual == required)
    );
    parser::NUS_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    let reader = open_resolved_acquisition(
        &ser,
        &parameters,
        Some(&schedule),
        &ReadLimits::new().max_working_bytes(required),
    )
    .unwrap();
    parser::NUS_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 1));
    assert_eq!(
        reader.read_trace(&[99_999]).unwrap().samples()[0],
        Complex64::new(9.0, 10.0)
    );
    assert!(lazy_nus_storage(usize::MAX, 2).is_err());
    assert!(lazy_nus_storage(1, usize::MAX).is_err());
    let parts_metadata = 2
        * (std::mem::size_of::<SamplingCoordinate>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<&SamplingCoordinate>()
            + std::mem::size_of::<SparseTrace>()
            + std::mem::size_of::<usize>())
        + std::mem::size_of::<usize>();
    assert_eq!(parts_nus_storage(2).unwrap(), parts_metadata);
    let parts_sources = 4 * std::mem::size_of::<SourceFile>()
        + "seracqusacqu2ssampling_schedule".len()
        + "acqusacqu2sacqu3sfid or sernuslistacqusacqu3s".len();
    let parts_required = parameter_bytes
        + parts_metadata
        + parts_sources
        + descriptor_bytes
        + 10 * std::mem::size_of::<usize>()
        + 128;
    assert!(parts_required < required);
    parser::NUS_PARSER_CALLS.with(|calls| calls.set(0));
    let error = read_parts_with_limits(
        Parts::new(&bytes, &[acqus, acqu2s])
            .nuslist(nuslist)
            .allow_experimental_vendor_semantics(true),
        ReadLimits::new().max_working_bytes(parts_required - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, ..
        } if *actual == parts_required)
    );
    parser::NUS_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    let dataset = read_parts_with_limits(
        Parts::new(&bytes, &[acqus, acqu2s])
            .nuslist(nuslist)
            .allow_experimental_vendor_semantics(true),
        ReadLimits::new().max_working_bytes(parts_required),
    )
    .unwrap();
    parser::NUS_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 1));
    assert_eq!(
        dataset.read_trace(&[99_999]).unwrap().samples()[0],
        Complex64::new(9.0, 10.0)
    );
    assert!(parts_nus_storage(usize::MAX).is_err());
}

#[test]
fn parameter_budget_precedes_parser_for_parts_and_owned_path_text() {
    use super::*;
    let text = "##$TD= 4\n##$PARMODE= 0\n##$AQ_mod= 3\n##$BYTORDA= 0\n##$DTYPA= 0\n##$SW_h= 8000\n##$SFO1= 400\n##END=\n";
    let nucleus = "未知 nucleus ".repeat(512).trim().to_owned();
    let text = format!(
        "##TITLE= synthetic descriptor\n{}##$NUC1= <{nucleus}>\n##$SOLVENT= <D2O>\n##$PULPROG= <zg>\n##$O1= 100\n##$GRPDLY= 0\n##END=\n",
        text.trim_end_matches("##END=\n")
    );
    let text = text.as_str();
    let parsed_bytes = parameter_storage_bound(text).unwrap();
    let required = parsed_bytes
        + 2 * std::mem::size_of::<SourceFile>()
        + "fidacqus".len()
        + "acqusacqu2sacqu3sfid or sernuslistacqusacqu3s".len();
    let bytes = [0u8; 16];
    PARAMETER_PARSER_CALLS.with(|calls| calls.set(0));
    let error = read_parts_with_limits(
        Parts::new(&bytes, &[text]),
        ReadLimits::new().max_working_bytes(required - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, limit,
        } if *actual == required && *limit == required - 1)
    );
    PARAMETER_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    let descriptor_bytes =
        metadata::descriptor_storage_bound(&[ParameterFile::parse(text).unwrap()]).unwrap();
    let required = required + descriptor_bytes + 5 * std::mem::size_of::<usize>();
    metadata::AXIS_BUILD_CALLS.with(|calls| calls.set(0));
    let error = read_parts_with_limits(
        Parts::new(&bytes, &[text]),
        ReadLimits::new().max_working_bytes(required - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, ..
        } if *actual == required)
    );
    metadata::AXIS_BUILD_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    let error = read_parts_with_limits(
        Parts::new(&bytes, &[text]),
        ReadLimits::new().max_working_bytes(required + 31),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, ..
        } if *actual == required + 32)
    );
    PARAMETER_PARSER_CALLS.with(|calls| calls.set(0));
    let dataset = read_parts_with_limits(
        Parts::new(&bytes, &[text]),
        ReadLimits::new().max_working_bytes(required + 32),
    )
    .unwrap();
    assert_eq!(
        dataset.read_trace(&[]).unwrap().samples(),
        &[Complex64::new(0.0, 0.0); 2]
    );
    PARAMETER_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 1));

    let directory = tempfile::tempdir().unwrap();
    let fid = directory.path().join("fid");
    let acqus = directory.path().join("acqus");
    std::fs::write(&fid, bytes).unwrap();
    std::fs::write(&acqus, text).unwrap();
    let source_bytes = 2 * std::mem::size_of::<SourceFile>()
        + std::mem::size_of::<TraceReader>()
        + "fidacqus".len()
        + 2 * fid.as_os_str().as_encoded_bytes().len()
        + acqus.as_os_str().as_encoded_bytes().len();
    PARAMETER_PARSER_CALLS.with(|calls| calls.set(0));
    let error = open_resolved_acquisition(
        &fid,
        std::slice::from_ref(&acqus),
        None,
        &ReadLimits::new().max_working_bytes(source_bytes - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, ..
        } if *actual == source_bytes)
    );
    PARAMETER_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    let required = parsed_bytes
        + source_bytes
        + text.len()
        + std::mem::size_of::<String>()
        + std::mem::size_of::<usize>();
    PARAMETER_PARSER_CALLS.with(|calls| calls.set(0));
    let error = open_resolved_acquisition(
        &fid,
        std::slice::from_ref(&acqus),
        None,
        &ReadLimits::new().max_working_bytes(required - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, limit,
        } if *actual == required && *limit == required - 1)
    );
    PARAMETER_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    let descriptor_bytes =
        metadata::descriptor_storage_bound(&[ParameterFile::parse(text).unwrap()]).unwrap();
    let required = required + descriptor_bytes;
    metadata::AXIS_BUILD_CALLS.with(|calls| calls.set(0));
    let error = open_resolved_acquisition(
        &fid,
        std::slice::from_ref(&acqus),
        None,
        &ReadLimits::new().max_working_bytes(required - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, ..
        } if *actual == required)
    );
    metadata::AXIS_BUILD_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    let error = open_resolved_acquisition(
        &fid,
        std::slice::from_ref(&acqus),
        None,
        &ReadLimits::new().max_working_bytes(required + 47),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::TraceBytes, required: actual, ..
        } if *actual == required + 48)
    );
    PARAMETER_PARSER_CALLS.with(|calls| calls.set(0));
    let reader = open_resolved_acquisition(
        &fid,
        &[acqus],
        None,
        &ReadLimits::new().max_working_bytes(required + 48),
    )
    .unwrap();
    assert_eq!(
        reader.descriptor().axes()[0].nucleus(),
        Some(nucleus.as_str())
    );
    PARAMETER_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 1));
    metadata::AXIS_BUILD_CALLS.with(|calls| assert_eq!(calls.get(), 1));
}

#[test]
fn parameter_tables_are_budgeted_together() {
    use super::*;
    let first = "##$PARMODE= 1\n";
    let second = "##$X= value\n";
    let required = parameter_storage_bound(first).unwrap()
        + parameter_storage_bound(second).unwrap()
        + 3 * std::mem::size_of::<SourceFile>()
        + "seracqusacqu2s".len()
        + "acqusacqu2sacqu3sfid or sernuslistacqusacqu3s".len();
    PARAMETER_PARSER_CALLS.with(|calls| calls.set(0));
    let error = read_parts_with_limits(
        Parts::new(&[], &[first, second]),
        ReadLimits::new().max_working_bytes(required - 1),
    )
    .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes, required: actual, ..
        } if *actual == required)
    );
    PARAMETER_PARSER_CALLS.with(|calls| assert_eq!(calls.get(), 0));
}

#[test]
fn group_delay_fallback_is_exactly_the_verified_10_through_13_whitelist() {
    let common = [
        2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048,
    ];
    for version in 10..=12 {
        for decimation in common {
            assert!(bruker_group_delay_fallback(version, decimation).is_some());
        }
    }
    for decimation in [2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96] {
        assert!(bruker_group_delay_fallback(13, decimation).is_some());
    }
    for version in [0, 9, 14, u32::MAX] {
        assert_eq!(bruker_group_delay_fallback(version, 2), None);
    }
    for version in 10..=13 {
        assert_eq!(bruker_group_delay_fallback(version, 1), None);
    }
    assert_eq!(bruker_group_delay_fallback(13, 128), None);
}
