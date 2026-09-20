//! Format detection and bounded acquisition access.

mod byte_source;
mod file_size;
mod options;
mod reader;

pub(crate) use byte_source::ByteSource;
pub(crate) use file_size::ensure_file_size;
pub use options::ReadLimits;
pub use reader::Reader;
pub(crate) use reader::TraceSource;

mod sized_file;
pub(crate) use sized_file::{read_sized_file, read_sized_file_controlled};

mod controlled_writer;
pub(crate) use controlled_writer::{ControlledWriter, io_control_error};
