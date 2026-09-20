//! Portable, versioned execution reports and independent numerical comparison.
//!
//! JSON export is a report, not a deserializer or automatic replay protocol.
//! Preserve source files and the matching source/build artifact alongside it.

#[macro_use]
mod json;
mod api;
mod mapping;
pub use api::*;
mod comparison;
pub use comparison::*;
mod error;
pub use error::ReportError;
#[cfg(test)]
mod encoding_tests;
