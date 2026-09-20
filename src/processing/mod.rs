//! Bounded, history-bearing minimal processing for dense rank-1 and rank-2 data.
//!
//! Explicit deterministic operations provide the supported processing workflow.
//! ACME (`NormalizedAcmeV1`), AsLS (`PositivePeaksV1`), IST and
//! `DensePipeline` automatic policy have a separate, restricted experimental
//! scientific scope, described by the [compatibility policy](crate#experimental-algorithms).
//! Recording a resolved result does not validate inference or promise bitwise
//! reproducibility. Pre-release algorithm/history rules evolve in place;
//! development snapshots have no migration or backward-compatibility guarantee.

pub(crate) mod contracts;
pub(crate) mod engine;
pub(crate) mod kernels;
pub(crate) mod methods;
pub(crate) mod policy;
pub(crate) mod prepare;

pub use crate::processing::contracts::polarity::{ExpectedPolarity, PolarityState};
pub use crate::resource::WorkLedger;
pub use contracts::capability::{GroupDelayCapability, RawCapabilities};
pub use contracts::error::{ProcessingError, ProcessingErrorCode, ProcessingPhase};
pub use contracts::history::{
    ComponentAccumulationOrder, ExecutionEnvironment, ExecutionSegment, HistoryInput,
    ProcessingHistory, ProcessingRecord,
};
pub use contracts::operation::*;
pub use contracts::options::ProcessingOptions;
pub use contracts::prepared_step::PreparedStepView;
pub use contracts::profile::{NormalizedAcmeV1, PositivePeaksV1};
pub use kernels::acme::{PhaseOptimizationError, PhaseSolution};
pub use kernels::asls::AslsError;
pub use kernels::ist::{
    GeneralGridPhaseCovariantGroupRetainedIstV1, IstError, IstInput, IstOptions, IstOutput,
    PhaseCovariantGroupRetainedIstV1,
};
pub use methods::auto_phase::{AutoPhaseRequest, PreparedAutoPhase};
pub use methods::baseline::{BaselineEstimate, PreparedBaselineEstimate};
pub use methods::combine::{LinearCombination, PreparedCombination};
pub use methods::nus::{
    AnalyzedNus, AutoNusSettings, NusNoiseError, NusNoiseReport, NusNoiseSource, NusSettings,
    PreparedNus,
};
pub use methods::phase::{PhaseEstimate, PreparedPhaseEstimate};
pub use policy::{DenseAxisConfig, DensePipeline, DensePipelineError, DensePipelineOptions};
pub use prepare::plan::{PreparedPlan, PreparedRawPlan, ProcessingInput, ProcessingPlan};
