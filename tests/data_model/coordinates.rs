use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{ComponentBasis, ProcessedAxis, ProcessedValidationError};
use nmr::raw::{ChemicalShiftReference, DirectSamples, RawAxis, RawAxisKind, ValidationError};

#[test]
fn uniform_axes_validate_generated_range_and_rounding_in_both_models() {
    use nmr::axis::AxisValidationError;
    let large = 2.0_f64.powi(53);
    let tiny = f64::from_bits(1);
    for (start, step, points, valid) in [
        (1e308, 1e308, 3, false),
        (-1e308, -1e308, 3, false),
        (large, 0.25, 3, false),
        (-large, -0.25, 3, false),
        // First and last adjacent pairs differ, but an interior pair collapses.
        (large, 1.5, 5, false),
        (-large, -1.5, 5, false),
        (f64::MAX, 0.0, 1, true),
        (f64::MAX, f64::MAX, 1, true),
        (0.0, 0.0, 2, false),
        (0.0, tiny, 4, true),
        (0.0, -tiny, 4, true),
        (large, 2.0, 4, true),
        (large, 1.5, 2, true),
        // Fused evaluation stays finite even when the separate product overflows.
        (-f64::MAX, f64::MAX, 3, true),
        (f64::MAX, -f64::MAX, 3, true),
        (-2000.0, 0.5, 8192, true),
        (12.0, -0.01, 1024, true),
    ] {
        let coordinates = AxisCoordinates::Uniform { start, step };
        let raw = RawAxis::new(
            RawAxisKind::Direct(DirectSamples::Complex),
            AxisDomain::Frequency,
            Some(AxisUnit::Hertz),
            points,
            coordinates.clone(),
        );
        let processed = ProcessedAxis::new(
            AxisRole::Signal,
            AxisDomain::Frequency,
            Some(AxisUnit::Hertz),
            points,
            coordinates,
            ComponentBasis::Cartesian,
        );
        if valid {
            raw.unwrap();
            processed.unwrap();
            let values: Vec<_> = (0..points).map(|i| step.mul_add(i as f64, start)).collect();
            assert!(values.iter().all(|v| v.is_finite()));
            assert!(
                values
                    .windows(2)
                    .all(|p| if step > 0.0 { p[1] > p[0] } else { p[1] < p[0] })
            );
        } else {
            assert_eq!(
                raw.unwrap_err(),
                ValidationError::Axis(AxisValidationError::InvalidCoordinates)
            );
            assert_eq!(
                processed.unwrap_err(),
                ProcessedValidationError::Axis(AxisValidationError::InvalidCoordinates)
            );
        }
    }
}

#[test]
fn chemical_shift_conversion_rejects_nonfinite_results() {
    use nmr::raw::EvidenceValidationError;
    for (carrier, reference, offset) in [
        (0.0, f64::MIN_POSITIVE, f64::MAX),
        (0.0, f64::MIN_POSITIVE, -f64::MAX),
        (f64::MAX, 1.0, f64::MAX),
        (-f64::MAX, 1.0, -f64::MAX),
    ] {
        assert_eq!(
            ChemicalShiftReference::user_constructed(carrier, reference)
                .unwrap()
                .ppm(offset),
            Err(EvidenceValidationError::InvalidChemicalShiftResult)
        );
    }
    let reference = ChemicalShiftReference::user_constructed(4.7, 400.0).unwrap();
    for offset in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            reference.ppm(offset),
            Err(EvidenceValidationError::InvalidFrequencyOffset)
        );
    }
    for (offset, expected) in [(-400.0, 3.7), (0.0, 4.7), (400.0, 5.7)] {
        assert!((reference.ppm(offset).unwrap() - expected).abs() < 1e-14);
    }
    assert_eq!(
        ChemicalShiftReference::user_constructed(0.0, f64::MIN_POSITIVE)
            .unwrap()
            .ppm(f64::MIN_POSITIVE)
            .unwrap(),
        1.0
    );
    assert_eq!(
        ChemicalShiftReference::user_constructed(-f64::MAX, 1.0)
            .unwrap()
            .ppm(f64::MAX)
            .unwrap(),
        0.0
    );
}

proptest::proptest! {
    #[test]
    fn uniform_axis_validation_matches_generated_coordinates(
        start in proptest::prelude::any::<f64>(),
        step in proptest::prelude::any::<f64>(),
        points in 1_usize..64,
    ) {
        let values: Vec<_> = (0..points).map(|i| step.mul_add(i as f64, start)).collect();
        let expected = start.is_finite() && step.is_finite()
            && values.iter().all(|v| v.is_finite())
            && values.windows(2).all(|p| if step > 0.0 { p[1] > p[0] } else { p[1] < p[0] });
        let result = ProcessedAxis::new(AxisRole::Signal, AxisDomain::Frequency,
            Some(AxisUnit::Hertz), points, AxisCoordinates::Uniform { start, step }, ComponentBasis::Scalar);
        proptest::prop_assert_eq!(result.is_ok(), expected);
    }
}
