//! Outer fixed-policy recipes and plot convenience; execution uses explicit plans.

use crate::acquisition::{GroupDelayState, IndirectComponents, RawAxisKind};
use crate::axis::AxisRole;
use crate::plot::{PlotData, PlotError};
use crate::processed::ProcessedDataset;
use crate::processing::contracts::operation::*;
use crate::processing::contracts::polarity::{ExpectedPolarity, PolarityState};
use crate::processing::contracts::profile::PositivePeaksV1;
use crate::processing::{ProcessingError, ProcessingOptions, ProcessingPlan};
use crate::raw::RawDataset;
use thiserror::Error;

impl FrequencyFrame {
    fn is_ppm(&self) -> bool {
        matches!(self, Self::Ppm(_))
    }
}

/// Checked configuration for the fixed dense processing pipeline.
///
/// Automatic choices remain experimental and require caller assessment for the
/// target data. This ordinary public API follows the crate's
/// [compatibility policy](crate#experimental-algorithms).
#[derive(Clone, Debug, PartialEq)]
pub struct DensePipeline {
    direct_delay: DirectDelayMode,
    phases: Vec<Option<PhaseCorrection>>,
    projection: Projection,
    expected_polarity: ExpectedPolarity,
    frequency_frames: Vec<Option<FrequencyFrame>>,
    descending_ppm: bool,
    baseline: Option<PositivePeaksV1>,
}

/// Named configuration for one current descriptor axis.
#[derive(Clone, Debug, PartialEq)]
pub struct DenseAxisConfig {
    /// Current descriptor position.
    pub axis: crate::AxisIndex,
    /// Optional explicit phase rotation.
    pub phase: Option<PhaseCorrection>,
    /// Optional calibrated frequency frame.
    pub frequency_frame: Option<FrequencyFrame>,
}

/// Global choices for the fixed dense processing recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct DensePipelineOptions {
    /// Explicit direct-axis delay policy.
    pub direct_delay: DirectDelayMode,
    /// Final scalar projection.
    pub projection: Projection,
    /// Caller-declared peak polarity.
    pub expected_polarity: ExpectedPolarity,
    /// Reverse ppm axes when necessary to obtain descending coordinates.
    pub descending_ppm: bool,
}

impl DensePipeline {
    /// Creates a fixed-order dense pipeline configuration.
    pub fn new(
        axes: Vec<DenseAxisConfig>,
        options: DensePipelineOptions,
    ) -> Result<Self, ProcessingError> {
        if axes.is_empty() {
            return Err(ProcessingError::InvalidParameter(
                "dense pipeline axis configuration",
            ));
        }
        let mut phases = vec![None; axes.len()];
        let mut frequency_frames = vec![None; axes.len()];
        let mut seen = vec![false; axes.len()];
        for config in axes {
            let index = config.axis.index();
            if index >= seen.len() || seen[index] {
                return Err(ProcessingError::InvalidParameter(
                    "duplicate, missing or out-of-range dense pipeline axis",
                ));
            }
            seen[index] = true;
            phases[index] = config.phase;
            frequency_frames[index] = config.frequency_frame;
        }
        let DensePipelineOptions {
            direct_delay,
            projection,
            expected_polarity,
            descending_ppm,
        } = options;
        if projection == Projection::UnphasedReal {
            return Err(ProcessingError::InvalidParameter(
                "UnphasedReal requires an explicit phase-failure policy",
            ));
        }
        Ok(Self {
            direct_delay,
            phases,
            projection,
            expected_polarity,
            frequency_frames,
            descending_ppm,
            baseline: None,
        })
    }

    /// Enables the fixed positive-peak AsLS profile after scalar projection.
    pub fn with_baseline(mut self, profile: PositivePeaksV1) -> Self {
        self.baseline = Some(profile);
        self
    }

    /// Builds the fixed policy's explicit recipe without executing numerical work.
    /// Inspect it through [`ProcessingPlan::operations`] or resolve it with
    /// [`ProcessingPlan::preflight`] before execution.
    pub fn plan(&self, raw: &RawDataset) -> Result<ProcessingPlan, ProcessingError> {
        let axes = raw.descriptor().axes();
        if axes.len() != self.phases.len() || !(1..=2).contains(&axes.len()) {
            return Err(ProcessingError::Mapping(
                "dense pipeline axis configuration does not match raw rank",
            ));
        }
        if raw.data().is_sparse() {
            return Err(ProcessingError::Mapping(
                "dense pipeline requires complete dense input",
            ));
        }
        let direct = axes.len() - 1;
        let mut operations = Vec::new();
        if let DirectDelayMode::TimeDomainShiftFoldV1(source, policy) = self.direct_delay {
            operations.push(ProcessingOperation::DigitalFilterCorrection {
                axis: direct,
                correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 { source, policy },
            });
        }
        operations.push(ProcessingOperation::Window {
            axis: direct,
            window: Window::exponential(1.0)?,
        });
        operations.push(ProcessingOperation::StandardZeroFill { axis: direct });
        operations.push(ProcessingOperation::FourierTransform {
            axis: direct,
            transform: FourierTransform::default(),
        });
        let phase_ramp = match self.direct_delay {
            DirectDelayMode::Automatic => match axes[direct].group_delay() {
                GroupDelayState::Pending(delay) if delay.delay_points() != 0.0 => {
                    Some(DelaySource::AxisEvidence)
                }
                GroupDelayState::Pending(_) => {
                    operations.push(ProcessingOperation::DigitalFilterCorrection {
                        axis: direct,
                        correction: DigitalFilterCorrection::AcknowledgeZeroDelayV1,
                    });
                    None
                }
                GroupDelayState::NotApplicable => None,
                GroupDelayState::Unknown => {
                    return Err(ProcessingError::MissingCapability {
                        capability: "direct group-delay evidence",
                        axis: Some(direct),
                    });
                }
            },
            DirectDelayMode::FrequencyDomainPhaseRampV1(source) => Some(source),
            _ => None,
        };
        if let Some(source) = phase_ramp {
            operations.push(ProcessingOperation::DigitalFilterCorrection {
                axis: direct,
                correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(source),
            });
        }

        if axes.len() == 2
            && matches!(
                axes[0].kind(),
                RawAxisKind::Indirect(IndirectComponents::Encoded(_))
            )
        {
            operations.push(ProcessingOperation::ComponentTransform { axis: 0 });
        }
        if let Some(correction) = self.phases[direct] {
            operations.push(ProcessingOperation::PhaseCorrection {
                axis: direct,
                correction,
            });
        }
        if axes.len() == 2 && axes[0].role().is_signal() {
            operations.push(ProcessingOperation::Window {
                axis: 0,
                window: Window::exponential(1.0)?,
            });
            operations.push(ProcessingOperation::StandardZeroFill { axis: 0 });
            operations.push(ProcessingOperation::FourierTransform {
                axis: 0,
                transform: FourierTransform::default(),
            });
            if let Some(correction) = self.phases[0] {
                operations.push(ProcessingOperation::PhaseCorrection {
                    axis: 0,
                    correction,
                });
            }
        } else if axes.len() == 2 && self.phases[0].is_some() {
            return Err(ProcessingError::InvalidParameter(
                "phase correction on a parameter axis",
            ));
        }
        operations.push(ProcessingOperation::Projection {
            projection: self.projection,
            polarity: PolarityState::from_request(self.expected_polarity),
        });
        if let Some(profile) = self.baseline {
            operations.push(ProcessingOperation::BaselineCorrection {
                axis: direct,
                profile: profile.into(),
            });
        }
        for (axis, frame) in self.frequency_frames.iter().enumerate() {
            if let Some(frame) = frame {
                if axes[axis].role() == AxisRole::ArrayParameter {
                    return Err(ProcessingError::InvalidParameter(
                        "frequency frame on a parameter axis",
                    ));
                }
                if *frame != FrequencyFrame::Hertz {
                    operations.push(ProcessingOperation::ResolveFrequencyFrame {
                        axis,
                        frame: frame.clone(),
                    });
                }
                if self.descending_ppm && frame.is_ppm() {
                    operations.push(ProcessingOperation::ReverseAxis { axis });
                }
            } else if axes[axis].role().is_signal() {
                return Err(ProcessingError::InvalidParameter(
                    "missing signal frequency frame",
                ));
            }
        }
        ProcessingPlan::new(operations)
    }

    /// Runs the fixed pipeline with default limits on a low-level raw dataset.
    pub fn process_dataset(&self, raw: &RawDataset) -> Result<ProcessedDataset, ProcessingError> {
        self.process_dataset_with_options(raw, ProcessingOptions::default())
    }

    /// Runs the inspectable recipe with explicit per-call resource limits.
    pub fn process_dataset_with_options(
        &self,
        raw: &RawDataset,
        options: ProcessingOptions,
    ) -> Result<ProcessedDataset, ProcessingError> {
        self.plan(raw)?.apply_raw_with_options(raw, options)
    }

    /// Runs the inspectable recipe while retaining aggregate reading context.
    pub fn apply(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
    ) -> Result<crate::Dataset, ProcessingError> {
        self.apply_with_context(input, options, &mut crate::ExecutionContext::default())
    }
    /// Runs the fixed recipe using shared execution control.
    pub fn apply_with_context(
        &self,
        input: &crate::Dataset,
        options: ProcessingOptions,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<crate::Dataset, ProcessingError> {
        control.check_cancelled()?;
        let raw = input.as_raw().ok_or(ProcessingError::Mapping(
            "dense pipeline requires raw input",
        ))?;
        self.plan(raw)?.apply_with_context(input, options, control)
    }

    /// Runs the fixed pipeline and creates plot-ready scalar data.
    pub fn process(&self, raw: &RawDataset) -> Result<PlotData, DensePipelineError> {
        let processed = self.process_dataset(raw)?;
        PlotData::from_processed(&processed).map_err(Into::into)
    }
}

/// Failure from the fixed dense pipeline or final plot conversion.
#[non_exhaustive]
#[derive(Clone, Debug, Error, PartialEq)]
pub enum DensePipelineError {
    /// Processing failed.
    #[error(transparent)]
    Processing(#[from] ProcessingError),
    /// Plot conversion failed.
    #[error(transparent)]
    Plot(#[from] PlotError),
}
