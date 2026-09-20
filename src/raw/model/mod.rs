//! Checked raw model, shape arithmetic and sample access.
//! Fields shared with siblings stay private to this model boundary.
use crate::acquisition::{
    ChemicalShiftReference, EvidenceValidationError, GroupDelayState, RawAxisKind,
    TransformValidationError,
};

use crate::axis::{
    AxisCoordinates, AxisDirection, AxisDomain, AxisQuantity, AxisRole, AxisUnit,
    AxisValidationError, FrequencyEvidence, coordinate_span, direction, validate_axis,
    validate_quantity,
};

use crate::provenance::ProvenanceError;

use crate::provenance::SourceFile;

use crate::read_error::{InputSource, ReadError, ReadErrorReason, ReadResource};

use crate::formats::{
    bruker::parameters as bruker, jeol::parameters as jeol, varian::parameters as varian,
};

pub use num_complex::Complex64;

use std::collections::BTreeSet;

use thiserror::Error;

mod metadata;
pub use metadata::*;

mod axis;
pub use axis::*;

mod provenance;
pub use provenance::*;

mod layout;
pub use layout::*;

mod descriptor;
pub use descriptor::*;

mod sampling;
pub use sampling::*;

mod data;
pub use data::*;

mod dataset;
pub use dataset::*;

mod error;
pub use error::*;

mod access;
pub(crate) use access::*;

pub(crate) mod sampling_declaration;
