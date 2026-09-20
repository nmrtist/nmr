//! Requested and resolved processing operation values.

mod parameters;
pub use parameters::{
    DelaySource, DigitalFilterCorrection, FourierExponentSign, FourierTransform, PhaseCorrection,
    TimeDomainResidualPolicy, Window, ZeroFill,
};
mod request;
pub use request::{OperationTarget, ProcessingOperation, ProcessingRequest};
mod resolution;
pub use resolution::{
    AttemptFailure, BaselineProfile, DirectDelayMode, FrequencyFrame, PhaseFailurePolicy,
    ProcessingDiagnostic, Projection, ReferenceSource, ResolvedOperation,
};
mod spectrum;
pub use spectrum::{BinAggregation, Normalization, PhaseMethod, RealBaseline, SpectrumOperation};
