use super::context::parameter_error;
use super::metadata::AxisLayout;
use super::metadata::acquisition_metadata;
use super::metadata::normalized_axis;
use super::parameters::ParameterFile;
use super::parameters::Parameters;
use super::parser;
use super::parts::parts_nus_storage;
use super::semantics::group_delay;
use super::semantics::indirect_layout;
use super::semantics::resolved_descriptor;
use crate::Acquisition;
use crate::AcquisitionData;
use crate::Complex64;
use crate::ReadError;
use crate::SamplingSchedule;
use crate::SparseTrace;
use crate::VendorMetadata;
use crate::acquisition::AxisIndex;
use crate::acquisition::DirectSamples;
use crate::acquisition::RawAxisKind;
use crate::raw::InputSource;
use crate::raw::ParameterErrorKind;
use crate::raw::RawFormat;
use crate::raw::RawLayout;
use crate::raw::RawProvenance;
use crate::raw::ReadLimits;

use super::storage::decode_ser_row;
use super::storage::direct_storage;
use super::storage::ensure_decode_limit;
use super::storage::trace_stride;

pub(crate) struct TwoDimensionalPaths<'a> {
    pub(super) ser: &'a InputSource,
    pub(super) acqus: &'a InputSource,
    pub(super) acqu2s: &'a InputSource,
    pub(super) nuslist: &'a InputSource,
}

pub(crate) fn decode_two_dimensional(
    ser: &[u8],
    parameter_files: Vec<ParameterFile>,
    nuslist: Option<&str>,
    options: &ReadLimits,
    paths: TwoDimensionalPaths<'_>,
) -> Result<Acquisition, ReadError> {
    let acqus_source = paths.acqus;
    let acqu2s_source = paths.acqu2s;
    let ser_source = paths.ser;
    let direct = &parameter_files[0];
    let indirect = &parameter_files[1];

    if direct.optional_integer("AQSEQ", acqus_source)?.unwrap_or(0) != 0 {
        return Err(ReadError::unsupported_code(
            acqus_source.clone(),
            crate::raw::UnsupportedFeatureCode::UNSUPPORTED_RANK,
            "nonzero AQSEQ is not verified for two-dimensional data",
        ));
    }
    let (indirect_components, indirect_lanes) = indirect_layout(
        indirect.integer("FnMODE", acqu2s_source)?,
        AxisIndex::new(0),
        acqu2s_source,
    )?;
    let stored_indirect = indirect.usize("TD", acqu2s_source)?;
    if stored_indirect < indirect_lanes || stored_indirect % indirect_lanes != 0 {
        return Err(parameter_error(
            acqu2s_source,
            Some("TD"),
            ParameterErrorKind::Invalid,
            format!("value {stored_indirect}: expected a trace count divisible by the FnMODE lane count {indirect_lanes}"),
        )
        .into());
    }

    let fn_type = direct
        .optional_integer("FnTYPE", acqus_source)?
        .unwrap_or(0);
    let (grid_stored_indirect, schedule_text) = match fn_type {
        0 => {
            if nuslist.is_some() {
                return Err(ReadError::unsupported_code(
                    paths.nuslist.clone(),
                    crate::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT,
                    "nuslist is not valid when acqus FnTYPE=0",
                ));
            }
            (stored_indirect, None)
        }
        2 => {
            let text = nuslist.ok_or_else(|| {
                ReadError::incomplete(paths.nuslist.clone(), "Bruker FnTYPE=2 requires a nuslist")
            })?;
            let nus_td = indirect.usize("NusTD", acqu2s_source)?;
            if nus_td < stored_indirect || nus_td % indirect_lanes != 0 {
                return Err(parameter_error(
                    acqu2s_source,
                    Some("NusTD"),
                    ParameterErrorKind::Invalid,
                    format!(
                        "value {nus_td}: expected a full-grid trace count divisible by {indirect_lanes} and no smaller than TD"
                    ),
                )
                .into());
            }
            (nus_td, Some(text))
        }
        value => {
            return Err(ReadError::unsupported_code(
                acqus_source.clone(),
                crate::raw::UnsupportedFeatureCode::SAMPLING_LAYOUT,
                format!("acqus FnTYPE={value}; validated modes are 0 (traditional) and 2 (NUS)"),
            ));
        }
    };

    let storage = direct_storage(direct, acqus_source)?;
    let direct_points = storage.td / 2;
    let payload_bytes = storage
        .td
        .checked_mul(storage.sample.bytes())
        .ok_or(ReadError::SizeOverflow)?;
    let stride = trace_stride(
        direct,
        payload_bytes,
        stored_indirect,
        ser.len(),
        acqus_source,
    )?;
    let expected_bytes = stride
        .checked_mul(stored_indirect)
        .ok_or(ReadError::SizeOverflow)?;
    if ser.len() < expected_bytes {
        return Err(ReadError::truncated(
            ser_source.clone(),
            expected_bytes,
            ser.len(),
        ));
    }
    if ser.len() > expected_bytes {
        let allocated = stride
            .checked_mul(grid_stored_indirect)
            .ok_or(ReadError::SizeOverflow)?;
        if fn_type != 2 || ser.len() != allocated {
            return Err(ReadError::corrupt(
                ser_source.clone(),
                "bytes remain beyond the declared ser layout",
            ));
        }
        if ser[expected_bytes..].iter().any(|&byte| byte != 0) {
            return Err(ReadError::corrupt(
                ser_source.clone(),
                "nonzero bytes remain beyond the declared Bruker payload",
            ));
        }
    }
    let decoded_samples = direct_points
        .checked_mul(stored_indirect)
        .ok_or(ReadError::SizeOverflow)?;
    let decoded_bytes = decoded_samples
        .checked_mul(std::mem::size_of::<Complex64>())
        .ok_or(ReadError::SizeOverflow)?;
    let schedule_entries = stored_indirect / indirect_lanes;
    let coordinate_bytes = schedule_text
        .map(|_| parts_nus_storage(schedule_entries))
        .transpose()?
        .unwrap_or(0);
    let peak_decode_bytes = decoded_bytes
        .checked_add(coordinate_bytes)
        .ok_or(ReadError::SizeOverflow)?;
    ensure_decode_limit(peak_decode_bytes, options)?;

    let schedule_coordinates = schedule_text
        .map(|text| {
            parser::parse_nuslist(
                text,
                grid_stored_indirect / indirect_lanes,
                schedule_entries,
                paths.nuslist,
            )
        })
        .transpose()?;

    let logical_indirect = grid_stored_indirect / indirect_lanes;
    let shape = vec![logical_indirect, direct_points];
    let component_lanes = vec![indirect_lanes, 1];
    let (data, sampling) = match schedule_coordinates {
        None => {
            let mut samples = vec![Complex64::new(0.0, 0.0); decoded_samples];
            for row in 0..stored_indirect {
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
            (
                AcquisitionData::dense(
                    RawLayout::from_parts(shape.clone(), component_lanes.clone())?,
                    samples,
                )?,
                None,
            )
        }
        Some(coordinates) => {
            let schedule = SamplingSchedule::new(vec![logical_indirect], coordinates)?;
            let coordinates = schedule.coordinates();
            if schedule.is_complete_unique()? {
                let mut samples = vec![Complex64::new(0.0, 0.0); decoded_samples];
                for (acquisition, coordinate) in coordinates.iter().enumerate() {
                    for lane in 0..indirect_lanes {
                        let source_row = acquisition * indirect_lanes + lane;
                        let target_row = coordinate.as_slice()[0] * indirect_lanes + lane;
                        decode_ser_row(
                            ser,
                            source_row,
                            stride,
                            payload_bytes,
                            storage,
                            ser_source,
                            &mut samples
                                [target_row * direct_points..(target_row + 1) * direct_points],
                        )?;
                    }
                }
                (
                    AcquisitionData::dense(
                        RawLayout::from_parts(shape.clone(), component_lanes.clone())?,
                        samples,
                    )?,
                    Some(schedule),
                )
            } else {
                let trace_bytes = coordinates
                    .len()
                    .checked_mul(std::mem::size_of::<SparseTrace>())
                    .ok_or(ReadError::SizeOverflow)?;
                let mut traces = Vec::new();
                traces
                    .try_reserve_exact(coordinates.len())
                    .map_err(|_| ReadError::allocation(trace_bytes))?;
                for (acquisition, coordinate) in coordinates.iter().enumerate() {
                    let trace_samples = direct_points
                        .checked_mul(indirect_lanes)
                        .ok_or(ReadError::SizeOverflow)?;
                    let mut samples = vec![Complex64::new(0.0, 0.0); trace_samples];
                    for lane in 0..indirect_lanes {
                        decode_ser_row(
                            ser,
                            acquisition * indirect_lanes + lane,
                            stride,
                            payload_bytes,
                            storage,
                            ser_source,
                            &mut samples[lane * direct_points..(lane + 1) * direct_points],
                        )?;
                    }
                    traces.push(SparseTrace::new(
                        crate::raw::ObservationOrdinal::new(acquisition),
                        coordinate.clone(),
                        samples,
                    ));
                }
                (
                    AcquisitionData::sparse(
                        RawLayout::from_parts(shape.clone(), component_lanes.clone())?,
                        traces,
                    )?,
                    Some(schedule),
                )
            }
        }
    };

    let indirect_axis = normalized_axis(
        indirect,
        acqu2s_source,
        AxisLayout {
            kind: RawAxisKind::Indirect(indirect_components),
            points: logical_indirect,
            label: "F1",
            group_delay: None,
        },
    )?;
    let group_delay = group_delay(direct, acqus_source)?;
    let direct_axis = normalized_axis(
        direct,
        acqus_source,
        AxisLayout {
            kind: RawAxisKind::Direct(DirectSamples::Complex),
            points: direct_points,
            label: "F2",
            group_delay,
        },
    )?;
    let descriptor = resolved_descriptor(
        vec![indirect_axis, direct_axis],
        acquisition_metadata(direct, acqus_source)?,
    )?;
    Ok(Acquisition::from_reader(
        descriptor,
        data,
        RawProvenance::reader(
            RawFormat::BrukerRaw,
            Vec::new(),
            VendorMetadata::bruker(Parameters::new(parameter_files)),
        ),
        sampling,
    )?)
}
