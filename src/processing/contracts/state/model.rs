use crate::acquisition::GroupDelayState;
use crate::processed::ProcessedAxis;
use crate::processed::ProcessedValidationError;
use crate::processing::contracts::operation::*;

pub(crate) enum StateError {
    Cancelled,
    InvalidDatasetState {
        operation: &'static str,
        reason: &'static str,
    },
    MissingCapability {
        capability: &'static str,
        axis: Option<usize>,
    },
    InvalidParameter(&'static str),
    InvalidState {
        axis: usize,
        operation: &'static str,
        reason: &'static str,
    },
    Mapping(&'static str),
    DelayEvidenceMismatch,
    MissingTimeCalibration,
    SizeOverflow,
    AllocationFailure,
    Validation(ProcessedValidationError),
}

impl From<ProcessedValidationError> for StateError {
    fn from(error: ProcessedValidationError) -> Self {
        Self::Validation(error)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProcessingDelayState {
    NotApplicable,
    Unknown,
    Pending(crate::acquisition::PendingGroupDelay),
    Corrected {
        delay: f64,
        evidence: Option<crate::acquisition::NormalizationEvidence>,
    },
}

impl From<&GroupDelayState> for ProcessingDelayState {
    fn from(value: &GroupDelayState) -> Self {
        match value {
            GroupDelayState::NotApplicable => Self::NotApplicable,
            GroupDelayState::Unknown => Self::Unknown,
            GroupDelayState::Pending(value) => Self::Pending(value.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AxisState {
    pub(crate) axis: ProcessedAxis,
    pub(crate) group_delay: ProcessingDelayState,
    pub(crate) chemical_shift_reference: Option<crate::acquisition::ChemicalShiftReference>,
    pub(crate) latest_fft: Option<FourierExponentSign>,
    pub(crate) operation_count: usize,
    pub(crate) phase_applied: bool,
    pub(crate) phase_attempt_failed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlanState {
    pub(crate) axes: Vec<AxisState>,
    pub(crate) observation_ordinals: Option<std::sync::Arc<[usize]>>,
    pub(crate) absolute_origin: std::sync::Arc<[usize]>,
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl AxisState {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &ProcessedAxis,
        &ProcessingDelayState,
        &Option<crate::acquisition::ChemicalShiftReference>,
        &Option<FourierExponentSign>,
        &usize,
        &bool,
        &bool,
    ) {
        (
            &self.axis,
            &self.group_delay,
            &self.chemical_shift_reference,
            &self.latest_fft,
            &self.operation_count,
            &self.phase_applied,
            &self.phase_attempt_failed,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            ProcessedAxis,
            ProcessingDelayState,
            Option<crate::acquisition::ChemicalShiftReference>,
            Option<FourierExponentSign>,
            usize,
            bool,
            bool,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            axis,
            group_delay,
            chemical_shift_reference,
            latest_fft,
            operation_count,
            phase_applied,
            phase_attempt_failed,
        ) = parts;
        let value = Self {
            axis,
            group_delay,
            chemical_shift_reference,
            latest_fft,
            operation_count,
            phase_applied,
            phase_attempt_failed,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl PlanState {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Vec<AxisState>,
        &Option<std::sync::Arc<[usize]>>,
        &std::sync::Arc<[usize]>,
    ) {
        (
            &self.axes,
            &self.observation_ordinals,
            &self.absolute_origin,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Vec<AxisState>,
            Option<std::sync::Arc<[usize]>>,
            std::sync::Arc<[usize]>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (axes, observation_ordinals, absolute_origin) = parts;
        let value = Self {
            axes,
            observation_ordinals,
            absolute_origin,
        };

        Ok(value)
    }
}
