//! Every executable request participates in preparation, limits, execution and replay.
use crate::Complex64;
use crate::acquisition::{DirectSamples, GroupDelayState, RawAxisKind};
use crate::axis::{AxisCoordinates, AxisRole};
use crate::execution::ExecutionContext;
use crate::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use crate::processing::contracts::polarity::PolarityState;
use crate::processing::contracts::{error::*, operation::*, options::*, profile::*};
use crate::processing::prepare::plan::*;
use crate::resource::WorkLedger;

use crate::axis::{AxisDomain, AxisUnit};
use crate::raw::{PendingGroupDelay, RawAxis, RawDatasetBuilder, RawMetadata};

// Exhaustive matches force new request kinds and digital-filter profiles to name
// their acceptance case. The accompanying operation-contracts ADR maps kernels.
fn contract(operation: &ProcessingOperation) -> usize {
    match operation {
        ProcessingOperation::Spectrum { .. } => 13,
        ProcessingOperation::Window { .. } => 0,
        ProcessingOperation::ZeroFill { .. } => 1,
        ProcessingOperation::StandardZeroFill { .. } => 2,
        ProcessingOperation::FourierTransform { .. } => 3,
        ProcessingOperation::DigitalFilterCorrection { correction, .. } => match correction {
            DigitalFilterCorrection::AcknowledgeZeroDelayV1 => 4,
            DigitalFilterCorrection::FrequencyDomainPhaseRampV1(_) => 5,
            DigitalFilterCorrection::TimeDomainShiftFoldV1 { .. } => 6,
        },
        ProcessingOperation::PhaseCorrection { .. } => 7,
        ProcessingOperation::BaselineCorrection { .. } => 8,
        ProcessingOperation::ComponentTransform { .. } => 9,
        ProcessingOperation::Projection { .. } => 10,
        ProcessingOperation::ResolveFrequencyFrame { .. } => 11,
        ProcessingOperation::ReverseAxis { .. } => 12,
    }
}

// No wildcard: changes to any spectrum method must extend this inventory.
fn spectrum_contract(operation: &SpectrumOperation) -> usize {
    match operation {
        SpectrumOperation::RetainRange { .. } => 0,
        SpectrumOperation::Reference { .. } => 1,
        SpectrumOperation::Reverse => 2,
        SpectrumOperation::Invert => 3,
        SpectrumOperation::Affine { .. } => 4,
        SpectrumOperation::MovingAverage { .. } => 5,
        SpectrumOperation::SavitzkyGolay { .. } => 6,
        SpectrumOperation::Baseline(method) => match method {
            RealBaseline::Offset => 7,
            RealBaseline::Polynomial { .. } => 8,
            RealBaseline::Asls { .. } => 9,
        },
        SpectrumOperation::Normalize(mode) => match mode {
            Normalization::MaxPeak => 10,
            Normalization::TotalArea { .. } => 11,
            Normalization::Constant(_) => 12,
        },
        SpectrumOperation::Bin { aggregation, .. } => match aggregation {
            BinAggregation::Sum => 13,
            BinAggregation::Mean => 14,
        },
        SpectrumOperation::Magnitude => 15,
        SpectrumOperation::Slice { .. } => 16,
        SpectrumOperation::Sum { .. } => 17,
        SpectrumOperation::Skyline { .. } => 18,
    }
}

#[test]
fn every_spectrum_method_has_preflight_history_budget_and_snapshot_registration() {
    use SpectrumOperation as S;
    let axis = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Ppm),
        5,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 1.0,
        },
        ComponentBasis::Cartesian,
    )
    .unwrap();
    let input: crate::Dataset = ProcessedDataset::from_dense_samples(
        ProcessedDescriptor::new(vec![axis.clone(), axis]).unwrap(),
        vec![1.0; 100],
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap()
    .into();
    let operations = [
        S::RetainRange { start: 1, end: 4 },
        S::Reference { delta_ppm: 1.0 },
        S::Reverse,
        S::Invert,
        S::Affine {
            scale: 2.0,
            real_offset: 3.0,
        },
        S::MovingAverage { window: 3 },
        S::SavitzkyGolay {
            window: 3,
            order: 2,
        },
        S::Baseline(RealBaseline::Offset),
        S::Baseline(RealBaseline::Polynomial { order: 2 }),
        S::Baseline(RealBaseline::Asls {
            lambda: 50000.0,
            asymmetry: 0.001,
            iterations: 20,
        }),
        S::Normalize(Normalization::MaxPeak),
        S::Normalize(Normalization::TotalArea {
            singleton_width: None,
        }),
        S::Normalize(Normalization::Constant(-2.0)),
        S::Bin {
            width: 2.0,
            aggregation: BinAggregation::Sum,
        },
        S::Bin {
            width: 2.0,
            aggregation: BinAggregation::Mean,
        },
        S::Magnitude,
        S::Slice {
            index: 2,
            component: 1,
        },
        S::Sum { component: 0 },
        S::Skyline { component: 0 },
    ];
    for (index, operation) in operations.into_iter().enumerate() {
        assert_eq!(spectrum_contract(&operation), index);
        let plan = ProcessingPlan::new(vec![ProcessingOperation::Spectrum { axis: 0, operation }])
            .unwrap();
        let prepared = plan.preflight(&input, ProcessingOptions::new()).unwrap();
        let r = prepared.resources();
        let options = ProcessingOptions::new()
            .max_output_bytes(r.output_bytes())
            .max_metadata_bytes(r.metadata_bytes())
            .max_working_bytes(r.working_bytes());
        let mut work = WorkLedger::new(prepared.estimated_work().unwrap());
        let output = plan
            .preflight(&input, options)
            .unwrap()
            .execute_with_context(&mut ExecutionContext::new(&mut work))
            .unwrap();
        let history = output
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        assert_eq!(
            prepared.steps().next().unwrap().resolved(),
            history.records()[0].resolved().unwrap()
        );
        assert_eq!(
            history
                .replay(&[&input], ProcessingOptions::new())
                .unwrap()
                .canonical_digests(),
            output.canonical_digests()
        );
        let mut bytes = vec![];
        crate::snapshot::write_snapshot(&output, &mut bytes, Default::default()).unwrap();
        assert_eq!(
            crate::snapshot::read_snapshot(&mut bytes.as_slice(), Default::default())
                .unwrap()
                .restore(crate::snapshot::AcceptRecordedHistory)
                .canonical_digests(),
            output.canonical_digests()
        );
    }
}

fn raw(delay: f64) -> crate::Dataset {
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        8,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        },
    )
    .unwrap()
    .with_group_delay(GroupDelayState::Pending(
        PendingGroupDelay::user_constructed(delay).unwrap(),
    ))
    .unwrap();
    let mut samples = vec![Complex64::default(); 8];
    samples[0] = Complex64::new(1.0, 0.0);
    RawDatasetBuilder::new(vec![axis], RawMetadata::default())
        .unwrap()
        .dense(samples)
        .unwrap()
        .into()
}

#[test]
fn every_explicit_operation_has_prepare_resource_execution_and_replay_coverage() {
    let fft = ProcessingOperation::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    };
    let phase = PhaseCorrection::zero_order_degrees(0.0).unwrap();
    let parameters = "##TITLE=contract\n##$TD=16\n##$PARMODE=0\n##$AQ_mod=3\n##$BYTORDA=0\n##$DTYPA=0\n##$SW_h=1000\n##$SFO1=400\n##$BF1=400\n##$GRPDLY=0.25\n##END=\n";
    let bytes = (0..16i32).flat_map(i32::to_le_bytes).collect::<Vec<_>>();
    let bruker = crate::formats::bruker::read_parts(crate::formats::bruker::Parts::new(
        &bytes,
        &[parameters],
    ))
    .unwrap();
    use crate::acquisition::{
        LinearComponentTransform, PeriodicLaneModulation, ResolvedComponentTransform,
    };
    let one = Complex64::new(1.0, 0.0);
    let zero = Complex64::default();
    let transform = ResolvedComponentTransform::user_constructed(
        LinearComponentTransform::try_new(
            2,
            vec![one, zero, zero, one],
            PeriodicLaneModulation::identity(2).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let encoded = ProcessedDataset::new(
        ProcessedDescriptor::new(vec![
            ProcessedAxis::new(
                AxisRole::IndirectAcquisition,
                AxisDomain::Time,
                Some(AxisUnit::Second),
                2,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 0.001,
                },
                ComponentBasis::Encoded(transform),
            )
            .unwrap(),
            ProcessedAxis::new(
                AxisRole::Signal,
                AxisDomain::Frequency,
                Some(AxisUnit::Hertz),
                2,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 1.0,
                },
                ComponentBasis::Cartesian,
            )
            .unwrap(),
        ])
        .unwrap(),
        ProcessedData::new(
            vec![2, 2],
            vec![2, 2],
            (0..16).map(|value| value as f64).collect(),
        )
        .unwrap(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let cases = [
        (
            raw(0.0),
            vec![
                ProcessingOperation::Window {
                    axis: 0,
                    window: Window::exponential(0.0).unwrap(),
                },
                ProcessingOperation::ZeroFill {
                    axis: 0,
                    zero_fill: ZeroFill::new(8).unwrap(),
                },
                ProcessingOperation::StandardZeroFill { axis: 0 },
                fft.clone(),
                ProcessingOperation::DigitalFilterCorrection {
                    axis: 0,
                    correction: DigitalFilterCorrection::AcknowledgeZeroDelayV1,
                },
                ProcessingOperation::PhaseCorrection {
                    axis: 0,
                    correction: phase,
                },
                ProcessingOperation::Projection {
                    projection: Projection::RealAbsorptive,
                    polarity: PolarityState::UserAssertedPositive,
                },
                ProcessingOperation::BaselineCorrection {
                    axis: 0,
                    profile: PositivePeaksV1.into(),
                },
                ProcessingOperation::ResolveFrequencyFrame {
                    axis: 0,
                    frame: FrequencyFrame::Hertz,
                },
                ProcessingOperation::ReverseAxis { axis: 0 },
                ProcessingOperation::Spectrum {
                    axis: 0,
                    operation: SpectrumOperation::Invert,
                },
            ],
        ),
        (
            raw(0.75),
            vec![
                fft,
                ProcessingOperation::DigitalFilterCorrection {
                    axis: 0,
                    correction: DigitalFilterCorrection::FrequencyDomainPhaseRampV1(
                        DelaySource::AxisEvidence,
                    ),
                },
            ],
        ),
        (
            bruker.into(),
            vec![ProcessingOperation::DigitalFilterCorrection {
                axis: 0,
                correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
                    source: DelaySource::AxisEvidence,
                    policy: TimeDomainResidualPolicy::IntegerOnlyRetainResidual,
                },
            }],
        ),
        (
            encoded.into(),
            vec![ProcessingOperation::ComponentTransform { axis: 0 }],
        ),
    ];
    let mut covered = [false; 14];
    for (input, operations) in cases {
        for operation in &operations {
            covered[contract(operation)] = true;
        }
        let plan = ProcessingPlan::new(operations).unwrap();
        assert!(matches!(
            plan.preflight(&input, ProcessingOptions::new().max_metadata_bytes(0))
                .map_err(ProcessingError::into_root_cause),
            Err(ProcessingError::LimitExceeded(_))
        ));
        let prepared = plan.preflight(&input, ProcessingOptions::new()).unwrap();
        let resources = prepared.resources();
        let preview: Vec<_> = prepared
            .steps()
            .map(|step| {
                (
                    step.requested().clone(),
                    step.resolved().clone(),
                    step.algorithm_version(),
                    step.input_descriptor().clone(),
                    step.output_descriptor().clone(),
                )
            })
            .collect();
        let limits = crate::resource::MemoryLimits::new()
            .max_output_bytes(resources.output_bytes())
            .max_metadata_bytes(resources.metadata_bytes())
            .max_working_bytes(resources.working_bytes());
        let output = plan
            .preflight(&input, ProcessingOptions::new().memory(limits))
            .unwrap()
            .execute()
            .unwrap();
        let history = output
            .as_processed()
            .unwrap()
            .provenance()
            .history()
            .unwrap();
        assert_eq!(preview.len(), history.records().len());
        for ((requested, resolved, version, before, after), record) in
            preview.iter().zip(history.records())
        {
            assert_eq!(record.requested().explicit(), Some(requested));
            assert_eq!(record.resolved(), Some(resolved));
            assert_eq!(record.algorithm_version(), Some(*version));
            assert_eq!(record.descriptor_transition(), Some((before, after)));
        }
        let replayed = history.replay(&[&input], ProcessingOptions::new()).unwrap();
        assert_eq!(output.canonical_digests(), replayed.canonical_digests());
        assert_eq!(output.as_dense_processed(), replayed.as_dense_processed());
    }
    assert!(covered.into_iter().all(|covered| covered));
}
