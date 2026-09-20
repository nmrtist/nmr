//! Bruker processed spectrum parameters, decoding and calibrated assembly.

mod axis;
mod parameters;
mod reading;
mod storage;

pub(crate) use parameters::Parameters;
pub(crate) use reading::read;
