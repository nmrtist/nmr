//! Reader for raw `fid` acquisitions produced by Varian systems.

mod decoder;

mod layout;

mod metadata;

mod parser;

mod reader;

mod options;

pub(crate) mod parameters;

mod opening;

mod parts;

mod semantics;

mod ordering;

pub(crate) use opening::open_resolved_acquisition;
pub use options::ReadAssertions;
pub use parameters::BlockHeader;
pub use parameters::FileHeader;
pub use parameters::ParameterRecord;
pub use parameters::ParameterType;
pub use parameters::ParameterValue;
pub use parameters::Parameters;
pub use parts::Parts;
pub use parts::read_parts;
pub use parts::read_parts_with_limits;
pub(crate) use semantics::projection_error;
