//! Repository fixture paths independent of the process working directory.

use std::path::{Path, PathBuf};

pub fn fixture(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}
