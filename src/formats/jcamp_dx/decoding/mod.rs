//! JCAMP reading, checked assembly and numeric decoding.

use crate::axis::AxisUnit;

struct Jcamp {
    x_factor: f64,
    y_factor: f64,
    first_x: f64,
    delta_x: f64,
    unit: AxisUnit,
    observe_frequency_mhz: Option<f64>,
    nucleus: Option<String>,
    samples: Vec<f64>,
    labels: Vec<(String, String)>,
}

mod assembly;
mod numeric;
mod parameters;
mod reading;

pub(crate) use reading::{read, read_parts, read_parts_with_limits};

pub(crate) use numeric::Encoded;
