use super::operation::{OperationTarget, ProcessingOperation, ResolvedOperation};
use crate::processed::ProcessedDescriptor;

#[derive(Clone, Debug)]
pub(in crate::processing) struct PreparedStep {
    pub(in crate::processing) step_index: usize,
    pub(in crate::processing) axis: Option<usize>,
    pub(in crate::processing) before: ProcessedDescriptor,
    pub(in crate::processing) after: ProcessedDescriptor,
    pub(in crate::processing) resolved: ResolvedOperation,
}

impl PreparedStep {
    /// Only single-axis kernels call this after preflight establishes their target.
    pub(in crate::processing) fn axis(&self) -> usize {
        self.axis
            .expect("single-axis kernel has a validated target")
    }
}

/// An immutable view of one resolved, not yet executed, explicit operation.
///
/// This preview grants no execution provenance. Descriptors and parameters are
/// the same values the prepared executor uses; the view cannot alter them.
///
/// ```compile_fail,E0594
/// use nmr::processing::{PreparedStepView, ResolvedOperation};
/// fn change(step: PreparedStepView<'_>) {
///     if let ResolvedOperation::ZeroFill { target_points, .. } = step.resolved() {
///         *target_points = 1;
///     }
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct PreparedStepView<'a> {
    pub(in crate::processing) requested: &'a ProcessingOperation,
    pub(in crate::processing) prepared: &'a PreparedStep,
}

impl<'a> PreparedStepView<'a> {
    /// Returns the original request.
    pub fn requested(self) -> &'a ProcessingOperation {
        self.requested
    }

    /// Returns the actual logical scope, in the descriptor before this step.
    pub fn target(self) -> OperationTarget {
        self.requested.target()
    }

    /// Returns the state-dependent parameters frozen by preflight.
    pub fn resolved(self) -> &'a ResolvedOperation {
        &self.prepared.resolved
    }

    /// Returns the numerical rule identifier that execution will record.
    pub fn algorithm_version(self) -> &'static str {
        crate::processing::contracts::history::explicit_algorithm_version(self.requested)
    }

    /// Returns the scientific descriptor immediately before this step.
    pub fn input_descriptor(self) -> &'a ProcessedDescriptor {
        &self.prepared.before
    }

    /// Returns the scientific descriptor immediately after this step.
    pub fn output_descriptor(self) -> &'a ProcessedDescriptor {
        &self.prepared.after
    }
}
