//! JEOL Delta JDF reader.

mod decoder;

mod embedded;

mod layout;

mod metadata;

mod parser;

mod reader;

mod semantics;

mod header;

pub(crate) mod parameters;

mod opening;

mod parts;

mod processed;

mod sections;

pub(crate) use header::HEADER_LEN;
pub(crate) use opening::is_dataset;
pub(crate) use opening::is_frequency_domain;
pub(crate) use opening::open_acquisition_declared;
pub use parameters::EmbeddedAxisEvidence;
pub use parameters::EmbeddedAxisKind;
pub use parameters::EmbeddedRecordArea;
pub use parameters::ParameterRecord;
pub use parameters::ParameterValue;
pub use parameters::Parameters;
pub use parameters::RawAxisUnit;
pub use parameters::SampleTransform;
pub use parts::Parts;
pub use parts::read_parts;
pub use parts::read_parts_with_limits;
pub(crate) use processed::read_processed;
pub use processed::read_processed_parts;
pub use processed::read_processed_parts_with_limits;
pub(crate) use semantics::require_experimental_opt_in;

pub(crate) use semantics::experimental_details;
