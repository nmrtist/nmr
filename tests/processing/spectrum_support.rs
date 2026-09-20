//! Synthetic spectra and sampling inputs for processing tests.
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedDataset, ProcessedDescriptor, ProcessedOrigin,
    ProcessedProvenance,
};
use nmr::processing::{ProcessingOperation as Op, SpectrumOperation as S, *};
use nmr::snapshot::{self, AcceptRecordedHistory, SnapshotLimits};
use nmr::{CancellationToken, Dataset, ExecutionContext};

pub(crate) fn axis(
    n: usize,
    unit: AxisUnit,
    basis: ComponentBasis,
    coordinates: AxisCoordinates,
) -> ProcessedAxis {
    ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(unit),
        n,
        coordinates,
        basis,
    )
    .unwrap()
}
pub(crate) fn spectrum(x: Vec<f64>, z: &[[f64; 2]], unit: AxisUnit) -> Dataset {
    dataset(
        vec![axis(
            x.len(),
            unit,
            ComponentBasis::Cartesian,
            AxisCoordinates::Explicit(x),
        )],
        z.iter().flatten().copied().collect(),
    )
}
pub(crate) fn dataset(axes: Vec<ProcessedAxis>, samples: Vec<f64>) -> Dataset {
    ProcessedDataset::from_dense_samples(
        ProcessedDescriptor::new(axes).unwrap(),
        samples,
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
    .into()
}
pub(crate) fn values(input: &Dataset) -> &[f64] {
    input.as_dense_processed().unwrap().samples()
}
pub(crate) fn apply(input: &Dataset, axis: usize, operation: S) -> Dataset {
    let plan = ProcessingPlan::new(vec![Op::Spectrum { axis, operation }]).unwrap();
    let prepared = plan.preflight(input, ProcessingOptions::new()).unwrap();
    let r = prepared.resources();
    let output = plan
        .preflight(
            input,
            ProcessingOptions::new()
                .max_output_bytes(r.output_bytes())
                .max_working_bytes(r.working_bytes())
                .max_metadata_bytes(r.metadata_bytes()),
        )
        .unwrap()
        .execute()
        .unwrap();
    assert!(
        plan.preflight(input, ProcessingOptions::new().max_metadata_bytes(0))
            .is_err()
    );
    let history = output
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .unwrap();
    if input
        .as_processed()
        .unwrap()
        .provenance()
        .history()
        .is_none()
    {
        let replay = history.replay(&[input], ProcessingOptions::new()).unwrap();
        assert_eq!(replay.canonical_digests(), output.canonical_digests());
    }
    let mut bytes = Vec::new();
    snapshot::write_snapshot(&output, &mut bytes, SnapshotLimits::default()).unwrap();
    let restored = snapshot::read_snapshot(&mut bytes.as_slice(), SnapshotLimits::default())
        .unwrap()
        .restore(AcceptRecordedHistory);
    assert_eq!(restored.canonical_digests(), output.canonical_digests());
    let token = CancellationToken::new();
    token.cancel();
    let mut work = WorkLedger::new(u128::MAX);
    let mut context = ExecutionContext::new(&mut work).with_cancellation(token);
    assert!(
        plan.apply_with_context(input, ProcessingOptions::new(), &mut context)
            .is_err()
    );
    output
}
pub(crate) fn close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (i, (a, b)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (a - b).abs() <= tolerance * (1.0 + b.abs()),
            "{i}: {a} != {b}"
        );
    }
}

pub(crate) fn nus_input(n: usize, indices: &[usize]) -> Dataset {
    use nmr::raw::*;
    let indirect = RawAxis::new(
        RawAxisKind::Indirect(IndirectComponents::Cartesian(
            ComponentEvidence::user_constructed(),
        )),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        n,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.01,
        },
    )
    .unwrap();
    let direct = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        3,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        },
    )
    .unwrap();
    let coordinates: Vec<_> = indices
        .iter()
        .map(|i| SamplingCoordinate::new(vec![*i]))
        .collect();
    let traces = indices
        .iter()
        .enumerate()
        .map(|(ordinal, &i)| {
            let phase = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            let mut samples = Vec::new();
            for lane in [phase.cos(), phase.sin()] {
                for j in 0..3 {
                    samples.push(nmr::Complex64::from_polar(
                        lane,
                        2.0 * std::f64::consts::PI * j as f64 / 3.0,
                    ));
                }
            }
            SparseTrace::new(
                ObservationOrdinal::new(ordinal),
                coordinates[ordinal].clone(),
                samples,
            )
        })
        .collect();
    RawDatasetBuilder::new(vec![indirect, direct], RawMetadata::default())
        .unwrap()
        .sparse(traces, SamplingSchedule::new(vec![n], coordinates).unwrap())
        .unwrap()
        .into()
}
