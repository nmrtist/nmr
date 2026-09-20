#[allow(unused_imports)]
use crate::processing::contracts::operation::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for AttemptFailure {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::NoUsableSignal => record(
                    "processing_operation.AttemptFailure.v1.NoUsableSignal",
                    vec![],
                    budget,
                ),
                Self::ObjectiveUndefined => record(
                    "processing_operation.AttemptFailure.v1.ObjectiveUndefined",
                    vec![],
                    budget,
                ),
                Self::OptimizationDidNotConverge => record(
                    "processing_operation.AttemptFailure.v1.OptimizationDidNotConverge",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.AttemptFailure.v1.NoUsableSignal" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::NoUsableSignal)
                }
                "processing_operation.AttemptFailure.v1.ObjectiveUndefined"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::ObjectiveUndefined)
                }
                "processing_operation.AttemptFailure.v1.OptimizationDidNotConverge"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::OptimizationDidNotConverge)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for BaselineProfile {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::PositivePeaksV1(v0) => record(
                    "processing_operation.BaselineProfile.v1.PositivePeaksV1",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.BaselineProfile.v1.PositivePeaksV1" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::PositivePeaksV1(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for BinAggregation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Sum => record("processing_operation.BinAggregation.v1.Sum", vec![], budget),
                Self::Mean => record(
                    "processing_operation.BinAggregation.v1.Mean",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.BinAggregation.v1.Sum" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Sum)
                }
                "processing_operation.BinAggregation.v1.Mean" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Mean)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for DelaySource {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::AxisEvidence => record(
                    "processing_operation.DelaySource.v1.AxisEvidence",
                    vec![],
                    budget,
                ),
                Self::Explicit(v0) => record(
                    "processing_operation.DelaySource.v1.Explicit",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.DelaySource.v1.AxisEvidence" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::AxisEvidence)
                }
                "processing_operation.DelaySource.v1.Explicit" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Explicit(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for DigitalFilterCorrection {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::AcknowledgeZeroDelayV1 => record(
                    "processing_operation.DigitalFilterCorrection.v1.AcknowledgeZeroDelayV1",
                    vec![],
                    budget,
                ),
                Self::FrequencyDomainPhaseRampV1(v0) => record(
                    "processing_operation.DigitalFilterCorrection.v1.FrequencyDomainPhaseRampV1",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::TimeDomainShiftFoldV1 { source, policy } => record(
                    "processing_operation.DigitalFilterCorrection.v1.TimeDomainShiftFoldV1",
                    vec![source.to_wire(budget)?, policy.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.DigitalFilterCorrection.v1.AcknowledgeZeroDelayV1"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::AcknowledgeZeroDelayV1)
                }
                "processing_operation.DigitalFilterCorrection.v1.FrequencyDomainPhaseRampV1"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::FrequencyDomainPhaseRampV1(next(&mut f, budget)?))
                }
                "processing_operation.DigitalFilterCorrection.v1.TimeDomainShiftFoldV1"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::TimeDomainShiftFoldV1 {
                        source: next(&mut f, budget)?,
                        policy: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for FourierExponentSign {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Negative => record(
                    "processing_operation.FourierExponentSign.v1.Negative",
                    vec![],
                    budget,
                ),
                Self::Positive => record(
                    "processing_operation.FourierExponentSign.v1.Positive",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.FourierExponentSign.v1.Negative" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Negative)
                }
                "processing_operation.FourierExponentSign.v1.Positive" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Positive)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for FourierTransform {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_operation.FourierTransform.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_operation.FourierTransform.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for FrequencyFrame {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Hertz => record(
                    "processing_operation.FrequencyFrame.v1.Hertz",
                    vec![],
                    budget,
                ),
                Self::Ppm(v0) => record(
                    "processing_operation.FrequencyFrame.v1.Ppm",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.FrequencyFrame.v1.Hertz" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Hertz)
                }
                "processing_operation.FrequencyFrame.v1.Ppm" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Ppm(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for Normalization {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::MaxPeak => record(
                    "processing_operation.Normalization.v1.MaxPeak",
                    vec![],
                    budget,
                ),
                Self::TotalArea { singleton_width } => record(
                    "processing_operation.Normalization.v1.TotalArea",
                    vec![singleton_width.to_wire(budget)?],
                    budget,
                ),
                Self::Constant(v0) => record(
                    "processing_operation.Normalization.v1.Constant",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.Normalization.v1.MaxPeak" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::MaxPeak)
                }
                "processing_operation.Normalization.v1.TotalArea" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::TotalArea {
                        singleton_width: next(&mut f, budget)?,
                    })
                }
                "processing_operation.Normalization.v1.Constant" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Constant(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for PhaseCorrection {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_operation.PhaseCorrection.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_operation.PhaseCorrection.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for PhaseFailurePolicy {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Fail => record(
                    "processing_operation.PhaseFailurePolicy.v1.Fail",
                    vec![],
                    budget,
                ),
                Self::ContinueUnphasedReal => record(
                    "processing_operation.PhaseFailurePolicy.v1.ContinueUnphasedReal",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.PhaseFailurePolicy.v1.Fail" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Fail)
                }
                "processing_operation.PhaseFailurePolicy.v1.ContinueUnphasedReal"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::ContinueUnphasedReal)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for PhaseMethod {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::AbsorptivePeak => record(
                    "processing_operation.PhaseMethod.v1.AbsorptivePeak",
                    vec![],
                    budget,
                ),
                Self::Entropy => record(
                    "processing_operation.PhaseMethod.v1.Entropy",
                    vec![],
                    budget,
                ),
                Self::NegativeMinimization => record(
                    "processing_operation.PhaseMethod.v1.NegativeMinimization",
                    vec![],
                    budget,
                ),
                Self::PeakRegression => record(
                    "processing_operation.PhaseMethod.v1.PeakRegression",
                    vec![],
                    budget,
                ),
                Self::RobustConsensus => record(
                    "processing_operation.PhaseMethod.v1.RobustConsensus",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.PhaseMethod.v1.AbsorptivePeak" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::AbsorptivePeak)
                }
                "processing_operation.PhaseMethod.v1.Entropy" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Entropy)
                }
                "processing_operation.PhaseMethod.v1.NegativeMinimization" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::NegativeMinimization)
                }
                "processing_operation.PhaseMethod.v1.PeakRegression" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::PeakRegression)
                }
                "processing_operation.PhaseMethod.v1.RobustConsensus" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::RobustConsensus)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessingDiagnostic {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::ContinuedAfterPhaseFailure => record(
                    "processing_operation.ProcessingDiagnostic.v1.ContinuedAfterPhaseFailure",
                    vec![],
                    budget,
                ),
                Self::AmbiguousPolarity => record(
                    "processing_operation.ProcessingDiagnostic.v1.AmbiguousPolarity",
                    vec![],
                    budget,
                ),
                Self::UnknownCoordinateQuality => record(
                    "processing_operation.ProcessingDiagnostic.v1.UnknownCoordinateQuality",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.ProcessingDiagnostic.v1.ContinuedAfterPhaseFailure"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::ContinuedAfterPhaseFailure)
                }
                "processing_operation.ProcessingDiagnostic.v1.AmbiguousPolarity"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::AmbiguousPolarity)
                }
                "processing_operation.ProcessingDiagnostic.v1.UnknownCoordinateQuality"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::UnknownCoordinateQuality)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessingOperation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Spectrum { axis, operation } => record(
                    "processing_operation.ProcessingOperation.v1.Spectrum",
                    vec![axis.to_wire(budget)?, operation.to_wire(budget)?],
                    budget,
                ),
                Self::Window { axis, window } => record(
                    "processing_operation.ProcessingOperation.v1.Window",
                    vec![axis.to_wire(budget)?, window.to_wire(budget)?],
                    budget,
                ),
                Self::ZeroFill { axis, zero_fill } => record(
                    "processing_operation.ProcessingOperation.v1.ZeroFill",
                    vec![axis.to_wire(budget)?, zero_fill.to_wire(budget)?],
                    budget,
                ),
                Self::StandardZeroFill { axis } => record(
                    "processing_operation.ProcessingOperation.v1.StandardZeroFill",
                    vec![axis.to_wire(budget)?],
                    budget,
                ),
                Self::FourierTransform { axis, transform } => record(
                    "processing_operation.ProcessingOperation.v1.FourierTransform",
                    vec![axis.to_wire(budget)?, transform.to_wire(budget)?],
                    budget,
                ),
                Self::DigitalFilterCorrection { axis, correction } => record(
                    "processing_operation.ProcessingOperation.v1.DigitalFilterCorrection",
                    vec![axis.to_wire(budget)?, correction.to_wire(budget)?],
                    budget,
                ),
                Self::PhaseCorrection { axis, correction } => record(
                    "processing_operation.ProcessingOperation.v1.PhaseCorrection",
                    vec![axis.to_wire(budget)?, correction.to_wire(budget)?],
                    budget,
                ),
                Self::BaselineCorrection { axis, profile } => record(
                    "processing_operation.ProcessingOperation.v1.BaselineCorrection",
                    vec![axis.to_wire(budget)?, profile.to_wire(budget)?],
                    budget,
                ),
                Self::ComponentTransform { axis } => record(
                    "processing_operation.ProcessingOperation.v1.ComponentTransform",
                    vec![axis.to_wire(budget)?],
                    budget,
                ),
                Self::Projection {
                    projection,
                    polarity,
                } => record(
                    "processing_operation.ProcessingOperation.v1.Projection",
                    vec![projection.to_wire(budget)?, polarity.to_wire(budget)?],
                    budget,
                ),
                Self::ResolveFrequencyFrame { axis, frame } => record(
                    "processing_operation.ProcessingOperation.v1.ResolveFrequencyFrame",
                    vec![axis.to_wire(budget)?, frame.to_wire(budget)?],
                    budget,
                ),
                Self::ReverseAxis { axis } => record(
                    "processing_operation.ProcessingOperation.v1.ReverseAxis",
                    vec![axis.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.ProcessingOperation.v1.Spectrum" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Spectrum {
                        axis: next(&mut f, budget)?,
                        operation: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.Window" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Window {
                        axis: next(&mut f, budget)?,
                        window: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.ZeroFill" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::ZeroFill {
                        axis: next(&mut f, budget)?,
                        zero_fill: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.StandardZeroFill"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::StandardZeroFill {
                        axis: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.FourierTransform"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::FourierTransform {
                        axis: next(&mut f, budget)?,
                        transform: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.DigitalFilterCorrection"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::DigitalFilterCorrection {
                        axis: next(&mut f, budget)?,
                        correction: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.PhaseCorrection"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::PhaseCorrection {
                        axis: next(&mut f, budget)?,
                        correction: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.BaselineCorrection"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::BaselineCorrection {
                        axis: next(&mut f, budget)?,
                        profile: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.ComponentTransform"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::ComponentTransform {
                        axis: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.Projection" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Projection {
                        projection: next(&mut f, budget)?,
                        polarity: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.ResolveFrequencyFrame"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::ResolveFrequencyFrame {
                        axis: next(&mut f, budget)?,
                        frame: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingOperation.v1.ReverseAxis" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::ReverseAxis {
                        axis: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessingRequest {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::BaselineEstimate { axis, method } => record(
                    "processing_operation.ProcessingRequest.v1.BaselineEstimate",
                    vec![axis.to_wire(budget)?, method.to_wire(budget)?],
                    budget,
                ),
                Self::PhaseMethod { axis, method } => record(
                    "processing_operation.ProcessingRequest.v1.PhaseMethod",
                    vec![axis.to_wire(budget)?, method.to_wire(budget)?],
                    budget,
                ),
                Self::Explicit(v0) => record(
                    "processing_operation.ProcessingRequest.v1.Explicit",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::AutoPhase {
                    axis,
                    profile,
                    polarity,
                    failure_policy,
                } => record(
                    "processing_operation.ProcessingRequest.v1.AutoPhase",
                    vec![
                        axis.to_wire(budget)?,
                        profile.to_wire(budget)?,
                        polarity.to_wire(budget)?,
                        failure_policy.to_wire(budget)?,
                    ],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.ProcessingRequest.v1.BaselineEstimate"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::BaselineEstimate {
                        axis: next(&mut f, budget)?,
                        method: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingRequest.v1.PhaseMethod" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::PhaseMethod {
                        axis: next(&mut f, budget)?,
                        method: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ProcessingRequest.v1.Explicit" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Explicit(next(&mut f, budget)?))
                }
                "processing_operation.ProcessingRequest.v1.AutoPhase" if values.len() == 4 => {
                    let mut f = values.into_iter();
                    Ok(Self::AutoPhase {
                        axis: next(&mut f, budget)?,
                        profile: next(&mut f, budget)?,
                        polarity: next(&mut f, budget)?,
                        failure_policy: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for Projection {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Real => record("processing_operation.Projection.v1.Real", vec![], budget),
                Self::RealAbsorptive => record(
                    "processing_operation.Projection.v1.RealAbsorptive",
                    vec![],
                    budget,
                ),
                Self::RealSigned => record(
                    "processing_operation.Projection.v1.RealSigned",
                    vec![],
                    budget,
                ),
                Self::Magnitude => record(
                    "processing_operation.Projection.v1.Magnitude",
                    vec![],
                    budget,
                ),
                Self::UnphasedReal => record(
                    "processing_operation.Projection.v1.UnphasedReal",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.Projection.v1.Real" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Real)
                }
                "processing_operation.Projection.v1.RealAbsorptive" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::RealAbsorptive)
                }
                "processing_operation.Projection.v1.RealSigned" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::RealSigned)
                }
                "processing_operation.Projection.v1.Magnitude" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Magnitude)
                }
                "processing_operation.Projection.v1.UnphasedReal" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::UnphasedReal)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for RealBaseline {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Offset => record(
                    "processing_operation.RealBaseline.v1.Offset",
                    vec![],
                    budget,
                ),
                Self::Polynomial { order } => record(
                    "processing_operation.RealBaseline.v1.Polynomial",
                    vec![order.to_wire(budget)?],
                    budget,
                ),
                Self::Asls {
                    lambda,
                    asymmetry,
                    iterations,
                } => record(
                    "processing_operation.RealBaseline.v1.Asls",
                    vec![
                        lambda.to_wire(budget)?,
                        asymmetry.to_wire(budget)?,
                        iterations.to_wire(budget)?,
                    ],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.RealBaseline.v1.Offset" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Offset)
                }
                "processing_operation.RealBaseline.v1.Polynomial" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Polynomial {
                        order: next(&mut f, budget)?,
                    })
                }
                "processing_operation.RealBaseline.v1.Asls" if values.len() == 3 => {
                    let mut f = values.into_iter();
                    Ok(Self::Asls {
                        lambda: next(&mut f, budget)?,
                        asymmetry: next(&mut f, budget)?,
                        iterations: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ReferenceSource {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::AxisEvidence => record(
                    "processing_operation.ReferenceSource.v1.AxisEvidence",
                    vec![],
                    budget,
                ),
                Self::Explicit(v0) => record(
                    "processing_operation.ReferenceSource.v1.Explicit",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.ReferenceSource.v1.AxisEvidence" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::AxisEvidence)
                }
                "processing_operation.ReferenceSource.v1.Explicit" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Explicit(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ResolvedOperation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::EstimatedBaseline {
                    values,
                    coefficients,
                } => record(
                    "processing_operation.ResolvedOperation.v1.EstimatedBaseline",
                    vec![values.to_wire(budget)?, coefficients.to_wire(budget)?],
                    budget,
                ),
                Self::AutomaticPhase {
                    correction,
                    objective,
                    evaluations,
                } => record(
                    "processing_operation.ResolvedOperation.v1.AutomaticPhase",
                    vec![
                        correction.to_wire(budget)?,
                        objective.to_wire(budget)?,
                        evaluations.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::Spectrum(v0) => record(
                    "processing_operation.ResolvedOperation.v1.Spectrum",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::AcknowledgeZeroDelayV1 { evidence } => record(
                    "processing_operation.ResolvedOperation.v1.AcknowledgeZeroDelayV1",
                    vec![evidence.to_wire(budget)?],
                    budget,
                ),
                Self::Window(v0) => record(
                    "processing_operation.ResolvedOperation.v1.Window",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::ZeroFill { target_points } => record(
                    "processing_operation.ResolvedOperation.v1.ZeroFill",
                    vec![target_points.to_wire(budget)?],
                    budget,
                ),
                Self::FourierTransform(v0) => record(
                    "processing_operation.ResolvedOperation.v1.FourierTransform",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::FrequencyDomainPhaseRampV1 { delay, sign } => record(
                    "processing_operation.ResolvedOperation.v1.FrequencyDomainPhaseRampV1",
                    vec![delay.to_wire(budget)?, sign.to_wire(budget)?],
                    budget,
                ),
                Self::TimeDomainShiftFoldV1 {
                    applied_delay,
                    skip,
                    fold,
                    residual,
                } => record(
                    "processing_operation.ResolvedOperation.v1.TimeDomainShiftFoldV1",
                    vec![
                        applied_delay.to_wire(budget)?,
                        skip.to_wire(budget)?,
                        fold.to_wire(budget)?,
                        residual.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::PhaseCorrection(v0) => record(
                    "processing_operation.ResolvedOperation.v1.PhaseCorrection",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::ComponentTransform {
                    transform,
                    observation_ordinals,
                    grid_origin,
                } => record(
                    "processing_operation.ResolvedOperation.v1.ComponentTransform",
                    vec![
                        transform.to_wire(budget)?,
                        observation_ordinals.to_wire(budget)?,
                        grid_origin.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::Projection {
                    projection,
                    polarity,
                } => record(
                    "processing_operation.ResolvedOperation.v1.Projection",
                    vec![projection.to_wire(budget)?, polarity.to_wire(budget)?],
                    budget,
                ),
                Self::ResolveFrequencyFrame { frame, reference } => record(
                    "processing_operation.ResolvedOperation.v1.ResolveFrequencyFrame",
                    vec![frame.to_wire(budget)?, reference.to_wire(budget)?],
                    budget,
                ),
                Self::ReverseAxis => record(
                    "processing_operation.ResolvedOperation.v1.ReverseAxis",
                    vec![],
                    budget,
                ),
                Self::BaselineCorrection(v0) => record(
                    "processing_operation.ResolvedOperation.v1.BaselineCorrection",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.ResolvedOperation.v1.EstimatedBaseline"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::EstimatedBaseline {
                        values: next(&mut f, budget)?,
                        coefficients: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.AutomaticPhase" if values.len() == 3 => {
                    let mut f = values.into_iter();
                    Ok(Self::AutomaticPhase {
                        correction: next(&mut f, budget)?,
                        objective: next(&mut f, budget)?,
                        evaluations: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.Spectrum" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Spectrum(next(&mut f, budget)?))
                }
                "processing_operation.ResolvedOperation.v1.AcknowledgeZeroDelayV1"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::AcknowledgeZeroDelayV1 {
                        evidence: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.Window" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Window(next(&mut f, budget)?))
                }
                "processing_operation.ResolvedOperation.v1.ZeroFill" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::ZeroFill {
                        target_points: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.FourierTransform"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::FourierTransform(next(&mut f, budget)?))
                }
                "processing_operation.ResolvedOperation.v1.FrequencyDomainPhaseRampV1"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::FrequencyDomainPhaseRampV1 {
                        delay: next(&mut f, budget)?,
                        sign: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.TimeDomainShiftFoldV1"
                    if values.len() == 4 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::TimeDomainShiftFoldV1 {
                        applied_delay: next(&mut f, budget)?,
                        skip: next(&mut f, budget)?,
                        fold: next(&mut f, budget)?,
                        residual: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.PhaseCorrection"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::PhaseCorrection(next(&mut f, budget)?))
                }
                "processing_operation.ResolvedOperation.v1.ComponentTransform"
                    if values.len() == 3 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::ComponentTransform {
                        transform: next(&mut f, budget)?,
                        observation_ordinals: next(&mut f, budget)?,
                        grid_origin: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.Projection" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Projection {
                        projection: next(&mut f, budget)?,
                        polarity: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.ResolveFrequencyFrame"
                    if values.len() == 2 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::ResolveFrequencyFrame {
                        frame: next(&mut f, budget)?,
                        reference: next(&mut f, budget)?,
                    })
                }
                "processing_operation.ResolvedOperation.v1.ReverseAxis" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::ReverseAxis)
                }
                "processing_operation.ResolvedOperation.v1.BaselineCorrection"
                    if values.len() == 1 =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::BaselineCorrection(next(&mut f, budget)?))
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for SpectrumOperation {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::RetainRange { start, end } => record(
                    "processing_operation.SpectrumOperation.v1.RetainRange",
                    vec![start.to_wire(budget)?, end.to_wire(budget)?],
                    budget,
                ),
                Self::Reference { delta_ppm } => record(
                    "processing_operation.SpectrumOperation.v1.Reference",
                    vec![delta_ppm.to_wire(budget)?],
                    budget,
                ),
                Self::Reverse => record(
                    "processing_operation.SpectrumOperation.v1.Reverse",
                    vec![],
                    budget,
                ),
                Self::Invert => record(
                    "processing_operation.SpectrumOperation.v1.Invert",
                    vec![],
                    budget,
                ),
                Self::Affine { scale, real_offset } => record(
                    "processing_operation.SpectrumOperation.v1.Affine",
                    vec![scale.to_wire(budget)?, real_offset.to_wire(budget)?],
                    budget,
                ),
                Self::MovingAverage { window } => record(
                    "processing_operation.SpectrumOperation.v1.MovingAverage",
                    vec![window.to_wire(budget)?],
                    budget,
                ),
                Self::SavitzkyGolay { window, order } => record(
                    "processing_operation.SpectrumOperation.v1.SavitzkyGolay",
                    vec![window.to_wire(budget)?, order.to_wire(budget)?],
                    budget,
                ),
                Self::Baseline(v0) => record(
                    "processing_operation.SpectrumOperation.v1.Baseline",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Normalize(v0) => record(
                    "processing_operation.SpectrumOperation.v1.Normalize",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Bin { width, aggregation } => record(
                    "processing_operation.SpectrumOperation.v1.Bin",
                    vec![width.to_wire(budget)?, aggregation.to_wire(budget)?],
                    budget,
                ),
                Self::Magnitude => record(
                    "processing_operation.SpectrumOperation.v1.Magnitude",
                    vec![],
                    budget,
                ),
                Self::Slice { index, component } => record(
                    "processing_operation.SpectrumOperation.v1.Slice",
                    vec![index.to_wire(budget)?, component.to_wire(budget)?],
                    budget,
                ),
                Self::Sum { component } => record(
                    "processing_operation.SpectrumOperation.v1.Sum",
                    vec![component.to_wire(budget)?],
                    budget,
                ),
                Self::Skyline { component } => record(
                    "processing_operation.SpectrumOperation.v1.Skyline",
                    vec![component.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.SpectrumOperation.v1.RetainRange" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::RetainRange {
                        start: next(&mut f, budget)?,
                        end: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.Reference" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Reference {
                        delta_ppm: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.Reverse" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Reverse)
                }
                "processing_operation.SpectrumOperation.v1.Invert" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Invert)
                }
                "processing_operation.SpectrumOperation.v1.Affine" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Affine {
                        scale: next(&mut f, budget)?,
                        real_offset: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.MovingAverage" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::MovingAverage {
                        window: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.SavitzkyGolay" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::SavitzkyGolay {
                        window: next(&mut f, budget)?,
                        order: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.Baseline" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Baseline(next(&mut f, budget)?))
                }
                "processing_operation.SpectrumOperation.v1.Normalize" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Normalize(next(&mut f, budget)?))
                }
                "processing_operation.SpectrumOperation.v1.Bin" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Bin {
                        width: next(&mut f, budget)?,
                        aggregation: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.Magnitude" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Magnitude)
                }
                "processing_operation.SpectrumOperation.v1.Slice" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Slice {
                        index: next(&mut f, budget)?,
                        component: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.Sum" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Sum {
                        component: next(&mut f, budget)?,
                    })
                }
                "processing_operation.SpectrumOperation.v1.Skyline" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Skyline {
                        component: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for TimeDomainResidualPolicy {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::CorrectFully => record(
                    "processing_operation.TimeDomainResidualPolicy.v1.CorrectFully",
                    vec![],
                    budget,
                ),
                Self::IntegerOnlyRetainResidual => record(
                    "processing_operation.TimeDomainResidualPolicy.v1.IntegerOnlyRetainResidual",
                    vec![],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.TimeDomainResidualPolicy.v1.CorrectFully"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::CorrectFully)
                }
                "processing_operation.TimeDomainResidualPolicy.v1.IntegerOnlyRetainResidual"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::IntegerOnlyRetainResidual)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for Window {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::SineBell {
                    offset,
                    end,
                    power,
                    first_point_scale,
                } => record(
                    "processing_operation.Window.v1.SineBell",
                    vec![
                        offset.to_wire(budget)?,
                        end.to_wire(budget)?,
                        power.to_wire(budget)?,
                        first_point_scale.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::LorentzToGauss { lb_hz, gb_hz } => record(
                    "processing_operation.Window.v1.LorentzToGauss",
                    vec![lb_hz.to_wire(budget)?, gb_hz.to_wire(budget)?],
                    budget,
                ),
                Self::Exponential { lb_hz } => record(
                    "processing_operation.Window.v1.Exponential",
                    vec![lb_hz.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_operation.Window.v1.SineBell" if values.len() == 4 => {
                    let mut f = values.into_iter();
                    Ok(Self::SineBell {
                        offset: next(&mut f, budget)?,
                        end: next(&mut f, budget)?,
                        power: next(&mut f, budget)?,
                        first_point_scale: next(&mut f, budget)?,
                    })
                }
                "processing_operation.Window.v1.LorentzToGauss" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::LorentzToGauss {
                        lb_hz: next(&mut f, budget)?,
                        gb_hz: next(&mut f, budget)?,
                    })
                }
                "processing_operation.Window.v1.Exponential" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Exponential {
                        lb_hz: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ZeroFill {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_operation.ZeroFill.v1",
                vec![(*self.model_parts().0).to_wire(budget)?],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_operation.ZeroFill.v1", 1)?;
            Self::from_model_parts((next(&mut f, budget)?,)).map_err(SnapshotError::model)
        }
    }
};
