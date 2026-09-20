//! Process a dense 2D raw acquisition and export its magnitude spectrum in ppm.
//! The fixed recipe uses 1-Hz exponential windows, standard zero filling,
//! source-evidenced direct delay correction and negative Fourier requests.
use nmr::plot::PlotData;
use nmr::processing::{
    DenseAxisConfig, DensePipeline, DensePipelineOptions, DirectDelayMode, ExpectedPolarity,
    FrequencyFrame, ProcessingOptions, Projection, ReferenceSource, WorkLedger,
};
use nmr::resource::MemoryLimits;
use nmr::{AxisIndex, ReadOptions, ReadPreference};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(path), Some(output), None) = (args.next(), args.next(), args.next()) else {
        return Err("usage: cargo run --example process_2d -- <raw-path> <output.npz>".into());
    };
    let input = ReadOptions::new()
        .preference(ReadPreference::PreferRaw)
        .allow_experimental_vendor_semantics(true)
        .read(path)?;
    let pipeline = DensePipeline::new(
        (0..2)
            .map(|axis| DenseAxisConfig {
                axis: AxisIndex::new(axis),
                phase: None,
                frequency_frame: Some(FrequencyFrame::Ppm(ReferenceSource::AxisEvidence)),
            })
            .collect(),
        DensePipelineOptions {
            direct_delay: DirectDelayMode::Automatic,
            projection: Projection::Magnitude,
            expected_polarity: ExpectedPolarity::Signed,
            descending_ppm: true,
        },
    )?;
    let result = pipeline.apply(&input, ProcessingOptions::new())?;
    let plot = PlotData::preflight(
        result.as_processed().ok_or("expected spectrum")?,
        MemoryLimits::new(),
    )?
    .execute()?;
    nmr::export::export_npz(&plot, &output, &mut WorkLedger::processing_default())?;
    println!("exported {} magnitude samples", plot.data().len());
    Ok(())
}
