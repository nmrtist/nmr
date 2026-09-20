//! Immutable processing records and state/evidence replay inputs.
//! Recorded processing facts, execution segments and pure history validation.

mod environment;
mod model;
mod records;
mod versions;

pub use environment::{ExecutionEnvironment, ExecutionSegment};
#[cfg(test)]
pub(crate) use model::validation::HISTORY_VALIDATIONS;
pub(crate) use model::validation::{replay_record, validate_derived_history};
pub use model::{HistoryInput, ProcessingHistory};
pub use records::{ComponentAccumulationOrder, ProcessingRecord};
pub(crate) use versions::{algorithm_version, explicit_algorithm_version};
