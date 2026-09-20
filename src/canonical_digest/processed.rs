use crate::acquisition::*;
use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisQuantity;
use crate::axis::AxisUnit;
use crate::execution::CancellationToken;
use crate::execution::ExecutionError;

use super::encoding::{Encoder, encode_evidence, encode_transform};
use super::{CanonicalDatasetDigests, CanonicalDigest};

pub(crate) fn processed_digests(
    descriptor: &crate::processed::ProcessedDescriptor,
    data: &crate::processed::ProcessedData,
    state: &crate::processing::contracts::state::PlanState,
) -> CanonicalDatasetDigests {
    processed_digests_controlled(descriptor, data, state, None)
        .expect("uncancelled identity calculation")
}

pub(crate) fn processed_digests_controlled(
    descriptor: &crate::processed::ProcessedDescriptor,
    data: &crate::processed::ProcessedData,
    state: &crate::processing::contracts::state::PlanState,
    cancellation: Option<&CancellationToken>,
) -> Result<CanonicalDatasetDigests, ExecutionError> {
    let descriptor = processed_descriptor_digest(descriptor, state, cancellation)?;
    let mut out = Encoder::new_checked(b"nmr.processed-samples.v1\0", cancellation);
    out.usize(data.shape().len());
    for &value in data.shape() {
        out.check()?;
        out.usize(value);
    }
    for &value in data.component_counts() {
        out.check()?;
        out.usize(value);
    }
    out.usize(data.samples().len());
    for &value in data.samples() {
        out.check()?;
        out.f64(value);
    }
    let samples = out.finish_checked()?;
    let mut out = Encoder::new_checked(b"nmr.processed-dataset.v1\0", cancellation);
    out.bytes(descriptor.as_bytes());
    out.bytes(samples.as_bytes());
    Ok(CanonicalDatasetDigests::new(
        descriptor,
        samples,
        out.finish_checked()?,
    ))
}

pub(crate) fn processed_descriptor_digest(
    descriptor: &crate::processed::ProcessedDescriptor,
    state: &crate::processing::contracts::state::PlanState,
    cancellation: Option<&CancellationToken>,
) -> Result<CanonicalDigest, ExecutionError> {
    use crate::axis::AxisRole;
    use crate::processing::contracts::state::ProcessingDelayState;
    let mut out = Encoder::new_checked(b"nmr.processed-descriptor.v1\0", cancellation);
    out.usize(descriptor.axes().len());
    for axis in descriptor.axes() {
        out.check()?;
        out.u8(match axis.role() {
            AxisRole::DirectAcquisition => 0,
            AxisRole::IndirectAcquisition => 1,
            AxisRole::ArrayParameter => 2,
            AxisRole::Signal => 3,
            AxisRole::Unknown => 4,
        });
        out.u8(match axis.domain() {
            AxisDomain::Time => 0,
            AxisDomain::Frequency => 1,
            AxisDomain::Parameter => 2,
            AxisDomain::Unknown => 3,
        });
        out.option_u8(axis.unit().map(|unit| match unit {
            AxisUnit::Second => 0,
            AxisUnit::Hertz => 1,
            AxisUnit::Ppm => 2,
            AxisUnit::Tesla => 3,
            AxisUnit::TeslaPerMeter => 4,
        }));
        out.option_u8(axis.quantity().map(|quantity| match quantity {
            AxisQuantity::MagneticFieldGradientStrength => 0,
            AxisQuantity::TimeDelay => 1,
        }));
        out.usize(axis.points());
        match axis.coordinates() {
            AxisCoordinates::Unknown => out.u8(0),
            AxisCoordinates::Uniform { start, step } => {
                out.u8(1);
                out.f64(*start);
                out.f64(*step);
            }
            AxisCoordinates::Explicit(values) => {
                out.u8(2);
                out.usize(values.len());
                for &value in values {
                    out.check()?;
                    out.f64(value);
                }
            }
        }
        match axis.component_basis() {
            ComponentBasis::Scalar => out.u8(0),
            ComponentBasis::Cartesian => out.u8(1),
            ComponentBasis::Encoded(value) => {
                out.u8(2);
                encode_transform(&mut out, value)?;
            }
            ComponentBasis::SharedComplex { axis, conjugated } => {
                out.u8(3);
                out.usize(axis.index());
                out.u8(u8::from(*conjugated));
            }
        }
        out.option_str(axis.nucleus());
        out.option_f64(axis.spectral_width_hz());
        match axis.frequency_evidence() {
            None => out.u8(0),
            Some(value) => {
                out.u8(1);
                out.option_f64(value.observe_frequency_mhz());
                out.option_f64(value.transmitter_offset_hz());
            }
        }
    }
    for axis in &state.axes {
        out.check()?;
        match &axis.group_delay {
            ProcessingDelayState::NotApplicable => out.u8(0),
            ProcessingDelayState::Unknown => out.u8(1),
            ProcessingDelayState::Pending(value) => {
                out.u8(2);
                out.f64(value.delay_points());
                encode_evidence(&mut out, value.evidence())?;
            }
            ProcessingDelayState::Corrected { delay, evidence } => {
                out.u8(3);
                out.f64(*delay);
                match evidence {
                    None => out.u8(0),
                    Some(value) => {
                        out.u8(1);
                        encode_evidence(&mut out, value)?;
                    }
                }
            }
        }
        match &axis.chemical_shift_reference {
            None => out.u8(0),
            Some(value) => {
                out.u8(1);
                out.f64(value.carrier_ppm());
                out.f64(value.reference_frequency_mhz());
                encode_evidence(&mut out, value.evidence())?;
            }
        }
        out.option_u8(axis.latest_fft.map(|sign| match sign {
            crate::processing::contracts::operation::FourierExponentSign::Negative => 0,
            crate::processing::contracts::operation::FourierExponentSign::Positive => 1,
        }));
        out.usize(axis.operation_count);
        out.u8(u8::from(axis.phase_applied));
        out.u8(u8::from(axis.phase_attempt_failed));
    }
    out.usize(state.absolute_origin.len());
    for &value in state.absolute_origin.iter() {
        out.check()?;
        out.usize(value);
    }
    match &state.observation_ordinals {
        None => out.u8(0),
        Some(values) => {
            out.u8(1);
            out.usize(values.len());
            for &value in values.iter() {
                out.check()?;
                out.usize(value);
            }
        }
    }
    // Optional v1 extension: existing unreferenced frozen identities are unchanged.
    if descriptor
        .axes()
        .iter()
        .any(|a| a.spectrum_reference().is_some())
    {
        out.bytes(b"nmr.spectrum-reference.v1\0");
        for axis in descriptor.axes() {
            match axis.spectrum_reference() {
                None => out.u8(0),
                Some(reference) => {
                    out.u8(1);
                    out.f64(reference.reference_frequency_mhz());
                    encode_evidence(&mut out, reference.evidence())?;
                }
            }
        }
    }
    out.finish_checked()
}

pub(crate) fn check_processed_evidence(
    descriptor: &crate::processed::ProcessedDescriptor,
    state: &crate::processing::contracts::state::PlanState,
    recorded: CanonicalDatasetDigests,
    token: Option<&CancellationToken>,
) -> Result<(), crate::internal::ModelError> {
    let descriptor = processed_descriptor_digest(descriptor, state, token)?;
    let mut out = Encoder::new_checked(b"nmr.processed-dataset.v1\0", token);
    out.bytes(descriptor.as_bytes());
    out.bytes(recorded.samples().as_bytes());
    if descriptor != recorded.descriptor() || out.finish_checked()? != recorded.dataset() {
        return Err(crate::internal::ModelError::DigestMismatch);
    }
    Ok(())
}
