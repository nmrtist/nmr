//! Checked, vendor-neutral raw and processed NMR data models.
//!
//! [`read`] detects and materializes a raw acquisition or processed spectrum.
//! [`raw::open`] is the lazy, raw-only reader for trace and region access. Readers do
//! not apply FFT, filtering, phasing, scan normalization, interpolation, or
//! NUS reconstruction.
//!
//! Public complex samples use `R + iI`. Readers apply a vendor-to-public sign
//! transform only when supported by format evidence. Known nucleus labels use
//! mass-number-first ASCII notation such as `1H` and `13C`; original vendor text
//! remains available through vendor/source metadata.
//!
//! Detection identifies and selects a candidate format. It does not promise that
//! the selected scientific layout is supported by [`read`] or [`raw::open`].
//!
//! # Experimental algorithms
//!
//! The supported scientific foundation is checked data models, evidence-aware
//! reading and explicit deterministic processing. ACME, AsLS, the fixed 512/128
//! IST kernel and `DensePipeline` automatic policy have a separate experimental
//! scope: successful execution is not a general inference-quality guarantee.
//! Their accepted data domains, diagnostics and future replacement profiles are
//! documented independently of the stable data and explicit-operation contracts.
//! This project has not been released. APIs, algorithms and schemas are updated
//! in place during development, retaining their initial version identifiers.
//! Development snapshots have no backward-compatibility or migration guarantee.
//!
//! Closed mathematical sets (raw/processed kind and positive/negative Fourier
//! exponent) remain exhaustive. Extensible format/error/result classifications
//! are non-exhaustive. Constructible request variants retain their listed fields;
//! new contracts use new variants or checked parameter objects. Read-only result
//! variants may acquire facts and require `..` in external matches.
//!
//! Algorithm quality and bitwise numerical reproducibility are separate from
//! interface compatibility. Histories record the current algorithm identifiers
//! and execution environment. Replay success does not compare output samples;
//! numerical comparison is caller-owned.
//!
//! ```no_run
//! let loaded = nmr::read("data/experiment")?;
//! println!("{:?}", loaded.source_format());
//! # Ok::<(), nmr::ReadError>(())
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod acquisition;
mod canonical_digest;
mod internal;
pub use raw::model::sampling_declaration::{SamplingDeclaration, SamplingIndexBase};
mod read_error;
// Frozen codecs retain their original internal family path.
use read_error as raw_error;
mod reading;

pub mod axis;
pub mod data;
pub mod dataset;
pub mod derivation;
pub mod execution;
pub mod execution_report;
pub mod export;
pub mod external;
pub mod formats;
pub mod plot;
pub mod processed;
pub mod processing;
pub mod provenance;
pub mod raw;
// Frozen report and snapshot codecs retain their original internal family path.
use processing::contracts::polarity as reference;
pub mod resource;
pub mod snapshot;

mod io;

pub use acquisition::AxisIndex;
pub use dataset::{Dataset, ProcessedDatasetWithContext, RawDatasetWithContext};
pub use execution::{CancellationToken, ExecutionContext};
pub use io::ReadLimits;
pub use num_complex::Complex64;
pub use read_error::{
    InputSource, ParameterError, ParameterErrorKind, ReadCandidate, ReadError, ReadErrorKind,
    ReadErrorReason, ReadResource,
};
pub use reading::api::{
    DatasetIdentity, DatasetKind, Format, FormatFamily, MetadataField, ReadOptions, ReadPreference,
    ReadWarning, WarningImpact, detect, read, read_with_context,
};

pub(crate) use raw::model::AccessError;

pub(crate) use axis::{
    AxisCoordinates, AxisDomain as Domain, AxisQuantity, AxisRole, AxisUnit, FrequencyEvidence,
};
pub(crate) use provenance::SourceFile;
pub(crate) use raw::model::{
    DiffusionAcquisition, RawAxis as Axis, RawData as AcquisitionData, RawDataset as Acquisition,
    RawDescriptor as AcquisitionDescriptor, RawMetadata as AcquisitionMetadata, SamplingCoordinate,
    SamplingSchedule, SparseTrace, VendorMetadata,
};

pub(crate) use io::ensure_file_size;
pub(crate) use raw::model::checked_product;
