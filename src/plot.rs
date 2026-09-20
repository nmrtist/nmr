//! Checked scalar data prepared for plotting or export.

use crate::axis::{AxisCoordinates, AxisDirection, AxisQuantity, AxisRole, AxisUnit};
use crate::execution::{ExecutionContext, ExecutionError, ExecutionStage};
use crate::processed::{ProcessedDataset, ProcessedProvenance};
use crate::resource::{LimitExceeded, MemoryLimits, ResourceEstimate, ResourceKind};
use thiserror::Error;

/// Quality of the source from which plot coordinates were obtained.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateSourceQuality {
    /// Coordinates have an established physical or experimental meaning.
    Established,
    /// Only a logical parameter index is available.
    Unknown,
}

/// Coordinates exposed by a plot axis.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum PlotCoordinates {
    /// Finite coordinates with a physical or experimental meaning.
    Physical {
        /// Physical unit when the quantity has one.
        unit: Option<AxisUnit>,
        /// One coordinate per logical point.
        values: Vec<f64>,
    },
    /// Zero-based logical indices for an uncalibrated parameter axis.
    LogicalIndex(Vec<u64>),
}

/// One checked axis in a scalar plot product.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotAxis {
    label: Option<String>,
    role: AxisRole,
    quantity: Option<AxisQuantity>,
    coordinates: PlotCoordinates,
    direction: AxisDirection,
    source_quality: CoordinateSourceQuality,
}

impl PlotAxis {
    /// Returns the optional source label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns the scientific role.
    pub fn role(&self) -> AxisRole {
        self.role
    }
    /// Returns the physical parameter quantity when established.
    pub fn quantity(&self) -> Option<AxisQuantity> {
        self.quantity
    }

    /// Returns physical coordinates or logical parameter indices.
    pub fn coordinates(&self) -> &PlotCoordinates {
        &self.coordinates
    }

    /// Returns the monotonic direction, if meaningful.
    pub fn direction(&self) -> AxisDirection {
        self.direction
    }

    /// Returns the quality of the coordinate source.
    pub fn source_quality(&self) -> CoordinateSourceQuality {
        self.source_quality
    }
}

/// Dense row-major scalar data with the direct axis fastest.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotData {
    shape: Vec<usize>,
    data: Vec<f64>,
    axes: Vec<PlotAxis>,
    provenance: ProcessedProvenance,
}

/// A plot conversion borrowing checked scalar input after resource preflight.
#[derive(Debug)]
pub struct PreparedPlot<'a> {
    input: &'a ProcessedDataset,
    resources: ResourceEstimate,
}

impl PreparedPlot<'_> {
    /// Copies plot payload with cooperative cancellation and progress.
    pub fn execute_with_context(
        self,
        control: &mut ExecutionContext<'_>,
    ) -> Result<PlotData, PlotError> {
        control.begin(ExecutionStage::Plot, None, None)?;
        control.observe_payload(self.resources.working_bytes());
        PlotData::materialize(self.input, control)
    }

    /// Returns the new sample, metadata and total peak payload bounds.
    pub fn resources(&self) -> ResourceEstimate {
        self.resources
    }

    /// Materializes the already-checked coordinates, samples and provenance.
    pub fn execute(self) -> Result<PlotData, PlotError> {
        self.execute_with_context(&mut ExecutionContext::default())
    }
}

impl PlotData {
    /// Prepares scalar plotting data with shared execution control.
    pub fn from_processed_with_context(
        dataset: &ProcessedDataset,
        limits: MemoryLimits,
        control: &mut ExecutionContext<'_>,
    ) -> Result<Self, PlotError> {
        control.check_cancelled()?;
        Self::preflight(dataset, limits)?.execute_with_context(control)
    }

    /// Creates plot data using finite default memory limits.
    pub fn from_processed(dataset: &ProcessedDataset) -> Result<Self, PlotError> {
        Self::from_processed_with_limits(dataset, MemoryLimits::default())
    }

    /// Creates plot data with limits checked before copying samples or provenance.
    pub fn from_processed_with_limits(
        dataset: &ProcessedDataset,
        limits: MemoryLimits,
    ) -> Result<Self, PlotError> {
        Self::preflight(dataset, limits)?.execute()
    }

    /// Checks scalar state and estimates conversion without allocating plot payload.
    /// Input storage and shared immutable axis backing already held by the caller
    /// are excluded. Output and metadata are subsets of total working bytes.
    pub fn preflight(
        dataset: &ProcessedDataset,
        limits: MemoryLimits,
    ) -> Result<PreparedPlot<'_>, PlotError> {
        if dataset
            .descriptor()
            .axes()
            .iter()
            .any(|axis| axis.component_count() != 1)
        {
            return Err(PlotError::NonScalarData);
        }
        let array = |count: usize, size: usize| {
            count
                .checked_mul(size)
                .filter(|bytes| *bytes <= isize::MAX as usize)
                .ok_or(PlotError::SizeOverflow)
        };
        let sum = |values: &[usize]| {
            values
                .iter()
                .try_fold(0usize, |sum, value| sum.checked_add(*value))
                .ok_or(PlotError::SizeOverflow)
        };
        let rank = dataset.descriptor().axes().len();
        let output = array(dataset.data().samples().len(), std::mem::size_of::<f64>())?;
        let mut metadata = sum(&[
            array(rank, std::mem::size_of::<PlotAxis>())?,
            array(rank, std::mem::size_of::<usize>())?,
            provenance_bytes(dataset.provenance())?,
        ])?;
        for axis in dataset.descriptor().axes() {
            if matches!(axis.coordinates(), AxisCoordinates::Unknown)
                && axis.role() != AxisRole::ArrayParameter
            {
                return Err(PlotError::MissingPhysicalCoordinates);
            }
            metadata = sum(&[
                metadata,
                axis.label().map_or(0, str::len),
                array(
                    axis.points(),
                    std::mem::size_of::<f64>().max(std::mem::size_of::<u64>()),
                )?,
            ])?;
        }
        let working = sum(&[output, metadata])?;
        for (resource, required, limit) in [
            (ResourceKind::OutputBytes, output, limits.output_bytes()),
            (
                ResourceKind::MetadataBytes,
                metadata,
                limits.metadata_bytes(),
            ),
            (ResourceKind::WorkingBytes, working, limits.working_bytes()),
        ] {
            if required > limit {
                return Err(LimitExceeded {
                    resource,
                    required,
                    limit,
                }
                .into());
            }
        }
        Ok(PreparedPlot {
            input: dataset,
            resources: ResourceEstimate::new(output, metadata, working),
        })
    }

    fn materialize(
        dataset: &ProcessedDataset,
        control: &mut ExecutionContext<'_>,
    ) -> Result<Self, PlotError> {
        let mut axes = Vec::new();
        axes.try_reserve_exact(dataset.descriptor().axes().len())
            .map_err(|_| PlotError::AllocationFailure)?;
        for axis in dataset.descriptor().axes() {
            let (coordinates, source_quality) = match axis.coordinates() {
                AxisCoordinates::Uniform { start, step } => {
                    let mut values = Vec::new();
                    values
                        .try_reserve_exact(axis.points())
                        .map_err(|_| PlotError::AllocationFailure)?;
                    for index in 0..axis.points() {
                        if index % 4096 == 0 {
                            control.advance((axis.points() - index).min(4096) as u128)?;
                        }
                        let value = (*step).mul_add(index as f64, *start);
                        if !value.is_finite() {
                            return Err(PlotError::InvalidCoordinate);
                        }
                        values.push(value);
                    }
                    (
                        PlotCoordinates::Physical {
                            unit: axis.unit(),
                            values,
                        },
                        CoordinateSourceQuality::Established,
                    )
                }
                AxisCoordinates::Explicit(values) => (
                    PlotCoordinates::Physical {
                        unit: axis.unit(),
                        values: copy_plot_samples(values, control)?,
                    },
                    CoordinateSourceQuality::Established,
                ),
                AxisCoordinates::Unknown if axis.role() == AxisRole::ArrayParameter => {
                    let mut values = Vec::new();
                    values
                        .try_reserve_exact(axis.points())
                        .map_err(|_| PlotError::AllocationFailure)?;
                    for value in 0..axis.points() {
                        if value % 4096 == 0 {
                            control.check_cancelled()?;
                        }
                        values.push(u64::try_from(value).map_err(|_| PlotError::SizeOverflow)?);
                    }
                    (
                        PlotCoordinates::LogicalIndex(values),
                        CoordinateSourceQuality::Unknown,
                    )
                }
                AxisCoordinates::Unknown => return Err(PlotError::MissingPhysicalCoordinates),
            };
            let direction = match &coordinates {
                PlotCoordinates::LogicalIndex(values) if values.len() > 1 => {
                    AxisDirection::Ascending
                }
                PlotCoordinates::Physical { values, .. } if values.len() > 1 => {
                    let ascending = values.windows(2).all(|pair| pair[0] < pair[1]);
                    let descending = values.windows(2).all(|pair| pair[0] > pair[1]);
                    match (ascending, descending) {
                        (true, false) => AxisDirection::Ascending,
                        (false, true) => AxisDirection::Descending,
                        _ => AxisDirection::Unknown,
                    }
                }
                _ => AxisDirection::Unknown,
            };
            axes.push(PlotAxis {
                label: axis.label().map(str::to_owned),
                role: axis.role(),
                quantity: axis.quantity(),
                coordinates,
                direction,
                source_quality,
            });
        }
        Ok(Self {
            shape: dataset.data().shape().to_vec(),
            data: copy_plot_samples(dataset.data().samples(), control)?,
            axes,
            provenance: dataset.provenance().clone(),
        })
    }

    /// Returns the logical row-major shape.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// Returns scalar samples in row-major order.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Returns axes ordered slowest to fastest.
    pub fn axes(&self) -> &[PlotAxis] {
        &self.axes
    }

    /// Returns the authoritative processing provenance.
    pub fn provenance(&self) -> &ProcessedProvenance {
        &self.provenance
    }
}

fn provenance_bytes(value: &ProcessedProvenance) -> Result<usize, PlotError> {
    let count = || -> Result<usize, crate::processing::ProcessingError> {
        use crate::processing::prepare::memory;
        memory::sum([
            memory::origin(value.origin())?,
            memory::sources(value.sources())?,
            memory::read_record(value.read_record())?,
            memory::source_metadata(value.source_metadata())?,
            value.history().map_or(Ok(0), memory::history)?,
        ])
    };
    count().map_err(|_| PlotError::SizeOverflow)
}

/// Failures while creating a checked scalar plot product.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PlotError {
    /// Cooperative cancellation or execution resource failure.
    #[error(transparent)]
    Execution(#[from] ExecutionError),
    /// A resource bound was rejected before plot payload allocation.
    #[error(transparent)]
    LimitExceeded(#[from] LimitExceeded),
    /// The source processed dataset did not pass aggregate validation.
    #[error("processed input is invalid")]
    InvalidProcessedData,
    /// One or more Cartesian or acquisition components remain.
    #[error("plot data requires a final scalar projection")]
    NonScalarData,
    /// A signal axis has no physical calibration.
    #[error("signal plot axis has no physical coordinates")]
    MissingPhysicalCoordinates,
    /// Coordinate materialization produced a non-finite value.
    #[error("plot coordinate is non-finite")]
    InvalidCoordinate,
    /// Coordinate or shape conversion overflowed.
    #[error("plot size computation overflow")]
    SizeOverflow,
    /// A plot allocation failed.
    #[error("plot allocation failed")]
    AllocationFailure,
}

fn copy_plot_samples(
    input: &[f64],
    control: &mut ExecutionContext<'_>,
) -> Result<Vec<f64>, PlotError> {
    control.check_cancelled()?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(input.len())
        .map_err(|_| PlotError::AllocationFailure)?;
    for block in input.chunks(4096) {
        control.advance(block.len() as u128)?;
        result.extend_from_slice(block);
    }
    Ok(result)
}
