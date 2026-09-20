//! Synchronous cooperative execution control, independent of a host task runtime.

use crate::resource::WorkLedger;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Shareable cancellation signal. Clones refer to the same signal.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    /// Creates an uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }
    /// Requests cancellation for all clones.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    /// Checks the signal without consuming it.
    pub fn check(&self) -> Result<(), ExecutionError> {
        if self.is_cancelled() {
            Err(ExecutionError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Meaning of a progress event's work count.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStage {
    /// Validation before numerical execution.
    Preflight,
    /// Explicit operation execution.
    Processing,
    /// Automatic phase search.
    AutoPhase,
    /// Automatic noise selection, validation and estimation.
    NoiseEstimation,
    /// Iterative reconstruction.
    Reconstruction,
    /// Recorded operation execution.
    Replay,
    /// Reading source data.
    Reading,
    /// Copying or preparing a data view.
    Access,
    /// Plot preparation.
    Plot,
    /// Snapshot encoding or decoding.
    Snapshot,
    /// Report encoding.
    Report,
    /// Data export.
    Export,
    /// Scientific identity calculation.
    Digest,
}

/// Known work total or conservative upper bound; neither implies elapsed-time accuracy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressTotal {
    /// Exact count in the event's units.
    Exact(u128),
    /// Conservative upper bound in the event's units.
    UpperBound(u128),
}

/// Synchronous observation of a stage. Callbacks run on the caller's thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressEvent {
    /// Current stage.
    pub stage: ExecutionStage,
    /// Zero-based caller operation index, when relevant.
    pub step_index: Option<usize>,
    /// Completed work in this stage's units.
    pub completed: u128,
    /// Known total or upper bound; absent when unknown.
    pub total: Option<ProgressTotal>,
}

/// Control failure, independent of scientific or I/O errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ExecutionError {
    /// Caller requested cancellation.
    #[error("execution cancelled")]
    Cancelled,
    /// Remaining numerical work is insufficient.
    #[error("execution work budget exhausted")]
    WorkLimit,
    /// A resource counter overflowed.
    #[error("execution resource counter overflow")]
    SizeOverflow,
}

enum Ledger<'a> {
    Borrowed(&'a mut WorkLedger),
    Owned(WorkLedger),
}

/// Composable synchronous context. The library creates no threads or runtime.
/// Numeric work, I/O bytes and peak payload bytes use independent counters.
pub struct ExecutionContext<'a> {
    ledger: Ledger<'a>,
    cancellation: CancellationToken,
    progress: Option<&'a mut dyn FnMut(ProgressEvent)>,
    event: ProgressEvent,
    io_bytes: u128,
    peak_payload_bytes: usize,
    pending_work: u128,
}
impl Default for ExecutionContext<'_> {
    fn default() -> Self {
        Self {
            ledger: Ledger::Owned(WorkLedger::processing_default()),
            cancellation: CancellationToken::new(),
            progress: None,
            event: ProgressEvent {
                stage: ExecutionStage::Preflight,
                step_index: None,
                completed: 0,
                total: None,
            },
            io_bytes: 0,
            peak_payload_bytes: 0,
            pending_work: 0,
        }
    }
}
impl<'a> ExecutionContext<'a> {
    /// Borrows a shared work ledger. Reuse that ledger across calls to accumulate work.
    pub fn new(ledger: &'a mut WorkLedger) -> Self {
        Self {
            ledger: Ledger::Borrowed(ledger),
            ..Self::default()
        }
    }
    /// Uses a cancellation signal which may also be held by another thread.
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = token;
        self
    }
    /// Observes synchronous progress, including stage boundaries.
    pub fn with_progress(mut self, receiver: &'a mut dyn FnMut(ProgressEvent)) -> Self {
        self.progress = Some(receiver);
        self
    }
    /// Borrows the context's cancellation token.
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }
    /// Borrows the numerical ledger.
    pub fn ledger(&self) -> &WorkLedger {
        match &self.ledger {
            Ledger::Borrowed(v) => v,
            Ledger::Owned(v) => v,
        }
    }
    /// Total I/O bytes observed by this context.
    pub fn io_bytes(&self) -> u128 {
        self.io_bytes
    }
    /// Largest reported simultaneous library payload, excluding caller storage and RSS.
    pub fn peak_payload_bytes(&self) -> usize {
        self.peak_payload_bytes
    }
    /// Checks cancellation without changing budget or progress.
    pub fn check_cancelled(&self) -> Result<(), ExecutionError> {
        self.cancellation.check()
    }
    pub(crate) fn ensure_work(&self, units: u128) -> Result<(), ExecutionError> {
        self.check_cancelled()?;
        if units > self.ledger().limit() - self.ledger().used() {
            Err(ExecutionError::WorkLimit)
        } else {
            Ok(())
        }
    }
    // Complete the previous sequential block before entering the next one.
    // A failed call discards its pending completion at the next stage boundary.
    pub(crate) fn charge(&mut self, units: u128) -> Result<(), ExecutionError> {
        self.complete_work()?;
        self.charge_without_progress(units)?;
        self.pending_work = units;
        Ok(())
    }
    pub(crate) fn charge_without_progress(&mut self, units: u128) -> Result<(), ExecutionError> {
        self.ensure_work(units)?;
        let ledger = match &mut self.ledger {
            Ledger::Borrowed(v) => v,
            Ledger::Owned(v) => v,
        };
        ledger.charge(units).map_err(Into::into)
    }
    pub(crate) fn complete_work(&mut self) -> Result<(), ExecutionError> {
        let units = std::mem::take(&mut self.pending_work);
        if units == 0 {
            return self.check_cancelled();
        }
        self.advance(units)
    }
    pub(crate) fn begin(
        &mut self,
        stage: ExecutionStage,
        step_index: Option<usize>,
        total: Option<ProgressTotal>,
    ) -> Result<(), ExecutionError> {
        self.check_cancelled()?;
        self.pending_work = 0;
        self.event = ProgressEvent {
            stage,
            step_index,
            completed: 0,
            total,
        };
        self.emit()
    }
    pub(crate) fn advance(&mut self, units: u128) -> Result<(), ExecutionError> {
        self.check_cancelled()?;
        self.event.completed = self
            .event
            .completed
            .checked_add(units)
            .ok_or(ExecutionError::SizeOverflow)?;
        self.emit()
    }
    fn emit(&mut self) -> Result<(), ExecutionError> {
        if let Some(receiver) = &mut self.progress {
            receiver(self.event);
        }
        self.check_cancelled()
    }
    pub(crate) fn observe_payload(&mut self, bytes: usize) {
        self.peak_payload_bytes = self.peak_payload_bytes.max(bytes);
    }
    pub(crate) fn io(&mut self, bytes: usize) -> Result<(), ExecutionError> {
        self.io_bytes = self
            .io_bytes
            .checked_add(bytes as u128)
            .ok_or(ExecutionError::SizeOverflow)?;
        self.advance(bytes as u128)
    }
}

impl From<crate::resource::ResourceError> for ExecutionError {
    fn from(error: crate::resource::ResourceError) -> Self {
        match error {
            crate::resource::ResourceError::WorkLimit => Self::WorkLimit,
            crate::resource::ResourceError::SizeOverflow => Self::SizeOverflow,
        }
    }
}
