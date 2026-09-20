//! Dataset structure, coordinates, ownership and access.

#[allow(dead_code)] // This shared builder module serves several independent test targets.
#[path = "../support/datasets.rs"]
mod datasets;

mod access;
mod chemical_shift_reference;
mod components;
mod coordinates;
mod data_blocks;
mod identity;
mod ownership;
mod processed;
mod properties;
mod raw;
mod raw_validation;
mod support;

#[path = "../support/fixture_paths.rs"]
mod fixture_paths;
