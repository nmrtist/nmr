//! Read a 1D raw acquisition and export a magnitude spectrum.
//! The window, zero filling and Fourier sign are explicit demonstration choices.

use nmr::plot::PlotData;
use nmr::processing::{
    FourierExponentSign, FourierTransform, PolarityState, ProcessingOperation, ProcessingOptions,
    ProcessingPlan, Projection, Window, WorkLedger,
};
use nmr::resource::MemoryLimits;
use nmr::{ReadOptions, ReadPreference};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let (Some(raw_path), Some(output), None) = (args.next(), args.next(), args.next()) else {
        return Err("usage: cargo run --example process_1d -- <raw-path> <output.npz>".into());
    };
    let input = ReadOptions::new()
        .preference(ReadPreference::PreferRaw)
        .read(raw_path)?;
    let raw = input.as_raw().ok_or("expected a raw acquisition")?;
    if raw.descriptor().axes().len() != 1 {
        return Err("expected a one-dimensional raw acquisition".into());
    }

    let plan = ProcessingPlan::new(vec![
        ProcessingOperation::Window {
            axis: 0,
            window: Window::exponential(1.0)?,
        },
        ProcessingOperation::StandardZeroFill { axis: 0 },
        ProcessingOperation::FourierTransform {
            axis: 0,
            transform: FourierTransform::new(FourierExponentSign::Negative),
        },
        ProcessingOperation::Projection {
            projection: Projection::Magnitude,
            polarity: PolarityState::Ambiguous180,
        },
    ])?;
    let prepared = plan.preflight(&input, ProcessingOptions::new())?;
    println!("resource bound: {:?}", prepared.resources());
    let spectrum = prepared.execute()?;
    let plot = PlotData::preflight(
        spectrum
            .as_processed()
            .ok_or("expected a processed spectrum")?,
        MemoryLimits::new(),
    )?
    .execute()?;
    nmr::export::export_npz(&plot, &output, &mut WorkLedger::processing_default())?;
    println!(
        "exported {} magnitude samples to {:?}",
        plot.data().len(),
        output
    );
    Ok(())
}
