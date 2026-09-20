//! Evidence-bearing library derivations that establish a new processing input.
use crate::{
    Dataset,
    dataset::DatasetMetadata,
    processed::{ProcessedDescriptor, ProcessedProvenance, RawDatasetSnapshot},
    provenance::CanonicalDatasetDigests,
};

/// Immutable input evidence, excluding ancestor sample buffers.
#[derive(Clone, Debug, PartialEq)]
pub enum DerivationInput {
    /// Raw acquisition descriptor, schedule, normalization and source identity.
    Raw {
        /// Complete sample-free acquisition snapshot.
        snapshot: Box<RawDatasetSnapshot>,
        /// Original read selection, identity and warnings.
        metadata: DatasetMetadata,
    },
    /// Processed descriptor, history, source identity and read context.
    Processed(Box<crate::external::ProcessedEvidence>),
}
impl DerivationInput {
    /// Input scientific identity, in its ordered source slot.
    pub fn canonical_digests(&self) -> CanonicalDatasetDigests {
        match self {
            Self::Raw { snapshot, .. } => snapshot.canonical_digests(),
            Self::Processed(p) => p.canonical_digests(),
        }
    }
    /// Original source context, including warnings.
    pub fn metadata(&self) -> &DatasetMetadata {
        match self {
            Self::Raw { metadata, .. } => metadata,
            Self::Processed(p) => p.metadata(),
        }
    }
    pub(crate) fn capture(input: &Dataset) -> Self {
        if let Some(raw) = input.as_raw() {
            Self::Raw {
                snapshot: Box::new(raw.snapshot_with_digests(input.canonical_digests())),
                metadata: input.metadata().clone(),
            }
        } else {
            Self::Processed(Box::new(crate::external::ProcessedEvidence::capture(input)))
        }
    }
}

/// Library-executed operation across one or two scientific inputs.
#[derive(Clone, Debug, PartialEq)]
pub enum DerivationOperation {
    /// A + scale*B, interpolating B linearly onto A, zero outside B's range.
    /// Retains A's reference and digital-filter state shared by both inputs.
    LinearCombination {
        /// Finite multiplier, negative for subtraction.
        scale: f64,
    },
    /// Explicit F2 operations on acquired observations, then general-grid IST.
    NusReconstruction {
        /// Explicit direct-axis plan; no default steps are inserted.
        direct_operations: Vec<crate::processing::ProcessingOperation>,
        /// Full-grid resolved component, delay, window and FFT parameters.
        direct_resolved: Vec<crate::processing::ResolvedOperation>,
        /// Recorded scientific reconstruction settings.
        settings: crate::processing::NusSettings,
        /// Actual noise policy, resolved sigma and validation evidence.
        noise_report: Box<crate::processing::NusNoiseReport>,
        /// Maximum actual iteration count across direct columns.
        iterations: usize,
        /// Actual final column thresholds in input amplitude units.
        column_thresholds: Vec<f64>,
        /// Actual normalized iterate changes, one per direct column.
        column_relative_changes: Vec<f64>,
    },
}
impl DerivationOperation {
    /// Stable library algorithm identity, distinct from caller declarations.
    pub fn algorithm_version(&self) -> &'static str {
        match self {
            Self::LinearCombination { .. } => "linear-combination-zero-outside.v1",
            Self::NusReconstruction { .. } => "nus-direct-group-ist.v1",
        }
    }
}

/// Checked derivation history and its fresh processing starting point.
/// Replay takes the original inputs in the recorded order; snapshot restoration
/// can continue from current samples without accessing those ancestors.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryDerivation {
    environment: crate::processing::ExecutionEnvironment,
    accepted_archive: bool,
    pub(crate) inputs: Vec<DerivationInput>,
    pub(crate) operation: DerivationOperation,
    pub(crate) descriptor: ProcessedDescriptor,
    pub(crate) state: crate::processing::contracts::state::PlanState,
    pub(crate) digests: CanonicalDatasetDigests,
}
impl LibraryDerivation {
    /// Available build and numerical environment at execution time.
    pub fn environment(&self) -> &crate::processing::ExecutionEnvironment {
        &self.environment
    }
    /// Whether this execution was explicitly accepted from a snapshot.
    pub fn accepted_archive(&self) -> bool {
        self.accepted_archive
    }

    /// Ordered original source slots, including both parents of binary arithmetic.
    pub fn inputs(&self) -> &[DerivationInput] {
        &self.inputs
    }
    /// Actual library operation and reconstruction diagnostics.
    pub fn operation(&self) -> &DerivationOperation {
        &self.operation
    }
    /// Descriptor immediately after this derivation, before continuation plans.
    pub fn descriptor(&self) -> &ProcessedDescriptor {
        &self.descriptor
    }
    /// Scientific identity at this processing boundary.
    pub fn canonical_digests(&self) -> CanonicalDatasetDigests {
        self.digests
    }
    /// Replay this library derivation against matching input identities.
    pub fn replay(
        &self,
        inputs: &[&Dataset],
        options: crate::processing::ProcessingOptions,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<Dataset, crate::processing::ProcessingError> {
        use crate::processing::*;
        if inputs.len() != self.inputs.len() {
            return Err(ProcessingError::InputCountMismatch {
                expected: self.inputs.len(),
                actual: inputs.len(),
            });
        }
        if inputs
            .iter()
            .zip(&self.inputs)
            .any(|(a, b)| a.canonical_digests() != b.canonical_digests())
        {
            return Err(ProcessingError::InputIdentityMismatch);
        }
        for (actual, recorded) in inputs.iter().zip(&self.inputs) {
            let sources = match recorded {
                DerivationInput::Raw { snapshot, .. } => {
                    if actual.as_raw().is_none_or(|raw| {
                        raw.provenance().sample_normalization() != snapshot.sample_normalization()
                    }) {
                        return Err(ProcessingError::InputIdentityMismatch);
                    }
                    snapshot.sources()
                }
                DerivationInput::Processed(parent) => {
                    if actual.as_processed().is_none_or(|p| {
                        p.provenance().read_record() != parent.provenance().read_record()
                    }) {
                        return Err(ProcessingError::InputIdentityMismatch);
                    }
                    parent.provenance().sources()
                }
            };
            if actual.sources().len() != sources.len()
                || actual.sources().iter().zip(sources).any(|(a, b)| {
                    a.kind() != b.kind() || a.role() != b.role() || a.digest() != b.digest()
                })
            {
                return Err(ProcessingError::InputIdentityMismatch);
            }
        }
        match &self.operation {
            DerivationOperation::LinearCombination { scale } => LinearCombination::new(*scale)?
                .prepare(inputs[0], inputs[1], options)?
                .execute_with_context(control),
            DerivationOperation::NusReconstruction {
                direct_operations,
                settings,
                noise_report,
                ..
            } if noise_report.source != NusNoiseSource::Explicit => AutoNusSettings {
                max_iterations: settings.max_iterations,
            }
            .prepare_with_context(
                inputs[0],
                ProcessingPlan::new(direct_operations.clone())?,
                options,
                control,
            )?
            .recorded_noise_source(noise_report.source)?
            .execute_with_context(control),
            DerivationOperation::NusReconstruction {
                direct_operations,
                settings,
                ..
            } => settings
                .prepare(
                    inputs[0],
                    ProcessingPlan::new(direct_operations.clone())?,
                    options,
                )?
                .execute_with_context(control),
        }
    }
    pub(crate) fn accept_archived_history(&mut self) {
        self.accepted_archive = true;
        for input in &mut self.inputs {
            if let DerivationInput::Processed(p) = input {
                p.accept_archived_history();
            }
        }
    }
    pub(crate) fn validate_recorded(
        &self,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<(), crate::internal::ModelError> {
        use crate::internal::ModelError as E;
        let count = match self.operation {
            DerivationOperation::LinearCombination { .. } => 2,
            DerivationOperation::NusReconstruction { .. } => 1,
        };
        if self.inputs.len() != count
            || self.state.descriptor().map_err(|_| E::Structure)? != self.descriptor
        {
            return Err(E::Structure);
        }
        for input in &self.inputs {
            match input {
                DerivationInput::Raw { snapshot, .. } => snapshot.validate_recorded(control)?,
                DerivationInput::Processed(p) => p.validate_recorded(control)?,
            }
        }
        match &self.operation {
            DerivationOperation::LinearCombination { scale } => {
                let [DerivationInput::Processed(a), DerivationInput::Processed(b)] =
                    self.inputs.as_slice()
                else {
                    return Err(E::Structure);
                };
                let state_a =
                    crate::processed::dataset::processing_state(a.descriptor(), a.provenance())
                        .map_err(|_| E::Structure)?;
                let state_b =
                    crate::processed::dataset::processing_state(b.descriptor(), b.provenance())
                        .map_err(|_| E::Structure)?;
                let expected = crate::processing::contracts::combination::combined_state(
                    &self.descriptor,
                    &state_a,
                    &state_b,
                );
                if !scale.is_finite()
                    || a.descriptor() != &self.descriptor
                    || self.state != expected
                {
                    return Err(E::Structure);
                }
                crate::processing::contracts::combination::compatible(
                    a.descriptor(),
                    b.descriptor(),
                )
                .map_err(|_| E::Structure)?;
            }
            DerivationOperation::NusReconstruction {
                direct_operations,
                direct_resolved,
                settings,
                noise_report,
                iterations,
                column_thresholds,
                column_relative_changes,
            } => {
                if !(1..=2048).contains(&settings.max_iterations)
                    || !noise_report.validate(
                        self.descriptor.axes().last().ok_or(E::Structure)?.points(),
                        *settings,
                    )
                {
                    return Err(E::Structure);
                }
                if *iterations > settings.max_iterations {
                    return Err(E::Structure);
                }
                let [DerivationInput::Raw { snapshot, .. }] = self.inputs.as_slice() else {
                    return Err(E::Structure);
                };
                let axes = snapshot.descriptor().axes();
                if axes.len() != 2
                    || axes[0].domain() != crate::axis::AxisDomain::Time
                    || !matches!(axes[0].coordinates(),crate::axis::AxisCoordinates::Uniform { step, .. } if *step>0.0)
                {
                    return Err(E::Structure);
                }
                let full = crate::processing::contracts::state::expand_raw_descriptor(axes)
                    .map_err(|_| E::Structure)?;
                let mut state = crate::processing::contracts::state::PlanState::from_raw(
                    &full,
                    axes,
                    snapshot.sampling_schedule(),
                    snapshot.absolute_origin(),
                )
                .map_err(|_| E::Structure)?;
                if direct_operations.is_empty() || direct_operations.len() != direct_resolved.len()
                {
                    return Err(E::Structure);
                }
                for (request, resolved) in direct_operations.iter().zip(direct_resolved) {
                    if !(request.axis() == Some(1)
                        || matches!(
                            request,
                            crate::processing::ProcessingOperation::ComponentTransform { axis: 0 }
                        ))
                    {
                        return Err(E::Structure);
                    }
                    if crate::processing::contracts::state::transition(&mut state, request)
                        .map_err(|_| E::Structure)?
                        != *resolved
                    {
                        return Err(E::Structure);
                    }
                }
                if state != self.state
                    || state.axes.len() != 2
                    || !matches!(
                        state.axes[0].axis.component_basis(),
                        crate::processed::ComponentBasis::Cartesian
                            | crate::processed::ComponentBasis::SharedComplex { .. }
                    )
                    || state.axes[1].axis.domain() != crate::axis::AxisDomain::Frequency
                {
                    return Err(E::Structure);
                }
                let columns = self.descriptor.axes().last().ok_or(E::Structure)?.points();
                if noise_report.source != crate::processing::NusNoiseSource::Explicit {
                    let m = snapshot
                        .sampling_schedule()
                        .map_or(axes[0].points(), |s| s.coordinates().len());
                    let shared = matches!(
                        self.descriptor.axes()[0].component_basis(),
                        crate::processed::ComponentBasis::SharedComplex { .. }
                    );
                    let k = if shared {
                        2
                    } else {
                        2 * self.descriptor.axes()[1].component_count()
                    };
                    if noise_report.source.interior()
                        && !crate::processing::methods::nus::jeol_filtered(&axes[1])
                    {
                        return Err(E::Structure);
                    }
                    let short = noise_report.source.holdout();
                    let folds = if short { 2 } else { 3 };
                    let bins: usize = noise_report
                        .frequency_ranges
                        .iter()
                        .map(|(a, b)| b - a)
                        .sum();
                    if (if short {
                        !(32..48).contains(&m) || columns < 128
                    } else {
                        m < 48 || columns < 32
                    }) || noise_report.effective_observations != m / folds
                        || m.checked_div(folds)
                            .and_then(|m| m.checked_mul(bins))
                            .and_then(|n| n.checked_mul(k))
                            != Some(noise_report.scalar_samples)
                    {
                        return Err(E::Structure);
                    }
                    for (quarter, &(start, end)) in noise_report.frequency_ranges.iter().enumerate()
                    {
                        let guard = if noise_report.source.interior() { 4 } else { 0 };
                        let first = (quarter * 8).max(guard);
                        let last = ((quarter + 1) * 8).min(32 - guard);
                        if !(first..last)
                            .any(|b| start == b * columns / 32 && end == (b + 1) * columns / 32)
                        {
                            return Err(E::Structure);
                        }
                    }
                    crate::processing::methods::nus::validate_automatic_plan(direct_operations)
                        .map_err(|_| E::Structure)?;
                }
                if column_thresholds.len() != columns
                    || column_relative_changes.len() != columns
                    || column_thresholds
                        .iter()
                        .chain(column_relative_changes)
                        .any(|v| !v.is_finite() || *v < 0.0)
                {
                    return Err(E::Structure);
                }
            }
        }
        crate::canonical_digest::check_processed_evidence(
            &self.descriptor,
            &self.state,
            self.digests,
            Some(control.cancellation()),
        )?;
        Ok(())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, crate::processing::ProcessingError> {
        use crate::processing::prepare::memory as m;
        let mut total = m::sum([
            std::mem::size_of::<Self>(),
            m::descriptor(&self.descriptor)?,
            self.environment.crate_version().len()
                + self.environment.architecture().len()
                + self.environment.operating_system().len()
                + self
                    .environment
                    .numerical_dependency_versions()
                    .map_or(0, str::len)
                + self.environment.explicit_fft_backend().len()
                + self.environment.build_identifier().map_or(0, str::len),
            m::array::<DerivationInput>(self.inputs.len())?,
        ])?;
        for input in &self.inputs {
            let bytes = match input {
                DerivationInput::Raw { snapshot, metadata } => {
                    m::sum([m::snapshot(snapshot)?, m::aggregate(metadata)?])?
                }
                DerivationInput::Processed(p) => m::sum([
                    std::mem::size_of_val(p.as_ref()),
                    m::descriptor(p.descriptor())?,
                    m::origin(p.provenance().origin())?,
                    m::sources(p.provenance().sources())?,
                    p.provenance()
                        .history()
                        .map(m::history)
                        .transpose()?
                        .unwrap_or(0),
                    m::source_metadata(p.provenance().source_metadata())?,
                    m::read_record(p.provenance().read_record())?,
                    m::aggregate(p.metadata())?,
                ])?,
            };
            total = m::sum([total, bytes])?;
        }
        total = m::sum([
            total,
            crate::processing::prepare::resources::state_storage_bytes(
                self.state.axes.len(),
                self.state
                    .observation_ordinals
                    .as_ref()
                    .map_or(0, |v| v.len()),
            )?,
        ])?;
        if let DerivationOperation::NusReconstruction {
            direct_operations,
            direct_resolved,
            column_thresholds,
            column_relative_changes,
            noise_report,
            ..
        } = &self.operation
        {
            total = m::sum([
                total,
                m::array::<crate::processing::ProcessingOperation>(direct_operations.len())?,
                m::array::<crate::processing::ResolvedOperation>(direct_resolved.len())?,
                m::array::<f64>(column_thresholds.len() + column_relative_changes.len())?,
                std::mem::size_of::<crate::processing::NusNoiseReport>(),
                m::array::<(usize, usize)>(noise_report.frequency_ranges.len())?,
            ])?;
        }
        Ok(total)
    }
}

pub(crate) fn provenance(
    inputs: Vec<DerivationInput>,
    operation: DerivationOperation,
    descriptor: ProcessedDescriptor,
    state: crate::processing::contracts::state::PlanState,
    digests: CanonicalDatasetDigests,
    sources: Vec<crate::provenance::SourceFile>,
) -> ProcessedProvenance {
    ProcessedProvenance::library(
        LibraryDerivation {
            environment: crate::processing::ExecutionEnvironment::current(),
            accepted_archive: false,
            inputs,
            operation,
            descriptor,
            state,
            digests,
        },
        sources,
    )
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl LibraryDerivation {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &crate::processing::ExecutionEnvironment,
        &bool,
        &Vec<DerivationInput>,
        &DerivationOperation,
        &ProcessedDescriptor,
        &crate::processing::contracts::state::PlanState,
        &CanonicalDatasetDigests,
    ) {
        (
            &self.environment,
            &self.accepted_archive,
            &self.inputs,
            &self.operation,
            &self.descriptor,
            &self.state,
            &self.digests,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            crate::processing::ExecutionEnvironment,
            bool,
            Vec<DerivationInput>,
            DerivationOperation,
            ProcessedDescriptor,
            crate::processing::contracts::state::PlanState,
            CanonicalDatasetDigests,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (environment, accepted_archive, inputs, operation, descriptor, state, digests) = parts;
        let value = Self {
            environment,
            accepted_archive,
            inputs,
            operation,
            descriptor,
            state,
            digests,
        };

        Ok(value)
    }
}
