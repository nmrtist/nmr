use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, CoordinateError};
use nmr::processed::{
    ComponentBasis, ProcessedAxis, ProcessedData, ProcessedDataset, ProcessedDescriptor,
    ProcessedOrigin, ProcessedProvenance,
};
use nmr::{AxisIndex, Complex64};

use crate::datasets::*;

#[test]
fn shared_complex_descriptors_require_a_distinct_cartesian_owner() {
    let shared = |owner| {
        axis(
            3,
            ComponentBasis::SharedComplex {
                axis: AxisIndex::new(owner),
                conjugated: true,
            },
        )
    };
    for owner in [0, 2, usize::MAX] {
        assert!(
            ProcessedDescriptor::new(vec![shared(owner), axis(4, ComponentBasis::Cartesian)])
                .is_err()
        );
    }
    assert!(ProcessedDescriptor::new(vec![shared(1), axis(4, ComponentBasis::Scalar)]).is_err());
    assert!(ProcessedDescriptor::new(vec![shared(1), shared(0)]).is_err());
    let descriptor =
        ProcessedDescriptor::new(vec![shared(1), axis(4, ComponentBasis::Cartesian)]).unwrap();
    assert_eq!(descriptor.component_counts(), vec![1, 2]);
}

#[test]
fn planes_and_cartesian_traces_match_independent_tensor_indexing() {
    for n0 in 1..5 {
        for n1 in 1..6 {
            for c0 in 1..4 {
                for c1 in 1..3 {
                    let values: Vec<_> = (0..n0 * c0 * n1 * c1).map(|n| n as f64).collect();
                    let data =
                        ProcessedData::new(vec![n0, n1], vec![c0, c1], values.clone()).unwrap();
                    for lane0 in 0..c0 {
                        for lane1 in 0..c1 {
                            let plane = data.component_plane(&[lane0, lane1]).unwrap();
                            assert_eq!(plane.len(), n0 * n1);
                            let expected: Vec<_> = (0..n0)
                                .flat_map(|i| {
                                    (0..n1).map(move |j| {
                                        ((i * c0 + lane0) * n1 * c1 + j * c1 + lane1) as f64
                                    })
                                })
                                .collect();
                            assert_eq!(plane.copied().collect::<Vec<_>>(), expected);
                        }
                    }
                }
            }
        }
    }
    let descriptor = ProcessedDescriptor::new(vec![
        axis(3, ComponentBasis::Cartesian),
        axis(4, ComponentBasis::Cartesian),
    ])
    .unwrap();
    let data = ProcessedDataset::from_dense_samples(
        descriptor,
        (0..48).map(|n| n as f64).collect(),
        ProcessedProvenance::new(ProcessedOrigin::Unknown, vec![]).unwrap(),
    )
    .unwrap();
    let trace = data
        .complex_trace(AxisIndex::new(1), &[2, 0], AxisIndex::new(0), &[0, 1])
        .unwrap();
    assert_eq!(
        trace.collect::<Vec<_>>(),
        (0..4)
            .map(|j| Complex64::new((33 + 2 * j) as f64, (41 + 2 * j) as f64))
            .collect::<Vec<_>>()
    );
    assert!(data.data().scalar_plane().is_err());
}

#[test]
fn coordinates_preserve_fused_evaluation_and_distinguish_unknown_from_bounds() {
    let known = axis(31, ComponentBasis::Scalar);
    for (i, value) in known.coordinate_iter().unwrap().enumerate() {
        assert_eq!(value.to_bits(), 0.125f64.mul_add(i as f64, -3.0).to_bits());
        assert_eq!(known.coordinate(i).unwrap().to_bits(), value.to_bits());
    }
    let unknown = ProcessedAxis::new(
        AxisRole::Unknown,
        AxisDomain::Unknown,
        None,
        2,
        AxisCoordinates::Unknown,
        ComponentBasis::Scalar,
    )
    .unwrap();
    assert_eq!(unknown.coordinate(0), Err(CoordinateError::Unknown));
    assert!(matches!(
        unknown.coordinate(2),
        Err(CoordinateError::OutOfBounds { .. })
    ));
}

proptest::proptest! {
    #[test]
    fn arbitrary_planes_match_independent_storage_formula(dimensions in proptest::collection::vec((1usize..4,1usize..4),1..5), salt in 0usize..97) {
        let shape:Vec<_>=dimensions.iter().map(|v|v.0).collect();let counts:Vec<_>=dimensions.iter().map(|v|v.1).collect();
        let lanes:Vec<_>=counts.iter().enumerate().map(|(i,c)|(salt+i)%c).collect();let total:usize=shape.iter().product();let scalars:usize=dimensions.iter().map(|(n,c)|n*c).product();
        let data=ProcessedData::new(shape.clone(),counts.clone(),(0..scalars).map(|v|v as f64).collect()).unwrap();
        let actual:Vec<_>=data.component_plane(&lanes).unwrap().copied().collect();
        for (logical,value) in actual.iter().enumerate().take(total) {
            let mut rest=logical;let mut coordinates=vec![0;shape.len()];for i in (0..shape.len()).rev(){coordinates[i]=rest%shape[i];rest/=shape[i];}
            let mut offset=0;for i in 0..shape.len(){offset=offset*shape[i]*counts[i]+coordinates[i]*counts[i]+lanes[i];}
            proptest::prop_assert_eq!(*value,offset as f64);
        }
    }
}
