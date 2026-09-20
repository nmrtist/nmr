use crate::acquisition::DirectSamples;
use crate::acquisition::IndirectComponents;
use crate::acquisition::RawAxisKind;
use crate::processed::ComponentBasis;
use crate::processed::ProcessedAxis;
use crate::processed::ProcessedDescriptor;

use super::{AxisState, PlanState, ProcessingDelayState, StateError, validate_auto_phase_axis};

#[cfg(test)]
thread_local! {
    pub(crate) static STATE_CONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl PlanState {
    pub(crate) fn from_descriptor(descriptor: &ProcessedDescriptor) -> Self {
        #[cfg(test)]
        STATE_CONSTRUCTIONS.with(|count| count.set(count.get() + 1));
        Self {
            axes: descriptor
                .axes()
                .iter()
                .cloned()
                .map(|axis| AxisState {
                    group_delay: if axis.role().is_signal() {
                        ProcessingDelayState::Unknown
                    } else {
                        ProcessingDelayState::NotApplicable
                    },
                    axis,
                    chemical_shift_reference: None,
                    latest_fft: None,
                    operation_count: 0,
                    phase_applied: false,
                    phase_attempt_failed: false,
                })
                .collect(),
            observation_ordinals: None,
            absolute_origin: vec![0; descriptor.axes().len()].into(),
        }
    }

    pub(crate) fn from_raw(
        descriptor: &ProcessedDescriptor,
        raw_axes: &[crate::raw::RawAxis],
        sampling: Option<&crate::raw::SamplingSchedule>,
        absolute_origin: &[usize],
    ) -> Result<Self, StateError> {
        Self::from_raw_with_cancellation(descriptor, raw_axes, sampling, absolute_origin, None)
    }

    pub(crate) fn from_raw_with_cancellation(
        descriptor: &ProcessedDescriptor,
        raw_axes: &[crate::raw::RawAxis],
        sampling: Option<&crate::raw::SamplingSchedule>,
        absolute_origin: &[usize],
        token: Option<&crate::CancellationToken>,
    ) -> Result<Self, StateError> {
        let check = || {
            if token.is_some_and(crate::CancellationToken::is_cancelled) {
                Err(StateError::Cancelled)
            } else {
                Ok(())
            }
        };
        check()?;
        if descriptor.axes().len() != raw_axes.len() || raw_axes.len() != absolute_origin.len() {
            return Err(StateError::Mapping("raw and processed axis ranks disagree"));
        }
        #[cfg(test)]
        STATE_CONSTRUCTIONS.with(|count| count.set(count.get() + 1));
        let observation_ordinals = sampling
            .map(|schedule| -> Result<_, StateError> {
                let points = raw_axes.first().map_or(0, crate::raw::RawAxis::points);
                check()?;
                let mut mapping = vec![usize::MAX; points];
                check()?;
                for (ordinal, coordinate) in schedule.coordinates().iter().enumerate() {
                    if ordinal % 4096 == 0 {
                        check()?;
                    }
                    if let Some(&point) = coordinate.as_slice().first() {
                        if point < mapping.len() {
                            mapping[point] = ordinal;
                        }
                    }
                }
                check()?;
                Ok(std::sync::Arc::from(mapping))
            })
            .transpose()?;
        Ok(Self {
            axes: descriptor
                .axes()
                .iter()
                .cloned()
                .zip(raw_axes)
                .map(|(axis, raw)| AxisState {
                    axis,
                    group_delay: raw.group_delay().into(),
                    chemical_shift_reference: raw.chemical_shift_reference().cloned(),
                    latest_fft: None,
                    operation_count: 0,
                    phase_applied: false,
                    phase_attempt_failed: false,
                })
                .collect(),
            observation_ordinals,
            absolute_origin: absolute_origin.into(),
        })
    }

    pub(crate) fn descriptor(&self) -> Result<ProcessedDescriptor, StateError> {
        ProcessedDescriptor::new(self.axes.iter().map(|state| state.axis.clone()).collect())
            .map_err(Into::into)
    }

    pub(crate) fn record_phase_failure(&mut self, axis: usize) -> Result<(), StateError> {
        validate_auto_phase_axis(self, axis)?;
        self.axes[axis].phase_attempt_failed = true;
        Ok(())
    }
}
pub(crate) fn expand_raw_descriptor(
    raw_axes: &[crate::raw::RawAxis],
) -> Result<ProcessedDescriptor, StateError> {
    if !(1..=2).contains(&raw_axes.len()) {
        return Err(StateError::Mapping(
            "only rank-1 and rank-2 raw data is supported",
        ));
    }
    let mut axes = Vec::new();
    axes.try_reserve_exact(raw_axes.len())
        .map_err(|_| StateError::AllocationFailure)?;
    for raw in raw_axes {
        let basis = match raw.kind() {
            RawAxisKind::Direct(DirectSamples::Real) | RawAxisKind::Parameter => {
                ComponentBasis::Scalar
            }
            RawAxisKind::Direct(DirectSamples::Complex) => ComponentBasis::Cartesian,
            RawAxisKind::Indirect(IndirectComponents::Scalar) => ComponentBasis::Scalar,
            RawAxisKind::Indirect(IndirectComponents::SharedComplex { conjugated, .. }) => {
                ComponentBasis::SharedComplex {
                    axis: crate::AxisIndex::new(raw_axes.len() - 1),
                    conjugated: *conjugated,
                }
            }
            RawAxisKind::Indirect(IndirectComponents::Cartesian(_)) => ComponentBasis::Cartesian,
            RawAxisKind::Indirect(IndirectComponents::Encoded(transform)) => {
                ComponentBasis::Encoded(transform.clone())
            }
        };
        let axis = ProcessedAxis::from_raw_axis(raw, basis)?;
        axes.push(axis);
    }
    ProcessedDescriptor::new(axes).map_err(Into::into)
}
