//! Self-contained sparse raw -> automatic F2/NUS -> F1 example.
//! Optional argument: a local raw acquisition path (read only).
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processing::{ProcessingOperation as Op, *};
use nmr::raw::*;
use nmr::{Complex64, Dataset};

fn synthetic() -> Result<Dataset, Box<dyn std::error::Error>> {
    let (n, m, f) = (256, 96, 128);
    let axis = |kind, points, step| {
        RawAxis::new(
            kind,
            AxisDomain::Time,
            Some(AxisUnit::Second),
            points,
            AxisCoordinates::Uniform { start: 0.0, step },
        )
    };
    let axes = vec![
        axis(
            RawAxisKind::Indirect(IndirectComponents::Cartesian(
                ComponentEvidence::user_constructed(),
            )),
            n,
            0.001,
        )?,
        axis(RawAxisKind::Direct(DirectSamples::Complex), f, 0.001)?,
    ];
    let indices: Vec<_> = (0..m).map(|i| i * 73 % n).collect();
    let coordinates: Vec<_> = indices
        .iter()
        .map(|&i| SamplingCoordinate::new(vec![i]))
        .collect();
    let mut rng = 7193_u64;
    let mut uniform = || {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    let mut traces = Vec::new();
    for (ordinal, &i) in indices.iter().enumerate() {
        let mut samples = Vec::new();
        for lane in 0..2 {
            for j in 0..f {
                let mut z = Complex64::default();
                for (amplitude, f1, f2) in [(1.0, 17.0, 21.0), (0.05, 67.0, -30.0)] {
                    let angle = std::f64::consts::TAU * f1 * i as f64 / n as f64;
                    z += Complex64::from_polar(
                        amplitude * if lane == 0 { angle.cos() } else { angle.sin() },
                        std::f64::consts::TAU * f2 * j as f64 / f as f64,
                    );
                }
                let radius = 0.01 * (-2.0 * uniform().ln()).sqrt();
                z += Complex64::from_polar(radius, std::f64::consts::TAU * uniform());
                samples.push(z);
            }
        }
        traces.push(SparseTrace::new(
            ObservationOrdinal::new(ordinal),
            coordinates[ordinal].clone(),
            samples,
        ));
    }
    Ok(RawDatasetBuilder::new(axes, RawMetadata::default())?
        .sparse(traces, SamplingSchedule::new(vec![n], coordinates)?)?
        .into())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1);
    let real = path.is_some();
    let input = if let Some(path) = path {
        nmr::ReadOptions::new()
            .preference(nmr::ReadPreference::PreferRaw)
            .allow_experimental_vendor_semantics(true)
            .read(path)?
    } else {
        synthetic()?
    };
    let raw = input.as_raw().ok_or("raw input required")?;
    let mut direct = Vec::new();
    if matches!(
        raw.descriptor().axes()[0].kind(),
        RawAxisKind::Indirect(IndirectComponents::Encoded(_))
    ) {
        direct.push(Op::ComponentTransform { axis: 0 });
    }
    if real {
        direct.push(Op::DigitalFilterCorrection {
            axis: 1,
            correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
                source: DelaySource::AxisEvidence,
                policy: TimeDomainResidualPolicy::CorrectFully,
            },
        });
        direct.push(Op::Window {
            axis: 1,
            window: Window::sine_bell(0.5, 1.0, 1.0, 1.0)?,
        });
    }
    direct.push(Op::FourierTransform {
        axis: 1,
        transform: FourierTransform::default(),
    });
    let start = std::time::Instant::now();
    let prepared = AutoNusSettings::default().prepare(
        &input,
        ProcessingPlan::new(direct)?,
        ProcessingOptions::new(),
    )?;
    println!(
        "shape={:?}, observations={}, work={}",
        prepared.output_descriptor().logical_shape(),
        prepared.measured_indices().len(),
        prepared.estimated_work()
    );
    let mut work = WorkLedger::new(
        prepared
            .estimated_work()
            .checked_add(1_000_000_000)
            .ok_or("work overflow")?,
    );
    let mut context = nmr::ExecutionContext::new(&mut work);
    let analyzed = prepared.analyze_with_context(&mut context)?;
    println!("noise={:?}", analyzed.noise_report());
    if std::env::var_os("NMR_NUS_ANALYZE_ONLY").is_some() {
        return Ok(());
    }
    let mixed = match analyzed.execute_with_context(&mut context) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("reconstruction failed after {:?}: {error}", start.elapsed());
            return Err(error.into());
        }
    };
    if let nmr::processed::ProcessedOrigin::Library(origin) =
        mixed.as_processed().unwrap().provenance().origin()
    {
        println!("build={:?}", origin.environment().build_identifier());
        if let nmr::derivation::DerivationOperation::NusReconstruction {
            noise_report,
            iterations,
            ..
        } = origin.operation()
        {
            println!("noise={noise_report:?}; iterations={iterations}");
        }
    }
    let output = ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])?
    .apply(&mixed)?;
    let view = output.as_dense_processed().ok_or("dense output required")?;
    assert!(view.samples().iter().all(|v| v.is_finite()));
    println!(
        "finite 2D frequency shape={:?}; elapsed={:?}",
        output.as_processed().unwrap().descriptor().logical_shape(),
        start.elapsed()
    );
    Ok(())
}
