use nmr::axis::{AxisCoordinates, AxisDomain, AxisQuantity, AxisRole, AxisUnit};
use nmr::export::{ExportError, PLOT_DATA_SCHEMA_ID};
use nmr::plot::PlotData;
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::processing::WorkLedger;
use std::fs;

fn plot() -> PlotData {
    let parameter = ProcessedAxis::new(
        AxisRole::ArrayParameter,
        AxisDomain::Parameter,
        None,
        2,
        AxisCoordinates::Unknown,
        ComponentBasis::Scalar,
    )
    .unwrap()
    .with_label(Some("delay \"index\"".to_owned()));
    let signal = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        3,
        AxisCoordinates::Uniform {
            start: -1.0,
            step: 1.0,
        },
        ComponentBasis::Scalar,
    )
    .unwrap();
    let dataset = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![parameter, signal]).unwrap(),
        ProcessedData::new(vec![2, 3], vec![1, 1], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap();
    PlotData::from_processed(&dataset).unwrap()
}

fn gradient_plot() -> PlotData {
    let parameter = ProcessedAxis::new(
        AxisRole::ArrayParameter,
        AxisDomain::Parameter,
        Some(AxisUnit::TeslaPerMeter),
        2,
        AxisCoordinates::Uniform {
            start: 0.02,
            step: 0.01,
        },
        ComponentBasis::Scalar,
    )
    .unwrap()
    .with_quantity(Some(AxisQuantity::MagneticFieldGradientStrength))
    .unwrap();
    let signal = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        2,
        AxisCoordinates::Uniform {
            start: -1.0,
            step: 1.0,
        },
        ComponentBasis::Scalar,
    )
    .unwrap();
    let dataset = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![parameter, signal]).unwrap(),
        ProcessedData::new(vec![2, 2], vec![1, 1], vec![1.0, 2.0, 3.0, 4.0]).unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Imported, vec![]).unwrap(),
    )
    .unwrap();
    PlotData::from_processed(&dataset).unwrap()
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn local_members(bytes: &[u8]) -> Vec<(String, Vec<u8>, u32)> {
    let mut offset = 0;
    let mut members = Vec::new();
    while le_u32(bytes, offset) == 0x0403_4b50 {
        assert_eq!(le_u16(bytes, offset + 6), 0);
        assert_eq!(le_u16(bytes, offset + 8), 0);
        let crc = le_u32(bytes, offset + 14);
        let compressed = le_u32(bytes, offset + 18) as usize;
        assert_eq!(compressed, le_u32(bytes, offset + 22) as usize);
        let name_len = le_u16(bytes, offset + 26) as usize;
        assert_eq!(le_u16(bytes, offset + 28), 0);
        let name_start = offset + 30;
        let data_start = name_start + name_len;
        let name = std::str::from_utf8(&bytes[name_start..data_start])
            .unwrap()
            .to_owned();
        members.push((
            name,
            bytes[data_start..data_start + compressed].to_vec(),
            crc,
        ));
        offset = data_start + compressed;
    }
    assert_eq!(le_u32(bytes, offset), 0x0201_4b50);
    assert_eq!(le_u32(bytes, bytes.len() - 22), 0x0605_4b50);
    assert_eq!(le_u16(bytes, bytes.len() - 2), 0);
    members
}

fn npy_payload(member: &[u8]) -> (&str, &[u8]) {
    assert_eq!(&member[..8], b"\x93NUMPY\x01\x00");
    let header_len = le_u16(member, 8) as usize;
    assert_eq!((10 + header_len) % 64, 0);
    let header = std::str::from_utf8(&member[10..10 + header_len]).unwrap();
    assert!(header.ends_with('\n'));
    (header, &member[10 + header_len..])
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

#[test]
fn npz_v1_has_fixed_members_headers_and_types() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("plot.npz");
    let mut work = WorkLedger::new(u128::MAX);
    nmr::export::export_npz(&plot(), &target, &mut work).unwrap();
    let bytes = fs::read(target).unwrap();
    assert_eq!(
        bytes,
        include_bytes!("../fixtures/npz/plot-v1.npz"),
        "the complete deterministic NPZ must match the committed V1 golden"
    );
    let members = local_members(&bytes);

    assert_eq!(
        members
            .iter()
            .map(|entry| entry.0.as_str())
            .collect::<Vec<_>>(),
        ["data.npy", "axis_0.npy", "axis_1.npy", "manifest.npy"]
    );
    for (_, contents, expected_crc) in &members {
        assert_eq!(crc32(contents), *expected_crc);
    }
    let expected_work: u128 = members
        .iter()
        .map(|entry| entry.1.len() as u128)
        .sum::<u128>()
        * 2;
    assert_eq!(work.used(), expected_work);

    let (data_header, data) = npy_payload(&members[0].1);
    assert!(data_header.contains("'descr': '<f8'"));
    assert!(data_header.contains("'fortran_order': False"));
    assert!(data_header.contains("'shape': (2, 3)"));
    assert_eq!(data.len(), 6 * 8);

    let (logical_header, logical) = npy_payload(&members[1].1);
    assert!(logical_header.contains("'descr': '<u8'"));
    assert_eq!(
        logical,
        &[0_u64.to_le_bytes(), 1_u64.to_le_bytes()].concat()
    );

    let (physical_header, physical) = npy_payload(&members[2].1);
    assert!(physical_header.contains("'descr': '<f8'"));
    assert_eq!(physical.len(), 3 * 8);

    let (manifest_header, manifest) = npy_payload(&members[3].1);
    assert!(manifest_header.contains("'descr': '|u1'"));
    let manifest = std::str::from_utf8(manifest).unwrap();
    assert!(manifest.contains(PLOT_DATA_SCHEMA_ID));
    assert!(manifest.contains("\"version\":1"));
    assert!(manifest.contains("delay \\\"index\\\""));
}

#[test]
fn existing_target_is_never_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("plot.npz");
    fs::write(&target, b"existing").unwrap();
    let mut work = WorkLedger::new(u128::MAX);
    assert!(matches!(
        nmr::export::export_npz(&plot(), &target, &mut work),
        Err(ExportError::TargetExists)
    ));
    assert_eq!(fs::read(&target).unwrap(), b"existing");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn npz_manifest_preserves_gradient_axis_unit() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("gradient.npz");
    let mut work = WorkLedger::new(u128::MAX);
    nmr::export::export_npz(&gradient_plot(), &target, &mut work).unwrap();
    let bytes = fs::read(target).unwrap();
    let members = local_members(&bytes);
    let (_, manifest) = npy_payload(&members.last().unwrap().1);
    let manifest = std::str::from_utf8(manifest).unwrap();
    assert!(manifest.contains("\"unit\":\"TeslaPerMeter\""));
    assert_eq!(
        gradient_plot().axes()[0].quantity(),
        Some(AxisQuantity::MagneticFieldGradientStrength)
    );
}

#[test]
fn work_limit_fails_before_creating_a_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("plot.npz");
    let mut work = WorkLedger::new(0);
    assert!(matches!(
        nmr::export::export_npz(&plot(), &target, &mut work),
        Err(ExportError::Execution(
            nmr::execution::ExecutionError::WorkLimit
        ))
    ));
    assert!(!target.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}
