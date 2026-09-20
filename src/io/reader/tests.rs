use crate::Complex64;
use crate::ExecutionContext;
use crate::ReadError;
use crate::raw::RawDescriptor;
use crate::raw::RawProvenance;
use crate::raw::Region;
use crate::raw::SamplingSchedule;
use crate::raw::SparseTrace;

use super::TraceSource;

use super::*;
use crate::SamplingCoordinate;
use crate::raw::{DirectSamples, IndirectComponents, RawAxisKind, RawFormat};
use crate::{AccessError, AcquisitionMetadata, Axis, AxisCoordinates, Domain, VendorMetadata};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingSource {
    trace_reads: Arc<AtomicUsize>,
    direct_points: usize,
}

impl TraceSource for CountingSource {
    fn trace_numeric_bytes(&self) -> Result<usize, ReadError> {
        self.direct_points
            .checked_mul(std::mem::size_of::<Complex64>())
            .ok_or(ReadError::SizeOverflow)
    }

    fn read_trace_controlled(
        &self,
        _control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        self.trace_reads.fetch_add(1, Ordering::Relaxed);
        Ok(vec![
            Complex64::new(coordinate[0] as f64, 0.0);
            self.direct_points
        ])
    }
}

struct SparseSource;

impl TraceSource for SparseSource {
    fn trace_numeric_bytes(&self) -> Result<usize, ReadError> {
        Ok(4 * std::mem::size_of::<Complex64>())
    }

    fn read_trace_controlled(
        &self,
        _control: &mut ExecutionContext<'_>,
        coordinate: &[usize],
    ) -> Result<Vec<Complex64>, ReadError> {
        if coordinate == [0] {
            Ok(vec![Complex64::new(1.0, 0.0); 4])
        } else {
            Err(AccessError::UnsampledCoordinate {
                coordinate: coordinate.to_vec(),
            }
            .into())
        }
    }
}

fn test_descriptor(format: RawFormat, indirect_points: usize) -> RawDescriptor {
    let axes = vec![
        Axis::new(
            RawAxisKind::Indirect(IndirectComponents::Scalar),
            Domain::Unknown,
            None,
            indirect_points,
            AxisCoordinates::Unknown,
        )
        .unwrap(),
        Axis::new(
            RawAxisKind::Direct(DirectSamples::Real),
            Domain::Unknown,
            None,
            4,
            AxisCoordinates::Unknown,
        )
        .unwrap(),
    ];
    let _ = format;
    RawDescriptor::new(axes, AcquisitionMetadata::default()).unwrap()
}

#[test]
fn dense_region_requests_only_intersecting_traces() {
    let descriptor = test_descriptor(RawFormat::BrukerRaw, 5);
    let trace_reads = Arc::new(AtomicUsize::new(0));
    let opened = Reader::new(
        descriptor,
        RawProvenance::reader(RawFormat::BrukerRaw, Vec::new(), VendorMetadata::none()),
        None,
        Box::new(CountingSource {
            trace_reads: Arc::clone(&trace_reads),
            direct_points: 4,
        }),
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );

    let region = Region::new([1, 1], [2, 2]).unwrap();
    let data = opened.read_region(&region, usize::MAX).unwrap();

    assert_eq!(data.data().shape(), &[2, 2]);
    assert_eq!(trace_reads.load(Ordering::Relaxed), 2);
}

#[test]
fn sparse_region_limits_precede_reads_and_preserve_duplicate_ordinals() {
    let trace_reads = Arc::new(AtomicUsize::new(0));
    let opened = Reader::new(
        test_descriptor(RawFormat::BrukerRaw, 5),
        RawProvenance::reader(RawFormat::BrukerRaw, Vec::new(), VendorMetadata::none()),
        Some(
            SamplingSchedule::new(
                vec![5],
                [0, 2, 4, 2]
                    .into_iter()
                    .map(|point| SamplingCoordinate::new(vec![point]))
                    .collect(),
            )
            .unwrap(),
        ),
        Box::new(CountingSource {
            trace_reads: Arc::clone(&trace_reads),
            direct_points: 4,
        }),
        64,
        usize::MAX,
        usize::MAX,
    );
    let region = Region::new([2, 1], [1, 2]).unwrap();
    let error = opened.read_region(&region, 63).unwrap_err();
    assert!(matches!(
        error.reason(),
        crate::raw::ReadErrorReason::LimitExceeded {
            resource: crate::raw::ReadResource::RegionBytes,
            limit: 63,
            required: 64,
        }
    ));
    // A large per-call limit cannot relax the limit fixed at open time.
    let error = opened
        .read_region(&Region::new([0, 0], [5, 4]).unwrap(), usize::MAX)
        .unwrap_err();
    assert!(matches!(
        error.reason(),
        crate::raw::ReadErrorReason::LimitExceeded {
            resource: crate::raw::ReadResource::RegionBytes,
            limit: 64,
            required: 256,
        }
    ));
    assert_eq!(trace_reads.load(Ordering::Relaxed), 0);
    let result = opened.read_region(&region, 64).unwrap();
    let traces = result.data().sparse_traces().unwrap();
    assert_eq!(traces.len(), 2);
    assert_eq!(traces[0].ordinal().get(), 1);
    assert_eq!(traces[1].ordinal().get(), 3);
    for trace in traces {
        assert_eq!(trace.coordinate().as_slice(), &[2]);
        assert_eq!(trace.samples(), &[Complex64::new(2.0, 0.0); 2]);
    }
    assert_eq!(trace_reads.load(Ordering::Relaxed), 2);
}

#[test]
fn numeric_working_limit_includes_retained_output_and_crop_before_reads() {
    for sparse in [false, true] {
        let make_reader = |working, reads: &Arc<AtomicUsize>| {
            Reader::new(
                test_descriptor(RawFormat::BrukerRaw, 2),
                RawProvenance::reader(RawFormat::BrukerRaw, Vec::new(), VendorMetadata::none()),
                sparse.then(|| {
                    SamplingSchedule::new(vec![2], vec![SamplingCoordinate::new(vec![0]); 2])
                        .unwrap()
                }),
                Box::new(CountingSource {
                    trace_reads: Arc::clone(reads),
                    direct_points: 4,
                }),
                usize::MAX,
                usize::MAX,
                working,
            )
        };
        let reads = Arc::new(AtomicUsize::new(0));
        let region = Region::new([0, 1], [2, 2]).unwrap();
        let word = std::mem::size_of::<usize>();
        let sparse_entries = 2 * (std::mem::size_of::<SparseTrace>() + word);
        let region_required = 160
            + if sparse {
                10 * word + sparse_entries
            } else {
                14 * word
            };
        let materialized_required = 192
            + if sparse {
                6 * word + sparse_entries
            } else {
                9 * word
            };
        // Numeric output/decode/crop plus independent container inventory.
        let error = make_reader(region_required - 1, &reads)
            .read_region(&region, usize::MAX)
            .unwrap_err();
        assert!(matches!(
            error.reason(),
            crate::raw::ReadErrorReason::LimitExceeded {
                resource: crate::raw::ReadResource::WorkingBytes,
                limit, required,
            } if *limit == region_required - 1 && *required == region_required
        ));
        assert_eq!(reads.load(Ordering::Relaxed), 0);
        make_reader(region_required, &reads)
            .read_region(&region, usize::MAX)
            .unwrap();
        assert_eq!(reads.load(Ordering::Relaxed), 2);
        reads.store(0, Ordering::Relaxed);
        // Full output (128) and an adapter trace (64) coexist.
        let error = make_reader(materialized_required - 1, &reads)
            .into_dataset()
            .unwrap_err();
        assert!(matches!(
            error.reason(),
            crate::raw::ReadErrorReason::LimitExceeded {
                resource: crate::raw::ReadResource::WorkingBytes,
                limit, required,
            } if *limit == materialized_required - 1 && *required == materialized_required
        ));
        assert_eq!(reads.load(Ordering::Relaxed), 0);
        make_reader(materialized_required, &reads)
            .into_dataset()
            .unwrap();
        assert_eq!(reads.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn retained_metadata_is_charged_to_access_and_subtracted_from_snapshot_budget() {
    struct SnapshotSource(Arc<AtomicUsize>, Arc<AtomicUsize>);
    impl TraceSource for SnapshotSource {
        fn trace_numeric_bytes(&self) -> Result<usize, ReadError> {
            Ok(64)
        }
        fn snapshot_controlled(
            &self,
            _control: &mut ExecutionContext<'_>,
            working: usize,
            _: &VendorMetadata,
        ) -> Result<Option<crate::provenance::SourceDigest>, ReadError> {
            self.1.store(working, Ordering::Relaxed);
            Ok(None)
        }
        fn read_trace_controlled(
            &self,
            _control: &mut ExecutionContext<'_>,
            _: &[usize],
        ) -> Result<Vec<Complex64>, ReadError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(vec![Complex64::new(0.0, 0.0); 4])
        }
    }
    let reads = Arc::new(AtomicUsize::new(0));
    let snapshot_limit = Arc::new(AtomicUsize::new(usize::MAX));
    let reader = |limit| {
        Reader::new(
            test_descriptor(RawFormat::BrukerRaw, 2),
            RawProvenance::reader(RawFormat::BrukerRaw, Vec::new(), VendorMetadata::none()),
            None,
            Box::new(SnapshotSource(
                Arc::clone(&reads),
                Arc::clone(&snapshot_limit),
            )),
            usize::MAX,
            usize::MAX,
            limit,
        )
        .with_retained_bytes(100)
    };
    assert!(reader(163).is_err());
    let word = std::mem::size_of::<usize>();
    let trace_required = 164 + 3 * word;
    assert!(
        reader(trace_required - 1)
            .unwrap()
            .read_trace(&[0])
            .is_err()
    );
    assert_eq!(reads.load(Ordering::Relaxed), 0);
    reader(trace_required).unwrap().read_trace(&[0]).unwrap();
    reads.store(0, Ordering::Relaxed);
    let observation_required = 164 + 4 * word;
    let ordinal = crate::raw::ObservationOrdinal::new(0);
    let error = reader(observation_required - 1)
        .unwrap()
        .read_observation(ordinal)
        .unwrap_err();
    assert!(
        matches!(error.reason(), crate::ReadErrorReason::LimitExceeded {
        resource: crate::ReadResource::WorkingBytes, required, limit,
    } if *required == observation_required && *limit == observation_required - 1)
    );
    assert_eq!(reads.load(Ordering::Relaxed), 0);
    let trace = reader(observation_required)
        .unwrap()
        .read_observation(ordinal)
        .unwrap();
    assert_eq!(trace.observation_ordinal(), Some(ordinal));
    assert_eq!(trace.coordinate(), &[0]);
    reads.store(0, Ordering::Relaxed);
    let region = Region::new([0, 1], [2, 2]).unwrap();
    let region_required = 260 + 14 * word;
    let materialized_required = 292 + 9 * word;
    let error = reader(region_required - 1)
        .unwrap()
        .read_region(&region, usize::MAX)
        .unwrap_err();
    assert!(matches!(
        error.reason(),
        crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes,
            required, limit,
        } if *required == region_required && *limit == region_required - 1
    ));
    assert_eq!(reads.load(Ordering::Relaxed), 0);
    reader(region_required)
        .unwrap()
        .read_region(&region, usize::MAX)
        .unwrap();
    assert_eq!(reads.load(Ordering::Relaxed), 2);
    reads.store(0, Ordering::Relaxed);
    let error = reader(materialized_required - 1)
        .unwrap()
        .into_dataset()
        .unwrap_err();
    assert!(matches!(
        error.reason(),
        crate::ReadErrorReason::LimitExceeded {
            resource: crate::ReadResource::WorkingBytes,
            required, limit,
        } if *required == materialized_required && *limit == materialized_required - 1
    ));
    assert_eq!(reads.load(Ordering::Relaxed), 0);
    assert_eq!(snapshot_limit.load(Ordering::Relaxed), usize::MAX);
    let dataset = reader(materialized_required)
        .unwrap()
        .into_dataset()
        .unwrap();
    assert_eq!(
        snapshot_limit.load(Ordering::Relaxed),
        materialized_required - 100
    );
    assert_eq!(reads.load(Ordering::Relaxed), 2);
    assert_eq!(
        dataset.data().dense_samples().unwrap(),
        &[Complex64::new(0.0, 0.0); 8]
    );
}

#[test]
fn materialization_uses_its_independent_limit() {
    let opened = Reader::new(
        test_descriptor(RawFormat::BrukerRaw, 2),
        RawProvenance::reader(RawFormat::BrukerRaw, Vec::new(), VendorMetadata::none()),
        None,
        Box::new(CountingSource {
            trace_reads: Arc::new(AtomicUsize::new(0)),
            direct_points: 4,
        }),
        usize::MAX,
        127,
        usize::MAX,
    );

    let error = opened.into_dataset().unwrap_err();
    assert!(matches!(
        error.reason(),
        crate::raw::ReadErrorReason::LimitExceeded {
            resource: crate::raw::ReadResource::MaterializedBytes,
            limit: 127,
            required: 128,
        }
    ));
}

#[test]
fn opened_access_errors_keep_vendor_context_and_structured_root_cause() {
    let opened = Reader::new(
        test_descriptor(RawFormat::VarianRaw, 3),
        RawProvenance::reader(RawFormat::VarianRaw, Vec::new(), VendorMetadata::none()),
        Some(SamplingSchedule::new(vec![3], vec![SamplingCoordinate::new(vec![0])]).unwrap()),
        Box::new(SparseSource),
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );

    let error = opened.read_trace(&[]).unwrap_err();
    assert_eq!(
        error.format(),
        Some(crate::Format::Raw(RawFormat::VarianRaw))
    );
    assert_eq!(error.kind(), crate::raw::ReadErrorKind::Invalid);
    assert!(matches!(
        error.reason(),
        crate::read_error::ReadErrorReason::Access(AccessError::TraceRankMismatch {
            expected: 1,
            actual: 0
        })
    ));

    let error = opened.read_trace(&[3]).unwrap_err();
    assert_eq!(error.kind(), crate::raw::ReadErrorKind::Invalid);
    assert!(matches!(
        error.reason(),
        crate::read_error::ReadErrorReason::Access(AccessError::TraceOutOfBounds {
            axis: 0,
            index: 3,
            points: 3
        })
    ));

    let error = opened.read_trace(&[1]).unwrap_err();
    assert_eq!(error.kind(), crate::raw::ReadErrorKind::UnsampledCoordinate);
    assert!(matches!(
        error.reason(),
        crate::read_error::ReadErrorReason::Access(AccessError::UnsampledCoordinate { coordinate })
            if coordinate.as_slice() == [1]
    ));

    let region = Region::new([1, 0], [2, 4]).unwrap();
    let error = opened.read_region(&region, usize::MAX).unwrap_err();
    assert_eq!(error.kind(), crate::raw::ReadErrorKind::UnsampledRegion);
    assert!(matches!(
        error.reason(),
        crate::read_error::ReadErrorReason::Access(AccessError::UnsampledRegion { start, shape })
            if start.as_slice() == [1, 0] && shape.as_slice() == [2, 4]
    ));

    let region = Region::new([0, usize::MAX], [1, 2]).unwrap();
    let error = opened.read_region(&region, usize::MAX).unwrap_err();
    assert_eq!(error.kind(), crate::raw::ReadErrorKind::Invalid);
    assert!(matches!(
        error.reason(),
        crate::read_error::ReadErrorReason::Access(AccessError::RegionOutOfBounds {
            axis: 1,
            start: usize::MAX,
            length: 2,
            points: 4
        })
    ));
}
