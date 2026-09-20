use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedDescriptor, ProcessedValidationError,
};
use nmr::raw::{
    DirectSamples, IndirectComponents, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata,
};

use super::support::*;

#[test]
fn processed_component_counts_are_derived_from_basis_without_algorithm_rank_limits() {
    let scalar = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Hertz),
        2,
        AxisCoordinates::Uniform {
            start: -1.0,
            step: 1.0,
        },
        ComponentBasis::Scalar,
    )
    .unwrap();
    let cartesian = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Unknown,
        ComponentBasis::Cartesian,
    )
    .unwrap();
    assert_eq!(scalar.component_count(), 1);
    assert_eq!(cartesian.component_count(), 2);
    assert_eq!(
        ProcessedDescriptor::new(vec![scalar.clone(), scalar.clone(), scalar])
            .unwrap()
            .logical_shape(),
        [2, 2, 2]
    );
}

#[test]
fn processed_nd_tensors_preserve_component_addressing_but_processing_rejects_rank() {
    use nmr::processed::{ProcessedData, ProcessedDataset, ProcessedOrigin, ProcessedProvenance};
    use nmr::processing::{
        FourierTransform, ProcessingError, ProcessingOperation, ProcessingOptions, ProcessingPlan,
    };
    for (shape, components, expected_plane, logical, component, expected_value) in [
        (
            vec![2, 3, 2],
            vec![2, 1, 2],
            vec![13., 15., 17., 19., 21., 23., 37., 39., 41., 43., 45., 47.],
            vec![1, 2, 1],
            vec![1, 0, 1],
            47.,
        ),
        (
            vec![2, 2, 2, 2],
            vec![2, 2, 2, 2],
            vec![
                0., 2., 8., 10., 32., 34., 40., 42., 128., 130., 136., 138., 160., 162., 168., 170.,
            ],
            vec![1, 0, 1, 0],
            vec![0, 1, 1, 0],
            156.,
        ),
    ] {
        let axes = shape
            .iter()
            .zip(&components)
            .map(|(&points, &count)| {
                processed_time_axis(
                    AxisRole::Signal,
                    points,
                    if count == 1 {
                        ComponentBasis::Scalar
                    } else {
                        ComponentBasis::Cartesian
                    },
                )
            })
            .collect();
        let count: usize = shape.iter().zip(&components).map(|(n, c)| n * c).product();
        let descriptor = ProcessedDescriptor::new(axes).unwrap();
        let data = ProcessedData::new(
            shape.clone(),
            components,
            (0..count).map(|i| i as f64).collect(),
        )
        .unwrap();
        assert_eq!(data.get(&logical, &component).unwrap(), expected_value);
        let plane = if shape.len() == 3 {
            vec![1, 0, 1]
        } else {
            vec![0; 4]
        };
        assert_eq!(
            data.component_plane(&plane)
                .unwrap()
                .copied()
                .collect::<Vec<_>>(),
            expected_plane
        );
        let dataset = ProcessedDataset::new(
            descriptor,
            data,
            ProcessedProvenance::new(ProcessedOrigin::Unknown, Vec::new()).unwrap(),
        )
        .unwrap();
        dataset.validate().unwrap();
        for operation in [
            ProcessingOperation::FourierTransform {
                axis: shape.len() - 1,
                transform: FourierTransform::default(),
            },
            ProcessingOperation::Window {
                axis: 0,
                window: nmr::processing::Window::exponential(1.0).unwrap(),
            },
        ] {
            let plan = ProcessingPlan::new(vec![operation]).unwrap();
            // The capability error precedes numerical allocation and output limits.
            assert_eq!(
                plan.apply_processed_with_options(
                    &dataset,
                    ProcessingOptions::new()
                        .max_output_bytes(0)
                        .max_working_bytes(0)
                )
                .unwrap_err()
                .into_root_cause(),
                ProcessingError::UnsupportedRank { rank: shape.len() }
            );
        }
    }
}

#[test]
fn processed_nd_dimensions_and_storage_arithmetic_remain_checked() {
    use nmr::processed::ProcessedData;
    assert_eq!(
        ProcessedDescriptor::new(Vec::new()).unwrap_err(),
        ProcessedValidationError::EmptyAxes
    );
    assert_eq!(
        ProcessedData::new(Vec::new(), Vec::new(), Vec::new()).unwrap_err(),
        ProcessedValidationError::EmptyAxes
    );
    let huge = ProcessedAxis::new(
        AxisRole::Signal,
        AxisDomain::Time,
        Some(AxisUnit::Second),
        usize::MAX,
        AxisCoordinates::Unknown,
        ComponentBasis::Cartesian,
    )
    .unwrap();
    assert_eq!(
        ProcessedDescriptor::new(vec![huge]).unwrap_err(),
        ProcessedValidationError::SizeOverflow
    );
    assert_eq!(
        ProcessedData::new(vec![usize::MAX, 2, 2], vec![2, 1, 1], Vec::new()).unwrap_err(),
        ProcessedValidationError::SizeOverflow
    );
    assert_eq!(
        ProcessedData::new(vec![2, 2, 2], vec![1, 1], Vec::new()).unwrap_err(),
        ProcessedValidationError::ComponentRankMismatch
    );
    assert_eq!(
        ProcessedData::new(vec![2, 0, 2], vec![1, 1, 1], Vec::new()).unwrap_err(),
        ProcessedValidationError::ZeroExtent
    );
    assert_eq!(
        ProcessedData::new(vec![2, 2, 2], vec![1, 1, 1], vec![0.; 7]).unwrap_err(),
        ProcessedValidationError::SampleLengthMismatch
    );
}

#[test]
fn declared_raw_lineage_accepts_axis_subsets_and_permutations_and_rejects_invalid_references() {
    use nmr::processed::{ProcessedData, ProcessedDataset, ProcessedOrigin, ProcessedProvenance};
    let indirect = || {
        RawAxis::new(
            RawAxisKind::Indirect(IndirectComponents::Scalar),
            AxisDomain::Time,
            Some(AxisUnit::Second),
            2,
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.5,
            },
        )
        .unwrap()
    };
    let raw = RawDatasetBuilder::new(
        vec![indirect(), indirect(), direct(2, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .dense(vec![Complex64::default(); 8])
    .unwrap();
    let build_refs = |lineage: Vec<nmr::provenance::InputAxisRef>, roles: Vec<AxisRole>| {
        let rank = roles.len();
        let descriptor = ProcessedDescriptor::new(
            roles
                .into_iter()
                .map(|role| processed_time_axis(role, 2, ComponentBasis::Scalar))
                .collect(),
        )
        .unwrap();
        ProcessedDataset::new(
            descriptor,
            ProcessedData::new(vec![2; rank], vec![1; rank], vec![0.; 1 << rank]).unwrap(),
            ProcessedProvenance::new(
                ProcessedOrigin::DeclaredRaw {
                    snapshot: Box::new(raw.snapshot()),
                    axis_lineage: lineage,
                },
                Vec::new(),
            )
            .unwrap(),
        )
    };
    let build = |lineage: Vec<usize>, roles| {
        build_refs(
            lineage
                .into_iter()
                .map(|axis| {
                    nmr::provenance::InputAxisRef::new(nmr::provenance::InputSlot::new(0), axis)
                })
                .collect(),
            roles,
        )
    };
    for slot in [1, usize::MAX] {
        assert_eq!(
            build_refs(
                vec![nmr::provenance::InputAxisRef::new(
                    nmr::provenance::InputSlot::new(slot),
                    0
                )],
                vec![AxisRole::Signal]
            )
            .unwrap_err(),
            ProcessedValidationError::InvalidAxisLineage { axis: 0 }
        );
    }
    build(
        vec![2, 0, 1],
        vec![
            AxisRole::DirectAcquisition,
            AxisRole::IndirectAcquisition,
            AxisRole::IndirectAcquisition,
        ],
    )
    .unwrap();
    build(vec![2, 0], vec![AxisRole::Signal; 2]).unwrap();
    let transposed = build(
        vec![2, 0],
        vec![AxisRole::DirectAcquisition, AxisRole::IndirectAcquisition],
    )
    .unwrap();
    let plan =
        nmr::processing::ProcessingPlan::new(vec![nmr::processing::ProcessingOperation::Window {
            axis: 0,
            window: nmr::processing::Window::exponential(1.0).unwrap(),
        }])
        .unwrap();
    assert_eq!(
        plan.apply_processed(&transposed).unwrap_err(),
        nmr::processing::ProcessingError::MissingCapability {
            capability: "direct acquisition axis in fastest storage position",
            axis: Some(0),
        }
    );
    build(vec![1], vec![AxisRole::IndirectAcquisition]).unwrap();
    assert_eq!(
        build(vec![0], vec![AxisRole::Signal; 2]).unwrap_err(),
        ProcessedValidationError::OriginRankMismatch
    );
    assert_eq!(
        build(vec![0, 0], vec![AxisRole::Signal; 2]).unwrap_err(),
        ProcessedValidationError::InvalidAxisLineage { axis: 1 }
    );
    assert_eq!(
        build(vec![3], vec![AxisRole::Signal]).unwrap_err(),
        ProcessedValidationError::InvalidAxisLineage { axis: 0 }
    );
    assert_eq!(
        build(vec![usize::MAX], vec![AxisRole::Signal]).unwrap_err(),
        ProcessedValidationError::InvalidAxisLineage { axis: 0 }
    );
    assert_eq!(
        build(vec![0], vec![AxisRole::DirectAcquisition]).unwrap_err(),
        ProcessedValidationError::OriginAxisMismatch { axis: 0 }
    );
    assert_eq!(
        ProcessedDescriptor::new(vec![
            processed_time_axis(
                AxisRole::DirectAcquisition,
                2,
                ComponentBasis::Scalar
            );
            2
        ])
        .unwrap_err(),
        ProcessedValidationError::MultipleDirectAxes
    );
}
