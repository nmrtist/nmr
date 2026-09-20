//! Deterministic, bounded PlotData NPZ V1 export.

use crate::axis::{AxisDirection, AxisRole, AxisUnit};

use crate::execution::{ExecutionContext, ExecutionError, ExecutionStage};
use crate::io::ControlledWriter;

use crate::plot::{CoordinateSourceQuality, PlotCoordinates, PlotData};

use crate::processing::contracts::error::ProcessingError;

use crate::resource::WorkLedger;

use std::io::{self, BufWriter, Write};

use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use thiserror::Error;

mod manifest;
pub use manifest::*;

mod api;
pub use api::*;

mod npz;
use npz::*;

mod commit;
use commit::*;

mod error;
pub use error::*;
