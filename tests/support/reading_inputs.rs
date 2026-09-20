//! Minimal synthetic inputs for regression tests.
use std::path::Path;
pub const JCAMP: &str = include_str!("../fixtures/jcamp_dx/ppm.dx");
pub const NUSLIST: &str = include_str!("../fixtures/bruker/nus-2d/nuslist");
pub fn bruker_nus(path: &Path) -> std::io::Result<()> {
    for (name, bytes) in [
        (
            "acqus",
            include_bytes!("../fixtures/bruker/nus-2d/acqus").as_slice(),
        ),
        (
            "acqu2s",
            include_bytes!("../fixtures/bruker/nus-2d/acqu2s").as_slice(),
        ),
        (
            "ser",
            include_bytes!("../fixtures/bruker/nus-2d/ser").as_slice(),
        ),
    ] {
        std::fs::write(path.join(name), bytes)?;
    }
    Ok(())
}
pub fn bruker_processed(path: &Path) -> std::io::Result<()> {
    for (name, bytes) in [
        (
            "procs",
            include_bytes!("../fixtures/bruker/processed-1d/pdata/1/procs").as_slice(),
        ),
        (
            "1r",
            include_bytes!("../fixtures/bruker/processed-1d/pdata/1/1r").as_slice(),
        ),
    ] {
        std::fs::write(path.join(name), bytes)?;
    }
    Ok(())
}
pub fn declaration() -> nmr::SamplingDeclaration {
    nmr::SamplingDeclaration::new(
        nmr::raw::AssertionId::try_new("synthetic-sampling-declaration").unwrap(),
        "explicit user transcription of synthetic nuslist",
        vec![4],
        vec![vec![4], vec![2]],
        nmr::SamplingIndexBase::One,
        vec![2],
    )
}
