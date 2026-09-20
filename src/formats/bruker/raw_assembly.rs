use super::context::ErrorContext;
use super::metadata::AxisLayout;
use super::metadata::acquisition_metadata;
use super::metadata::normalized_axis;
use super::parameters::ParameterFile;
use super::parameters::Parameters;
use super::sample_decode::decode_complex_pairs;
use super::semantics::group_delay;
use super::semantics::resolved_descriptor;
use crate::Acquisition;
use crate::AcquisitionData;
use crate::Complex64;
use crate::ReadError;
use crate::VendorMetadata;
use crate::acquisition::DirectSamples;
use crate::acquisition::RawAxisKind;
use crate::raw::RawFormat;
use crate::raw::RawProvenance;

use super::storage::direct_storage;

pub(crate) fn decode_one_dimensional(
    bytes: &[u8],
    parameters: ParameterFile,
    acqus_source: &(impl ErrorContext + ?Sized),
    fid_source: &(impl ErrorContext + ?Sized),
) -> Result<Acquisition, ReadError> {
    let samples = decode_samples(bytes, &parameters, acqus_source, fid_source)?;
    let points = samples.len();
    let group_delay = group_delay(&parameters, acqus_source)?;
    let axis = normalized_axis(
        &parameters,
        acqus_source,
        AxisLayout {
            kind: RawAxisKind::Direct(DirectSamples::Complex),
            points,
            label: "F1",
            group_delay,
        },
    )?;
    let descriptor =
        resolved_descriptor(vec![axis], acquisition_metadata(&parameters, acqus_source)?)?;
    let data = AcquisitionData::dense(descriptor.layout().clone(), samples)?;
    Ok(Acquisition::from_reader(
        descriptor,
        data,
        RawProvenance::reader(
            RawFormat::BrukerRaw,
            Vec::new(),
            VendorMetadata::bruker(Parameters::new(vec![parameters])),
        ),
        None,
    )?)
}

pub(crate) fn decode_samples(
    bytes: &[u8],
    parameters: &ParameterFile,
    acqus_source: &(impl ErrorContext + ?Sized),
    fid_source: &(impl ErrorContext + ?Sized),
) -> Result<Vec<Complex64>, ReadError> {
    let storage = direct_storage(parameters, acqus_source)?;
    let sample_bytes = storage.sample.bytes();
    let expected = storage
        .td
        .checked_mul(sample_bytes)
        .ok_or(ReadError::SizeOverflow)?;
    if bytes.len() < expected {
        return Err(ReadError::truncated(
            fid_source.input_source(),
            expected,
            bytes.len(),
        ));
    }
    if bytes[expected..].iter().any(|byte| *byte != 0) {
        return Err(ReadError::corrupt(
            fid_source.input_source(),
            "bytes remain beyond the declared fid payload",
        ));
    }

    decode_complex_pairs(
        &bytes[..expected],
        storage,
        "fid",
        fid_source,
        storage.td / 2,
    )
}
