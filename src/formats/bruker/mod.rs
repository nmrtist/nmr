//! Reader for verified Bruker TopSpin raw `fid` and `ser` acquisitions.

mod sample_decode;

mod layout;

mod metadata;

mod parser;

mod reader;

mod semantics;

mod context;

pub(crate) mod parameters;

mod opening;

mod parts;

mod decode_2d;
mod decode_3d;
mod raw_assembly;
mod storage;

#[cfg(test)]
mod tests;

pub(crate) mod processed;

pub(crate) use opening::open_resolved_acquisition_declared;
pub use parameters::ParameterFile;
pub use parameters::Parameters;
pub use parts::Parts;
pub use parts::read_parts;
pub use parts::read_parts_with_limits;
pub(crate) use semantics::ResolvedGroupDelay;
pub(crate) use semantics::require_experimental_nus;
