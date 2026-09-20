use crate::execution::CancellationToken;
use crate::execution::ExecutionError;
use crate::raw::RawData;
use crate::raw::RawDataset;
use crate::raw::RawDescriptor;

use super::encoding::{Encoder, encode_axis, encode_evidence};
use super::{CanonicalDatasetDigests, CanonicalDigest};

const DESCRIPTOR_PROTOCOL: &[u8] = b"nmr.raw-descriptor.v1\0";
const SAMPLES_PROTOCOL: &[u8] = b"nmr.raw-samples.v1\0";
const DATASET_PROTOCOL: &[u8] = b"nmr.raw-dataset.v1\0";

pub(crate) fn descriptor_digest_controlled(
    descriptor: &RawDescriptor,
    cancellation: Option<&CancellationToken>,
) -> Result<CanonicalDigest, ExecutionError> {
    let mut out = Encoder::new_checked(DESCRIPTOR_PROTOCOL, cancellation);
    out.usize(descriptor.axes().len());
    for axis in descriptor.axes() {
        out.check()?;
        encode_axis(&mut out, axis)?;
    }
    out.usize(descriptor.layout().absolute_origin().len());
    for &origin in descriptor.layout().absolute_origin() {
        out.check()?;
        out.usize(origin);
    }
    out.u8(0); // StorageOrder::RowMajorDirectFastest
    encode_evidence(&mut out, descriptor.layout_evidence())?;
    let metadata = descriptor.acquisition();
    out.option_str(metadata.solvent());
    out.option_f64(metadata.temperature_kelvin());
    out.option_u64(metadata.scans());
    out.option_str(metadata.pulse_program());
    match metadata.diffusion() {
        None => out.u8(0),
        Some(value) => {
            out.u8(1);
            out.usize(value.gradient_axis());
            out.str(value.gradient_parameter());
            out.f64(value.gradient_pulse_duration_seconds());
            out.str(value.gradient_pulse_duration_parameter());
            out.f64(value.diffusion_time_seconds());
            out.str(value.diffusion_time_parameter());
            out.option_f64(value.recovery_delay_seconds());
            out.option_str(value.recovery_delay_parameter());
            out.option_str(value.gradient_shape());
            out.option_str(value.gradient_shape_parameter());
        }
    }
    out.finish_checked()
}

pub(crate) fn sample_digest_controlled(
    data: &RawData,
    cancellation: Option<&CancellationToken>,
) -> Result<CanonicalDigest, ExecutionError> {
    let mut out = Encoder::new_checked(SAMPLES_PROTOCOL, cancellation);
    out.usize(data.shape().len());
    for &value in data.shape() {
        out.check()?;
        out.usize(value);
    }
    for &value in data.component_lanes() {
        out.check()?;
        out.usize(value);
    }
    for &value in data.layout().absolute_origin() {
        out.check()?;
        out.usize(value);
    }
    if let Some(samples) = data.dense_samples() {
        out.u8(0);
        out.usize(samples.len());
        for &sample in samples {
            out.check()?;
            out.complex(sample);
        }
    } else {
        let traces = data
            .sparse_traces()
            .expect("raw data representation is exhaustive");
        out.u8(1);
        out.usize(traces.len());
        for trace in traces {
            out.check()?;
            out.usize(trace.ordinal().get());
            out.usize(trace.coordinate().as_slice().len());
            for &coordinate in trace.coordinate().as_slice() {
                out.check()?;
                out.usize(coordinate);
            }
            out.usize(trace.samples().len());
            for &sample in trace.samples() {
                out.check()?;
                out.complex(sample);
            }
        }
    }
    out.finish_checked()
}

pub(crate) fn dataset_digests(dataset: &RawDataset) -> CanonicalDatasetDigests {
    dataset_digests_controlled(dataset, None).expect("uncancelled identity calculation")
}

pub(crate) fn dataset_digests_controlled(
    dataset: &RawDataset,
    cancellation: Option<&CancellationToken>,
) -> Result<CanonicalDatasetDigests, ExecutionError> {
    let descriptor = descriptor_digest_controlled(dataset.descriptor(), cancellation)?;
    let samples = sample_digest_controlled(dataset.data(), cancellation)?;
    raw_binding(
        descriptor,
        samples,
        dataset.sampling_schedule(),
        cancellation,
    )
}

pub(crate) fn raw_binding(
    descriptor: CanonicalDigest,
    samples: CanonicalDigest,
    schedule: Option<&crate::raw::SamplingSchedule>,
    cancellation: Option<&CancellationToken>,
) -> Result<CanonicalDatasetDigests, ExecutionError> {
    let mut out = Encoder::new_checked(DATASET_PROTOCOL, cancellation);
    out.bytes(descriptor.as_bytes());
    out.bytes(samples.as_bytes());
    match schedule {
        None => out.u8(0),
        Some(schedule) => {
            out.u8(1);
            out.usize(schedule.grid().len());
            for &value in schedule.grid() {
                out.check()?;
                out.usize(value);
            }
            out.usize(schedule.coordinates().len());
            for coordinate in schedule.coordinates() {
                out.check()?;
                for &value in coordinate.as_slice() {
                    out.check()?;
                    out.usize(value);
                }
            }
            if let Some(declaration) = schedule.declaration() {
                out.bytes(b"nmr.sampling-declaration.v1\0");
                out.option_str(Some(declaration.assertion().as_str()));
                out.option_str(Some(declaration.source()));
                out.u8(u8::from(
                    declaration.index_base() == crate::SamplingIndexBase::One,
                ));
                for &lanes in declaration.indirect_lanes() {
                    out.check()?;
                    out.usize(lanes);
                }
            }
        }
    }
    Ok(CanonicalDatasetDigests::new(
        descriptor,
        samples,
        out.finish_checked()?,
    ))
}
