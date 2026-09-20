use crate::Complex64;

use super::*;

#[test]
fn transform_rejects_bad_shapes_numbers_and_rank() {
    assert_eq!(
        PeriodicLaneModulation::identity(0),
        Err(TransformValidationError::ZeroInputLanes)
    );
    let modulation = PeriodicLaneModulation::identity(2).unwrap();
    assert!(matches!(
        LinearComponentTransform::try_new(2, vec![Complex64::new(1.0, 0.0); 3], modulation.clone()),
        Err(TransformValidationError::CoefficientLength { .. })
    ));
    assert_eq!(
        LinearComponentTransform::try_new(
            2,
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            modulation.clone(),
        ),
        Err(TransformValidationError::RankDeficient)
    );
    assert!(matches!(
        LinearComponentTransform::try_new(
            2,
            vec![
                Complex64::new(f64::NAN, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
            ],
            modulation,
        ),
        Err(TransformValidationError::NonFiniteCoefficient)
    ));

    let near_threshold = |epsilon| {
        LinearComponentTransform::try_new(
            2,
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(epsilon, 0.0),
            ],
            PeriodicLaneModulation::identity(2).unwrap(),
        )
    };
    assert!(near_threshold(2.1e-12).is_ok());
    assert_eq!(
        near_threshold(1.9e-12),
        Err(TransformValidationError::RankDeficient)
    );
    assert!(
        LinearComponentTransform::try_new(
            2,
            vec![
                Complex64::new(f64::MAX, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(f64::MAX, 0.0),
            ],
            PeriodicLaneModulation::identity(2).unwrap(),
        )
        .is_ok()
    );
}

#[test]
fn transform_uses_absolute_origin_and_lane_increasing_sum() {
    let modulation = PeriodicLaneModulation::try_new(
        2,
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(-1.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        ModulationIndexDomain::AbsoluteGridCoordinate(AxisIndex::new(0)),
        3,
    )
    .unwrap();
    let transform = LinearComponentTransform::try_new(
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 1.0),
            Complex64::new(0.0, -1.0),
        ],
        modulation,
    )
    .unwrap();
    let input = [Complex64::new(2.0, 0.0), Complex64::new(3.0, 0.0)];
    assert_eq!(
        transform.apply(&input, 3).unwrap(),
        [Complex64::new(5.0, 0.0), Complex64::new(0.0, -1.0)]
    );
    assert_eq!(
        transform.apply(&input, 4).unwrap(),
        [Complex64::new(1.0, 0.0), Complex64::new(0.0, -5.0)]
    );
    assert_eq!(transform.apply(&input, 5), transform.apply(&input, 3));
}

#[test]
fn each_lane_injection_exposes_its_matrix_column_for_one_and_i() {
    let coefficients = vec![
        Complex64::new(1.0, 2.0),
        Complex64::new(3.0, 4.0),
        Complex64::new(5.0, 6.0),
        Complex64::new(-2.0, 1.0),
        Complex64::new(0.5, -3.0),
        Complex64::new(4.0, -1.0),
    ];
    let transform = LinearComponentTransform::try_new(
        3,
        coefficients.clone(),
        PeriodicLaneModulation::identity(3).unwrap(),
    )
    .unwrap();
    for lane in 0..3 {
        let mut input = [Complex64::new(0.0, 0.0); 3];
        input[lane] = Complex64::new(1.0, 0.0);
        assert_eq!(
            transform.apply(&input, 0).unwrap(),
            [coefficients[lane], coefficients[3 + lane]]
        );
        input[lane] = Complex64::new(0.0, 1.0);
        assert_eq!(
            transform.apply(&input, 0).unwrap(),
            [
                coefficients[lane] * Complex64::new(0.0, 1.0),
                coefficients[3 + lane] * Complex64::new(0.0, 1.0),
            ]
        );
    }
}

#[test]
fn modulation_phase_handles_the_full_signed_index_domain() {
    let modulation = PeriodicLaneModulation::try_new(
        2,
        3,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0),
            Complex64::new(3.0, 0.0),
        ],
        ModulationIndexDomain::ObservationOrdinal,
        i64::MAX - 2,
    )
    .unwrap();
    let transform = LinearComponentTransform::try_new(
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        modulation,
    )
    .unwrap();
    let output = transform
        .apply(
            &[Complex64::new(1.0, 0.0), Complex64::new(1.0, 0.0)],
            i64::MIN,
        )
        .unwrap();
    // (i64::MIN - (i64::MAX - 2)) mod 3 == 2.
    assert_eq!(output, [Complex64::new(3.0, 0.0); 2]);
}

#[test]
fn observation_ordinal_modulation_distinguishes_duplicate_coordinates() {
    let modulation = PeriodicLaneModulation::try_new(
        2,
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(1.0, 0.0),
            Complex64::new(-1.0, 0.0),
            Complex64::new(-1.0, 0.0),
        ],
        ModulationIndexDomain::ObservationOrdinal,
        0,
    )
    .unwrap();
    let transform = LinearComponentTransform::try_new(
        2,
        vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(1.0, 0.0),
        ],
        modulation,
    )
    .unwrap();
    let duplicate_coordinate_lanes = [Complex64::new(2.0, 3.0), Complex64::new(5.0, 7.0)];
    assert_eq!(
        transform.apply(&duplicate_coordinate_lanes, 0).unwrap(),
        duplicate_coordinate_lanes
    );
    assert_eq!(
        transform.apply(&duplicate_coordinate_lanes, 1).unwrap(),
        [Complex64::new(-2.0, -3.0), Complex64::new(-5.0, -7.0),]
    );
}

#[test]
fn ppm_sign_is_frozen_to_higher_absolute_frequency() {
    let reference = ChemicalShiftReference::user_constructed(4.7, 400.0).unwrap();
    assert_eq!(reference.ppm(400.0).unwrap(), 5.7);
    assert_eq!(reference.ppm(-400.0).unwrap(), 3.7);
}
