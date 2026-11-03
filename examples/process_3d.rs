//! Smoke-check explicit dense 3D Bruker processing on one or more acquisitions.
//! This recipe keeps the acquired grid and uses a signed, unphased projection;
//! it is not a quantitative peak or phase reference.
use nmr::processing::{
    DelaySource, DigitalFilterCorrection, FourierExponentSign, FourierTransform, FrequencyFrame,
    PolarityState, ProcessingOperation, ProcessingPlan, Projection, ReferenceSource,
};
use nmr::{ReadOptions, ReadPreference};

#[path = "process_3d/reference.rs"]
mod reference;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = std::env::args_os().skip(1).peekable();
    let reference_path = if paths.peek().is_some_and(|arg| arg == "--nmrpipe-rows") {
        paths.next();
        Some(paths.next().ok_or("expected NMRPipe row stream path")?)
    } else {
        None
    };
    if paths.peek().is_none() {
        return Err(
            "usage: cargo run --example process_3d -- [--nmrpipe-rows <file>] <raw-path> ..."
                .into(),
        );
    }
    let plan = ProcessingPlan::new(recipe())?;
    for path in paths {
        let input = ReadOptions::new()
            .preference(ReadPreference::PreferRaw)
            .read(&path)?;
        let raw = input.as_raw().ok_or("expected a raw acquisition")?;
        if raw.data().shape().len() != 3 {
            return Err(format!("{path:?}: expected three acquisition axes").into());
        }
        let spectrum = plan.apply_raw(raw)?;
        println!("{path:?}: {:?}", spectrum.descriptor().logical_shape());
        for axis in spectrum.descriptor().axes() {
            println!(
                "  {} {}: {:.6} to {:.6} ppm",
                axis.label().unwrap_or("?"),
                axis.nucleus().unwrap_or("?"),
                axis.coordinate(0)?,
                axis.coordinate(axis.points() - 1)?,
            );
        }
        if let Some(reference_path) = &reference_path {
            reference::compare(&spectrum, reference_path.as_ref())?;
        }
    }
    Ok(())
}

fn recipe() -> Vec<ProcessingOperation> {
    let mut operations = vec![
        ProcessingOperation::ComponentTransform { axis: 0 },
        ProcessingOperation::ComponentTransform { axis: 1 },
    ];
    for axis in (0..3).rev() {
        operations.push(ProcessingOperation::FourierTransform {
            axis,
            transform: FourierTransform::new(FourierExponentSign::Negative),
        });
        if axis == 2 {
            operations.push(ProcessingOperation::DigitalFilterCorrection {
                axis,
                correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(
                    DelaySource::AxisEvidence,
                ),
            });
        }
        operations.push(ProcessingOperation::ResolveFrequencyFrame {
            axis,
            frame: FrequencyFrame::Ppm(ReferenceSource::AxisEvidence),
        });
    }
    operations.push(ProcessingOperation::Projection {
        projection: Projection::Real,
        polarity: PolarityState::Ambiguous180,
    });
    operations
}
