use super::context::ErrorContext;
use super::context::parameter_error;
use super::context::positive;
use super::parameters::ParameterFile;

use crate::acquisition::registry;
use crate::acquisition::{ChemicalShiftReference, GroupDelayState, PendingGroupDelay, RawAxisKind};
use crate::acquisition::{NormalizationEvidence, NormalizationFact};
use crate::raw::{ParameterError, ParameterErrorKind};
use crate::{AcquisitionMetadata, Axis, AxisCoordinates, FrequencyEvidence, ReadError};

#[cfg(test)]
thread_local! {
    pub(super) static AXIS_BUILD_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) struct AxisLayout<'a> {
    pub(super) kind: RawAxisKind,
    pub(super) points: usize,
    pub(super) label: &'a str,
    pub(super) group_delay: Option<super::ResolvedGroupDelay>,
}

/// Conservative construction and retained payload for validated ranks 1..=3.
/// Optional evidence and every indirect transform reserve the largest supported
/// Bruker form. Vec-to-Arc source and destination arrays are both included.
pub(super) fn descriptor_storage_bound(parameters: &[ParameterFile]) -> Result<usize, ReadError> {
    use crate::acquisition::{DerivationStepId, ResolvedComponentTransform};
    use std::mem::size_of;
    let rank = parameters.len();
    if !(1..=3).contains(&rank) {
        return Err(ReadError::SizeOverflow);
    }
    let evidence = |facts: usize| {
        2 * (facts * size_of::<NormalizationFact>() + size_of::<DerivationStepId>())
            + 6 * size_of::<usize>()
    };
    // Axis Vec, three layout arrays, and two copies of each two-byte F1/F2/F3 label.
    let axes = rank * (size_of::<Axis>() + 3 * size_of::<usize>() + 4);
    // One chemical reference per axis, one direct delay, and the layout evidence.
    let references = (rank + 2) * evidence(1);
    // At most four coefficients and four modulation samples, each Vec + Arc.
    let transform = ResolvedComponentTransform::wrapper_allocation_bytes()
        + 16 * size_of::<crate::Complex64>()
        + 6 * size_of::<usize>()
        + evidence(3);
    let mut bytes = axes + references + (rank - 1) * transform;
    for parameter in parameters {
        if let Some(nucleus) = parameter.get("NUC1") {
            // Source text copy, compact text, and canonical/opaque output.
            bytes = nucleus
                .len()
                .checked_mul(2)
                .and_then(|length| length.checked_add(nucleus.len().max(7)))
                .and_then(|length| bytes.checked_add(length))
                .ok_or(ReadError::SizeOverflow)?;
        }
    }
    for text in [
        parameters[0].title(),
        parameters[0].get("SOLVENT"),
        parameters[0].get("PULPROG"),
    ]
    .into_iter()
    .flatten()
    {
        bytes = bytes
            .checked_add(text.len())
            .ok_or(ReadError::SizeOverflow)?;
    }
    Ok(bytes)
}

pub(super) fn normalized_axis(
    parameters: &ParameterFile,
    source: &(impl ErrorContext + ?Sized),
    layout: AxisLayout<'_>,
) -> Result<Axis, ReadError> {
    #[cfg(test)]
    AXIS_BUILD_CALLS.with(|calls| calls.set(calls.get() + 1));
    let direct = matches!(&layout.kind, RawAxisKind::Direct(_));
    let observe_frequency_mhz = match parameters.get("SFO1") {
        Some(_) => parameters.float("SFO1", source)?,
        None => parameters.float("BF1", source)?,
    };
    let observe_frequency_mhz = positive("SFO1 or BF1", observe_frequency_mhz, source)?;
    let spectral_width_hz = match parameters.optional_float("SW_h", source)? {
        Some(value) => positive("SW_h", value, source)?,
        None => {
            let sweep_ppm = positive("SW", parameters.float("SW", source)?, source)?;
            positive("SW * SFO1", sweep_ppm * observe_frequency_mhz, source)?
        }
    };
    let base_frequency_mhz = parameters
        .optional_float("BF1", source)?
        .unwrap_or(observe_frequency_mhz);
    let base_frequency_mhz = positive("BF1", base_frequency_mhz, source)?;
    let reference = FrequencyEvidence::new(
        Some(observe_frequency_mhz),
        parameters.optional_float("O1", source)?,
    )?;
    let chemical_shift_reference = parameters
        .optional_float("O1", source)?
        .map(|offset_hz| {
            ChemicalShiftReference::resolved(
                offset_hz / base_frequency_mhz,
                base_frequency_mhz,
                NormalizationEvidence::from_format(
                    registry::BRUKER_DIRECT_V1,
                    vec![NormalizationFact::ChemicalShiftReference],
                    vec![registry::CHEMICAL_SHIFT_V1],
                )?,
            )
        })
        .transpose()?;
    Ok(Axis::new(
        layout.kind,
        crate::Domain::Time,
        Some(crate::AxisUnit::Second),
        layout.points,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0 / spectral_width_hz,
        },
    )?
    .with_label(Some(layout.label.to_owned()))
    .with_nucleus(parameters.text("NUC1"))?
    .with_spectral_width_hz(Some(spectral_width_hz))?
    .with_frequency_evidence(Some(reference))?
    .with_chemical_shift_reference(chemical_shift_reference)?
    .with_group_delay(match layout.group_delay {
        Some(delay) => GroupDelayState::Pending(PendingGroupDelay::resolved(
            delay.points,
            NormalizationEvidence::from_format(
                delay.rule,
                vec![NormalizationFact::DigitalFilterDelay],
                vec![registry::GROUP_DELAY_V1],
            )?,
        )?),
        None if direct => GroupDelayState::Unknown,
        None => GroupDelayState::NotApplicable,
    })?)
}

pub(super) fn acquisition_metadata(
    parameters: &ParameterFile,
    source: &(impl ErrorContext + ?Sized),
) -> Result<AcquisitionMetadata, ParameterError> {
    let scans = parameters
        .get("NS")
        .map(|raw| raw.parse::<u64>())
        .transpose()
        .map_err(|_| {
            parameter_error(
                source,
                Some("NS"),
                ParameterErrorKind::Invalid,
                format!(
                    "value {:?}: expected a non-negative integer",
                    parameters.get("NS").unwrap_or_default()
                ),
            )
        })?;
    Ok(AcquisitionMetadata {
        title: parameters.title().map(str::to_owned),
        solvent: parameters.text("SOLVENT"),
        temperature_kelvin: parameters.optional_float("TE", source)?,
        scans,
        pulse_program: parameters.text("PULPROG"),
        diffusion: None,
    })
}
