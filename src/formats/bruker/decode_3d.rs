use super::metadata::AxisLayout;
use super::metadata::acquisition_metadata;
use super::metadata::normalized_axis;
use super::parameters::ParameterFile;
use super::parameters::Parameters;
use super::semantics::group_delay;
use super::semantics::indirect_layout;
use super::semantics::indirect_stored_points;
use super::semantics::resolved_descriptor;
use crate::Acquisition;
use crate::AcquisitionData;
use crate::Complex64;
use crate::ReadError;
use crate::VendorMetadata;
use crate::acquisition::AxisIndex;
use crate::acquisition::DirectSamples;
use crate::acquisition::RawAxisKind;
use crate::raw::InputSource;
use crate::raw::RawFormat;
use crate::raw::RawLayout;
use crate::raw::RawProvenance;
use crate::raw::ReadLimits;

use super::storage::decode_ser_row;
use super::storage::direct_storage;
use super::storage::ensure_decode_limit;
use super::storage::trace_stride;

pub(crate) fn decode_three_dimensional(
    ser: &[u8],
    parameter_files: Vec<ParameterFile>,
    nuslist: Option<&str>,
    options: &ReadLimits,
    parameter_sources: &[InputSource],
    ser_source: &InputSource,
    nuslist_source: &InputSource,
) -> Result<Acquisition, ReadError> {
    let direct_source = &parameter_sources[0];
    let fast_source = &parameter_sources[1];
    let slow_source = &parameter_sources[2];
    let direct = &parameter_files[0];
    let fast = &parameter_files[1];
    let slow = &parameter_files[2];
    if nuslist.is_some() {
        return Err(ReadError::unsupported_code(
            nuslist_source.clone(),
            crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
            "three-dimensional Bruker NUS is not yet supported",
        ));
    }
    if direct
        .optional_integer("FnTYPE", direct_source)?
        .unwrap_or(0)
        != 0
    {
        return Err(ReadError::unsupported_code(
            direct_source.clone(),
            crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
            "three-dimensional Bruker FnTYPE must be traditional (0)",
        ));
    }
    if direct
        .optional_integer("AQSEQ", direct_source)?
        .unwrap_or(0)
        != 0
    {
        return Err(ReadError::unsupported_feature(
            direct_source.clone(),
            crate::raw::UnsupportedFeatureCode::NON_SEPARABLE_COMPONENT_LAYOUT,
            None,
            vec!["only Bruker 3D AQSEQ=0 (3-2-1) has a proven separable mapping".to_owned()],
        ));
    }
    let (fast_components, fast_lanes) = indirect_layout(
        fast.integer("FnMODE", fast_source)?,
        AxisIndex::new(1),
        fast_source,
    )?;
    let (slow_components, slow_lanes) = indirect_layout(
        slow.integer("FnMODE", slow_source)?,
        AxisIndex::new(0),
        slow_source,
    )?;
    let stored_fast = indirect_stored_points(fast, fast_source, fast_lanes)?;
    let stored_slow = indirect_stored_points(slow, slow_source, slow_lanes)?;
    let storage = direct_storage(direct, direct_source)?;
    let direct_points = storage.td / 2;
    let payload_bytes = storage
        .td
        .checked_mul(storage.sample.bytes())
        .ok_or(ReadError::SizeOverflow)?;
    let rows = stored_slow
        .checked_mul(stored_fast)
        .ok_or(ReadError::SizeOverflow)?;
    let stride = trace_stride(direct, payload_bytes, rows, ser.len(), direct_source)?;
    let expected = stride.checked_mul(rows).ok_or(ReadError::SizeOverflow)?;
    if ser.len() < expected {
        return Err(ReadError::truncated(
            ser_source.clone(),
            expected,
            ser.len(),
        ));
    }
    if ser.len() > expected {
        return Err(ReadError::corrupt(
            ser_source.clone(),
            "bytes remain beyond the declared 3D ser layout",
        ));
    }
    let decoded_samples = rows
        .checked_mul(direct_points)
        .ok_or(ReadError::SizeOverflow)?;
    let decoded_bytes = decoded_samples
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    ensure_decode_limit(decoded_bytes, options)?;
    let mut samples = vec![Complex64::default(); decoded_samples];
    for row in 0..rows {
        decode_ser_row(
            ser,
            row,
            stride,
            payload_bytes,
            storage,
            ser_source,
            &mut samples[row * direct_points..(row + 1) * direct_points],
        )?;
    }
    let shape = vec![
        stored_slow / slow_lanes,
        stored_fast / fast_lanes,
        direct_points,
    ];
    let component_lanes = vec![slow_lanes, fast_lanes, 1];
    let data = AcquisitionData::dense(RawLayout::from_parts(shape, component_lanes)?, samples)?;
    let slow_axis = normalized_axis(
        slow,
        slow_source,
        AxisLayout {
            kind: RawAxisKind::Indirect(slow_components),
            points: stored_slow / slow_lanes,
            label: "F1",
            group_delay: None,
        },
    )?;
    let fast_axis = normalized_axis(
        fast,
        fast_source,
        AxisLayout {
            kind: RawAxisKind::Indirect(fast_components),
            points: stored_fast / fast_lanes,
            label: "F2",
            group_delay: None,
        },
    )?;
    let direct_axis = normalized_axis(
        direct,
        direct_source,
        AxisLayout {
            kind: RawAxisKind::Direct(DirectSamples::Complex),
            points: direct_points,
            label: "F3",
            group_delay: group_delay(direct, direct_source)?,
        },
    )?;
    let descriptor = resolved_descriptor(
        vec![slow_axis, fast_axis, direct_axis],
        acquisition_metadata(direct, direct_source)?,
    )?;
    Ok(Acquisition::from_reader(
        descriptor,
        data,
        RawProvenance::reader(
            RawFormat::BrukerRaw,
            Vec::new(),
            VendorMetadata::bruker(Parameters::new(parameter_files)),
        ),
        None,
    )?)
}
