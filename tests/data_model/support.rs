use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisRole, AxisUnit};
use nmr::processed::{ComponentBasis, ProcessedAxis};
use nmr::raw::{
    DirectSamples, LinearComponentTransform, PeriodicLaneModulation, RawAxis, RawAxisKind,
};

pub(super) fn direct(points: usize, encoding: DirectSamples) -> RawAxis {
    RawAxis::new(
        RawAxisKind::Direct(encoding),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        points,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.5,
        },
    )
    .unwrap()
}

pub(super) fn identity_transform(lanes: usize) -> nmr::raw::ResolvedComponentTransform {
    assert!(lanes >= 2);
    let mut coefficients = vec![Complex64::new(0.0, 0.0); lanes * 2];
    coefficients[0] = Complex64::new(1.0, 0.0);
    coefficients[lanes + 1] = Complex64::new(1.0, 0.0);
    nmr::raw::ResolvedComponentTransform::user_constructed(
        LinearComponentTransform::try_new(
            lanes,
            coefficients,
            PeriodicLaneModulation::identity(lanes).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

pub(super) fn processed_time_axis(
    role: AxisRole,
    points: usize,
    basis: ComponentBasis,
) -> ProcessedAxis {
    ProcessedAxis::new(
        role,
        AxisDomain::Time,
        Some(AxisUnit::Second),
        points,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.5,
        },
        basis,
    )
    .unwrap()
}
