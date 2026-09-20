//! Checked core and dataset values for processed scientific products.

pub use crate::acquisition::ComponentBasis;

use crate::axis::{
    AxisCoordinates, AxisDirection, AxisDomain, AxisQuantity, AxisRole, AxisUnit,
    AxisValidationError, FrequencyEvidence, coordinate_span, direction, validate_axis,
    validate_quantity,
};

use thiserror::Error;

mod axis;
pub use axis::*;

mod descriptor;
pub use descriptor::*;

mod data;
pub use data::*;

pub(crate) mod origin;
pub use origin::*;

mod access;
use access::*;

mod error;
pub use error::*;
