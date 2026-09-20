//! Read a scalar processed spectrum and export its samples and coordinates.
use nmr::plot::PlotData;
use nmr::processing::WorkLedger;
use nmr::resource::MemoryLimits;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(input), Some(output), None) = (args.next(), args.next(), args.next()) else {
        return Err(
            "usage: cargo run --example read_spectrum -- <spectrum-path> <output.npz>".into(),
        );
    };
    let dataset = nmr::read(input)?;
    let spectrum = dataset.as_processed().ok_or("processed input required")?;
    let plot = PlotData::preflight(spectrum, MemoryLimits::new())?.execute()?;
    println!("shape: {:?}", plot.shape());
    println!("samples: {:?}", plot.data());
    for (index, axis) in plot.axes().iter().enumerate() {
        println!("axis {index}: {:?}", axis.coordinates());
    }
    nmr::export::export_npz(&plot, &output, &mut WorkLedger::processing_default())?;
    println!("exported {} samples to {:?}", plot.data().len(), output);
    Ok(())
}
