//! SharedComplex is one complex channel, with no invented F1 quadrature lane.
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processed::{ComponentBasis, ProcessedOrigin};
use nmr::processing::{ProcessingOperation as Op, *};
use nmr::raw::*;
use nmr::{Complex64, Dataset};

fn inputs(conjugated: bool) -> (Dataset, Dataset, Vec<usize>) {
    let (n, f, m) = (128, 32, 32);
    let axis = |kind, points| {
        RawAxis::new(
            kind,
            AxisDomain::Time,
            Some(AxisUnit::Second),
            points,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.001,
            },
        )
        .unwrap()
    };
    let axes = vec![
        axis(
            RawAxisKind::Indirect(IndirectComponents::SharedComplex {
                conjugated,
                evidence: ComponentEvidence::user_constructed(),
            }),
            n,
        ),
        axis(RawAxisKind::Direct(DirectSamples::Complex), f),
    ];
    let samples: Vec<_> = (0..n)
        .flat_map(|i| {
            (0..f).map(move |j| {
                [(1.0, 7.0, 3.0), (0.1, 29.0, -7.0)]
                    .into_iter()
                    .map(|(a, f1, f2)| {
                        let phase = std::f64::consts::TAU
                            * ((if conjugated { -1.0 } else { 1.0 }) * f1 * i as f64 / n as f64
                                + f2 * j as f64 / f as f64);
                        Complex64::from_polar(a, phase)
                    })
                    .sum()
            })
        })
        .collect();
    let indices: Vec<_> = (0..m).map(|i| i * 73 % n).collect();
    let coords: Vec<_> = indices
        .iter()
        .map(|&i| SamplingCoordinate::new(vec![i]))
        .collect();
    let traces = indices
        .iter()
        .enumerate()
        .map(|(ordinal, &i)| {
            SparseTrace::new(
                ObservationOrdinal::new(ordinal),
                coords[ordinal].clone(),
                samples[i * f..(i + 1) * f].to_vec(),
            )
        })
        .collect();
    let dense = RawDatasetBuilder::new(axes.clone(), RawMetadata::default())
        .unwrap()
        .dense(samples)
        .unwrap()
        .into();
    let sparse = RawDatasetBuilder::new(axes, RawMetadata::default())
        .unwrap()
        .sparse(traces, SamplingSchedule::new(vec![n], coords).unwrap())
        .unwrap()
        .into();
    (dense, sparse, indices)
}
fn fft(axis: usize) -> ProcessingPlan {
    ProcessingPlan::new(vec![Op::FourierTransform {
        axis,
        transform: FourierTransform::default(),
    }])
    .unwrap()
}
#[test]
fn shared_complex_sparse_reconstruction_preserves_bits_sign_peaks_and_snapshot_replay() {
    for conjugated in [false, true] {
        let (dense, sparse, indices) = inputs(conjugated);
        let reference = fft(1).apply(&dense).unwrap();
        let mixed = NusSettings {
            max_iterations: 1000,
            noise_standard_deviation: Some(0.0),
        }
        .prepare(&sparse, fft(1), ProcessingOptions::new())
        .unwrap()
        .execute()
        .unwrap();
        let p = mixed.as_processed().unwrap();
        assert_eq!(p.descriptor().component_counts(), vec![1, 2]);
        assert!(
            matches!(p.descriptor().axes()[0].component_basis(),ComponentBasis::SharedComplex {conjugated:c,..} if *c==conjugated)
        );
        let expected = reference.as_processed().unwrap().data();
        for &row in &indices {
            for col in 0..32 {
                for c in 0..2 {
                    assert_eq!(
                        p.data().get(&[row, col], &[0, c]).unwrap().to_bits(),
                        expected.get(&[row, col], &[0, c]).unwrap().to_bits()
                    );
                }
            }
        }
        let spectrum = fft(0).apply(&mixed).unwrap();
        let oracle = fft(0).apply(&reference).unwrap();
        let actual = spectrum.as_processed().unwrap().data();
        let expected = oracle.as_processed().unwrap().data();
        let mut error = 0.0;
        let mut energy = 0.0;
        for (a, b) in actual.samples().iter().zip(expected.samples()) {
            error += (a - b).powi(2);
            energy += b * b;
        }
        assert!(
            (error / energy).sqrt() < 1e-3,
            "conjugated={conjugated}, error={}",
            (error / energy).sqrt()
        );
        for (a, f1, f2) in [(1.0, 7, 3), (0.1, 29, -7)] {
            let col = (16 + f2) as usize;
            let peak = actual.get(&[64 + f1, col], &[0, 0]).unwrap();
            assert!(
                (peak / (128.0 * 32.0 * a) - 1.0).abs() < 1e-3,
                "peak={peak}"
            );
        }
        let mut bytes = Vec::new();
        nmr::snapshot::write_snapshot(&mixed, &mut bytes, Default::default()).unwrap();
        let restored = nmr::snapshot::read_snapshot(&mut bytes.as_slice(), Default::default())
            .unwrap()
            .restore(nmr::snapshot::AcceptRecordedHistory);
        assert_eq!(restored.canonical_digests(), mixed.canonical_digests());
        let ProcessedOrigin::Library(origin) =
            restored.as_processed().unwrap().provenance().origin()
        else {
            panic!()
        };
        let replay = origin
            .replay(
                &[&sparse],
                ProcessingOptions::new(),
                &mut nmr::ExecutionContext::default(),
            )
            .unwrap();
        assert_eq!(replay.canonical_digests(), mixed.canonical_digests());
    }
}
