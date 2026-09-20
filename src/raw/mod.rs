//! Vendor-neutral raw-dataset model and bounded path access.
//!
//! Logical coordinates, quadrature lanes, and canonical storage remain
//! distinct. Readers apply only storage-level decoding, reordering, sign
//! conventions, and numeric scaling. Public complex values use `R + iI`; a
//! reader changes a vendor imaginary sign only when format evidence establishes
//! that transform.

pub use crate::ReadLimits;
pub use crate::acquisition::{
    AssertionId, AxisIndex, ChemicalShiftReference, ComponentEvidence, DirectSamples,
    EvidenceValidationError, GroupDelayState, IndirectComponents, LinearComponentTransform,
    ModulationIndexDomain, NormalizationEvidence, NormalizationFact, PendingGroupDelay,
    PeriodicLaneModulation, RawAxisKind, ResolutionAuthority, ResolvedComponentTransform, RuleId,
    TransformValidationError, TrustClass,
};
pub use crate::io::Reader;
pub use crate::provenance::{
    SampleNormalization, SourceDigest, SourceFile, SourceId, SourceIdentity, SourceKind,
};
pub use crate::raw::model::{
    AccessError, DiffusionAcquisition, ObservationOrdinal, RawAxis, RawData, RawDataset,
    RawDatasetBuilder, RawDescriptor, RawFormat, RawLayout, RawMetadata, RawProvenance,
    SamplingCoordinate, SamplingSchedule, SparseTrace, StorageOrder, ValidationError,
    VendorMetadata,
};
pub use crate::read_error::{
    InputSource, ParameterError, ParameterErrorKind, ReadError, ReadErrorKind, ReadErrorReason,
    ReadResource, UnsupportedFeatureCode,
};

use crate::Complex64;
use std::path::Path;

/// Options used to establish a checked acquisition reader.
///
/// Defaults fail closed when vendor metadata does not establish a unique
/// scientific layout. Assertions can fill missing facts but never override source facts.
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    limits: ReadLimits,
    varian: Option<crate::formats::varian::ReadAssertions>,
    experimental_vendor_semantics: bool,
}

impl OpenOptions {
    /// Creates default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opts into vendor interpretations with incomplete independent evidence.
    ///
    /// Required for Bruker NUS and every JEOL layout, including digital-filter and
    /// list interpretations. This is an experimental compatibility boundary;
    /// enabling it does not certify the interpretation or permit unsupported layouts.
    pub fn allow_experimental_vendor_semantics(mut self, value: bool) -> Self {
        self.experimental_vendor_semantics = value;
        self
    }

    /// Replaces resource limits.
    pub fn limits(mut self, value: ReadLimits) -> Self {
        self.limits = value;
        self
    }

    /// Supplies complete Varian acquisition assertions.
    pub fn varian_assertions(mut self, value: crate::formats::varian::ReadAssertions) -> Self {
        self.varian = Some(value);
        self
    }

    /// Returns configured resource limits.
    pub fn read_limits(&self) -> ReadLimits {
        self.limits
    }

    /// Opens a path after format detection and layout validation.
    pub fn open(&self, path: impl AsRef<Path>) -> Result<Reader, ReadError> {
        self.open_with_context(path, &mut crate::ExecutionContext::default())
    }

    /// Opens and validates a source with cooperative cancellation.
    pub fn open_with_context(
        &self,
        path: impl AsRef<Path>,
        control: &mut crate::ExecutionContext<'_>,
    ) -> Result<Reader, ReadError> {
        control.begin(crate::execution::ExecutionStage::Reading, None, None)?;
        let path = path.as_ref();
        let candidate = crate::reading::resolver::resolve_raw_and_select(path)?;
        candidate.validate_required()?;
        let reader = candidate.open_raw(
            &self.limits,
            self.varian.as_ref(),
            self.experimental_vendor_semantics,
            None,
            control.cancellation(),
        )?;
        control.check_cancelled()?;
        Ok(reader)
    }
}

/// Detects and selects the raw dataset format identified by a path.
///
/// Recognition is not a support probe. A successful result does not promise
/// that [`open`] or [`read`] supports the selected acquisition layout.
pub fn detect(path: impl AsRef<Path>) -> Result<RawFormat, ReadError> {
    let path = path.as_ref();
    let candidate = crate::reading::resolver::resolve_raw_and_select(path)?;
    match candidate.format {
        crate::Format::Raw(format) => Ok(format),
        crate::Format::Processed(_) => unreachable!(),
    }
}

/// Opens a raw dataset with default resource limits.
pub fn open(path: impl AsRef<Path>) -> Result<Reader, ReadError> {
    OpenOptions::new().open(path)
}

/// Reads and materializes a complete raw dataset with default options.
pub fn read(path: impl AsRef<Path>) -> Result<RawDataset, ReadError> {
    read_with_context(path, &mut crate::ExecutionContext::default())
}

/// Samples returned for one logical indirect coordinate.
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    coordinate: Box<[usize]>,
    observation_ordinal: Option<ObservationOrdinal>,
    direct_points: usize,
    component_lanes: Box<[usize]>,
    samples: Box<[Complex64]>,
}

impl Trace {
    pub(crate) fn new(
        coordinate: &[usize],
        observation_ordinal: Option<ObservationOrdinal>,
        direct_points: usize,
        component_lanes: &[usize],
        samples: Vec<Complex64>,
    ) -> Result<Self, ReadError> {
        let component_count = component_lanes
            .iter()
            .try_fold(1usize, |count, &lanes| count.checked_mul(lanes))
            .ok_or(ReadError::SizeOverflow)?;
        let expected = direct_points
            .checked_mul(component_count)
            .ok_or(ReadError::SizeOverflow)?;
        if samples.len() != expected {
            return Err(ReadError::corrupt(
                InputSource::memory("decoded trace"),
                "decoded trace length does not match its component layout",
            ));
        }
        Ok(Self {
            coordinate: coordinate.into(),
            observation_ordinal,
            direct_points,
            component_lanes: component_lanes.into(),
            samples: samples.into(),
        })
    }

    /// Returns the zero-based logical indirect coordinate.
    pub fn coordinate(&self) -> &[usize] {
        &self.coordinate
    }

    /// Returns the exact acquisition-order observation for ordinal reads.
    pub fn observation_ordinal(&self) -> Option<ObservationOrdinal> {
        self.observation_ordinal
    }

    /// Returns the number of logical points on the direct axis.
    pub fn direct_points(&self) -> usize {
        self.direct_points
    }

    /// Returns component-lane counts for every raw axis.
    pub fn component_lanes(&self) -> &[usize] {
        &self.component_lanes
    }

    /// Returns component-major samples with direct points fastest.
    pub fn samples(&self) -> &[Complex64] {
        &self.samples
    }
}

impl std::ops::Deref for Trace {
    type Target = [Complex64];

    fn deref(&self) -> &Self::Target {
        self.samples()
    }
}

/// A checked logical region request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Region {
    start: Box<[usize]>,
    shape: Box<[usize]>,
}

impl Region {
    /// Creates a non-empty region whose rank is validated immediately.
    pub fn new(
        start: impl Into<Box<[usize]>>,
        shape: impl Into<Box<[usize]>>,
    ) -> Result<Self, ValidationError> {
        let start = start.into();
        let shape = shape.into();
        if start.len() != shape.len() || shape.is_empty() || shape.contains(&0) {
            return Err(ValidationError::InvalidRegion);
        }
        Ok(Self { start, shape })
    }

    /// Returns the absolute zero-based origin in the full logical grid.
    pub fn start(&self) -> &[usize] {
        &self.start
    }

    /// Returns logical point counts for the requested region.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }
}

/// Raw data returned for a logical region.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionData {
    region: Region,
    data: RawData,
}

impl RegionData {
    pub(crate) fn new(region: Region, data: RawData) -> Self {
        Self { region, data }
    }

    /// Returns the region represented by this result.
    pub fn region(&self) -> &Region {
        &self.region
    }

    /// Returns dense or sparse samples retaining the region's absolute grid origin.
    pub fn data(&self) -> &RawData {
        &self.data
    }

    /// Consumes the wrapper and returns its normalized samples.
    pub fn into_data(self) -> RawData {
        self.data
    }
}

/// Opens and materializes raw data with shared execution control.
pub fn read_with_context(
    path: impl AsRef<Path>,
    control: &mut crate::ExecutionContext<'_>,
) -> Result<RawDataset, ReadError> {
    OpenOptions::new()
        .open_with_context(path, control)?
        .into_dataset_with_context(control)
}

pub(crate) mod model;
