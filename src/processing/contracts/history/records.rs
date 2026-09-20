use crate::processed::model::ProcessedDescriptor;
use crate::processing::contracts::operation::AttemptFailure;
use crate::processing::contracts::operation::OperationTarget;
use crate::processing::contracts::operation::ProcessingDiagnostic;
use crate::processing::contracts::operation::ProcessingOperation;
use crate::processing::contracts::operation::ProcessingRequest;
use crate::processing::contracts::operation::ResolvedOperation;

use super::algorithm_version;

/// One immutable processing-history record.
///
/// Records use the current algorithm identifiers and checked state contracts.
/// During pre-release development these contracts evolve in place; development
/// snapshots have no backward-compatibility or migration guarantee.
/// Record variants are non-exhaustive: use accessors or include `..` in matches.
///
/// ```compile_fail,E0638
/// use nmr::processing::ProcessingRecord;
/// fn inspect(record: &ProcessingRecord) {
///     if let ProcessingRecord::Attempted { requested, failure, diagnostics } = record {}
/// }
/// ```
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessingRecord {
    /// An operation was attempted but did not modify samples or the descriptor.
    #[non_exhaustive]
    Attempted {
        /// Operation exactly as requested.
        requested: ProcessingRequest,
        /// Typed recoverable failure.
        failure: AttemptFailure,
        /// Diagnostics produced by this attempt.
        diagnostics: Vec<ProcessingDiagnostic>,
    },
    /// An operation was applied, including a recorded identity permutation or state-only change.
    #[non_exhaustive]
    Applied {
        /// Operation exactly as requested.
        requested: ProcessingRequest,
        /// Parameters actually resolved and applied.
        resolved: Box<ResolvedOperation>,
        /// Descriptor immediately before the operation.
        input_descriptor: ProcessedDescriptor,
        /// Descriptor immediately after the operation.
        output_descriptor: ProcessedDescriptor,
        /// Frozen implementation algorithm identifier.
        algorithm_version: &'static str,
        /// Component accumulation order, when the operation combines lanes.
        accumulation_order: Option<ComponentAccumulationOrder>,
        /// Diagnostics produced by this operation.
        diagnostics: Vec<ProcessingDiagnostic>,
    },
}

/// Frozen order used while accumulating component-transform lanes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentAccumulationOrder {
    /// Lanes are accumulated from zero through `L - 1`.
    LaneAscending,
}

impl ProcessingRecord {
    /// Returns the scope on which this record's resolved parameters operate.
    pub fn target(&self) -> OperationTarget {
        self.requested().target()
    }

    /// Returns the operation exactly as requested.
    pub fn requested(&self) -> &ProcessingRequest {
        match self {
            Self::Attempted { requested, .. } | Self::Applied { requested, .. } => requested,
        }
    }

    /// Returns the parameters actually applied, or `None` for a failed attempt.
    /// Interpret these together with [`Self::target`].
    pub fn resolved(&self) -> Option<&ResolvedOperation> {
        match self {
            Self::Applied { resolved, .. } => Some(resolved),
            Self::Attempted { .. } => None,
        }
    }

    /// Returns the recoverable failure from an unapplied attempt.
    pub fn failure(&self) -> Option<AttemptFailure> {
        match self {
            Self::Attempted { failure, .. } => Some(*failure),
            Self::Applied { .. } => None,
        }
    }

    /// Returns diagnostics retained for this record.
    pub fn diagnostics(&self) -> &[ProcessingDiagnostic] {
        match self {
            Self::Attempted { diagnostics, .. } | Self::Applied { diagnostics, .. } => diagnostics,
        }
    }

    /// Returns descriptors before and after an applied operation.
    pub fn descriptor_transition(&self) -> Option<(&ProcessedDescriptor, &ProcessedDescriptor)> {
        match self {
            Self::Applied {
                input_descriptor,
                output_descriptor,
                ..
            } => Some((input_descriptor, output_descriptor)),
            Self::Attempted { .. } => None,
        }
    }

    /// Returns the frozen algorithm identifier for an applied operation.
    pub fn algorithm_version(&self) -> Option<&'static str> {
        match self {
            Self::Applied {
                algorithm_version, ..
            } => Some(*algorithm_version),
            _ => None,
        }
    }

    pub(crate) fn applied(
        requested: impl Into<ProcessingRequest>,
        resolved: ResolvedOperation,
        input_descriptor: ProcessedDescriptor,
        output_descriptor: ProcessedDescriptor,
        diagnostics: Vec<ProcessingDiagnostic>,
    ) -> Self {
        let requested = requested.into();
        let algorithm_version = algorithm_version(&requested);
        let accumulation_order = matches!(
            requested,
            ProcessingRequest::Explicit(ProcessingOperation::ComponentTransform { .. })
        )
        .then_some(ComponentAccumulationOrder::LaneAscending);
        Self::Applied {
            requested,
            resolved: Box::new(resolved),
            input_descriptor,
            output_descriptor,
            algorithm_version,
            accumulation_order,
            diagnostics,
        }
    }

    pub(crate) fn attempted(
        requested: impl Into<ProcessingRequest>,
        failure: AttemptFailure,
        diagnostics: Vec<ProcessingDiagnostic>,
    ) -> Self {
        Self::Attempted {
            requested: requested.into(),
            failure,
            diagnostics,
        }
    }
}
