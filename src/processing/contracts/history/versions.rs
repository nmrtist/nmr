use crate::processing::contracts::operation::ProcessingOperation;
use crate::processing::contracts::operation::ProcessingRequest;

pub(crate) fn algorithm_version(request: &ProcessingRequest) -> &'static str {
    match request {
        ProcessingRequest::PhaseMethod { method, .. } => method.algorithm_version(),
        ProcessingRequest::BaselineEstimate { method, .. } => {
            crate::processing::SpectrumOperation::Baseline(*method).algorithm_version()
        }
        ProcessingRequest::AutoPhase { .. } => "normalized-acme.lorentzian-guard.v1",
        ProcessingRequest::Explicit(operation) => explicit_algorithm_version(operation),
    }
}

pub(crate) fn explicit_algorithm_version(operation: &ProcessingOperation) -> &'static str {
    match operation {
        ProcessingOperation::Spectrum { operation, .. } => operation.algorithm_version(),
        ProcessingOperation::Window { .. } => "window.v1",
        ProcessingOperation::ZeroFill { .. } | ProcessingOperation::StandardZeroFill { .. } => {
            "zero-fill.v1"
        }
        ProcessingOperation::FourierTransform { .. } => "centered-fft.v1",
        ProcessingOperation::DigitalFilterCorrection {
            correction: crate::processing::contracts::operation::DigitalFilterCorrection::AcknowledgeZeroDelayV1,
            ..
        } => "acknowledge-zero-delay.v1",
        ProcessingOperation::DigitalFilterCorrection {
            correction:
                crate::processing::contracts::operation::DigitalFilterCorrection::FrequencyDomainPhaseRampV1(_),
            ..
        } => "frequency-domain-phase-ramp.v1",
        ProcessingOperation::DigitalFilterCorrection {
            correction:
                crate::processing::contracts::operation::DigitalFilterCorrection::TimeDomainShiftFoldV1 { .. },
            ..
        } => "time-domain-shift-fold.v1",
        ProcessingOperation::PhaseCorrection { .. } => "phase-correction.v1",
        ProcessingOperation::BaselineCorrection { .. } => "asls.v1",
        ProcessingOperation::ComponentTransform { .. } => "linear-component-transform.v1",
        ProcessingOperation::Projection { .. } => "projection.v1",
        ProcessingOperation::ResolveFrequencyFrame { .. } => "frequency-frame.v1",
        ProcessingOperation::ReverseAxis { .. } => "reverse-axis.v1",
    }
}
