use nmr::Complex64;
use nmr::axis::{AxisCoordinates, AxisDomain, AxisQuantity, AxisRole, AxisUnit};
use nmr::processing::GroupDelayCapability;
use nmr::raw::{
    AxisIndex, ChemicalShiftReference, ComponentEvidence, DirectSamples, GroupDelayState,
    IndirectComponents, ModulationIndexDomain, PendingGroupDelay, PeriodicLaneModulation, RawAxis,
    RawAxisKind, RawDatasetBuilder, RawMetadata, StorageOrder, ValidationError,
};

use super::support::*;

#[test]
fn raw_axis_kind_is_the_only_source_of_role_and_lane_count() {
    let cases = [
        (
            RawAxisKind::Direct(DirectSamples::Real),
            AxisRole::DirectAcquisition,
            1,
        ),
        (
            RawAxisKind::Direct(DirectSamples::Complex),
            AxisRole::DirectAcquisition,
            1,
        ),
        (
            RawAxisKind::Indirect(IndirectComponents::Scalar),
            AxisRole::IndirectAcquisition,
            1,
        ),
        (
            RawAxisKind::Indirect(IndirectComponents::Cartesian(
                ComponentEvidence::user_constructed(),
            )),
            AxisRole::IndirectAcquisition,
            2,
        ),
        (
            RawAxisKind::Indirect(IndirectComponents::Encoded(identity_transform(3))),
            AxisRole::IndirectAcquisition,
            3,
        ),
        (RawAxisKind::Parameter, AxisRole::ArrayParameter, 1),
    ];
    for (kind, role, lanes) in cases {
        let domain = if matches!(kind, RawAxisKind::Parameter) {
            AxisDomain::Parameter
        } else {
            AxisDomain::Unknown
        };
        let axis = RawAxis::new(kind, domain, None, 2, AxisCoordinates::Unknown).unwrap();
        assert_eq!(axis.role(), role);
        assert_eq!(axis.component_lanes(), lanes);
    }
}

#[test]
fn descriptor_freezes_slowest_to_fastest_layout_and_direct_axis_position() {
    let indirect = RawAxis::new(
        RawAxisKind::Indirect(IndirectComponents::Cartesian(
            ComponentEvidence::user_constructed(),
        )),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        3,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let dataset = RawDatasetBuilder::new(
        vec![indirect.clone(), direct(2, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .dense(vec![Complex64::default(); 12])
    .unwrap();
    assert_eq!(dataset.data().layout().logical_shape(), &[3, 2]);
    assert_eq!(dataset.data().layout().lane_counts(), &[2, 1]);
    assert_eq!(dataset.data().layout().storage_shape().unwrap(), [6, 2]);
    assert_eq!(
        dataset.data().layout().storage_order(),
        StorageOrder::RowMajorDirectFastest
    );

    assert_eq!(
        RawDatasetBuilder::new(
            vec![direct(2, DirectSamples::Complex), indirect],
            RawMetadata::default()
        )
        .err()
        .unwrap(),
        ValidationError::DirectAxisNotFastest
    );
}

#[test]
fn public_builder_cannot_accept_layout_arrays_independent_of_axes() {
    let dataset = RawDatasetBuilder::new(
        vec![direct(2, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .dense(vec![Complex64::new(1.0, 2.0), Complex64::new(3.0, 4.0)])
    .unwrap();
    assert_eq!(dataset.descriptor().logical_shape(), vec![2]);
    assert_eq!(dataset.descriptor().component_lanes(), vec![1]);
    assert_eq!(dataset.data().storage_shape().unwrap(), vec![2]);
}

#[test]
fn real_direct_samples_and_parameter_signal_evidence_fail_at_the_boundary() {
    assert_eq!(
        RawDatasetBuilder::new(vec![direct(1, DirectSamples::Real)], RawMetadata::default())
            .unwrap()
            .dense(vec![Complex64::new(0.0, 1.0)])
            .unwrap_err(),
        ValidationError::ImaginarySamplesOnRealAxis
    );
    let parameter = RawAxis::new(
        RawAxisKind::Parameter,
        AxisDomain::Parameter,
        None,
        2,
        AxisCoordinates::Explicit(vec![1.0, 2.0]),
    )
    .unwrap();
    assert_eq!(
        parameter
            .with_chemical_shift_reference(Some(
                ChemicalShiftReference::user_constructed(0.0, 400.0).unwrap()
            ))
            .unwrap_err(),
        ValidationError::SignalEvidenceOnParameterAxis
    );
}

#[test]
fn parameter_quantity_requires_matching_role_domain_and_unit() {
    let gradient = RawAxis::new(
        RawAxisKind::Parameter,
        AxisDomain::Parameter,
        Some(AxisUnit::TeslaPerMeter),
        2,
        AxisCoordinates::Uniform {
            start: 0.1,
            step: 0.1,
        },
    )
    .unwrap()
    .with_quantity(Some(AxisQuantity::MagneticFieldGradientStrength))
    .unwrap();
    assert_eq!(
        gradient.quantity(),
        Some(AxisQuantity::MagneticFieldGradientStrength)
    );
    assert!(
        direct(2, DirectSamples::Real)
            .with_quantity(Some(AxisQuantity::TimeDelay))
            .is_err()
    );
}

#[test]
fn capabilities_are_derived_from_descriptor_and_correction_state() {
    let indirect = RawAxis::new(
        RawAxisKind::Indirect(IndirectComponents::Encoded(identity_transform(2))),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        2,
        AxisCoordinates::Unknown,
    )
    .unwrap();
    let direct = direct(2, DirectSamples::Complex)
        .with_group_delay(GroupDelayState::Pending(
            PendingGroupDelay::user_constructed(1.25).unwrap(),
        ))
        .unwrap()
        .with_chemical_shift_reference(Some(
            ChemicalShiftReference::user_constructed(4.7, 400.0).unwrap(),
        ))
        .unwrap();
    let dataset = RawDatasetBuilder::new(vec![indirect, direct], RawMetadata::default())
        .unwrap()
        .dense(vec![Complex64::default(); 8])
        .unwrap();
    let capabilities = dataset.capabilities();
    assert!(capabilities.supports_dense_processing());
    assert!(capabilities.has_component_transform(0));
    assert!(capabilities.has_ppm_reference(1));
    assert_eq!(
        capabilities.direct_group_delay(),
        GroupDelayCapability::Pending
    );
}

#[test]
fn rank_three_raw_is_valid_but_dense_processing_capability_is_false() {
    let indirect = || {
        RawAxis::new(
            RawAxisKind::Indirect(IndirectComponents::Scalar),
            AxisDomain::Time,
            Some(AxisUnit::Second),
            2,
            AxisCoordinates::Unknown,
        )
        .unwrap()
    };
    let dataset = RawDatasetBuilder::new(
        vec![indirect(), indirect(), direct(2, DirectSamples::Complex)],
        RawMetadata::default(),
    )
    .unwrap()
    .dense(vec![Complex64::default(); 8])
    .unwrap();
    assert_eq!(dataset.capabilities().rank(), 3);
    assert!(!dataset.capabilities().supports_dense_processing());
}

#[test]
fn modulation_domain_and_origin_are_explicit_and_signed() {
    let modulation = PeriodicLaneModulation::try_new(
        2,
        2,
        vec![Complex64::new(1.0, 0.0); 4],
        ModulationIndexDomain::AbsoluteGridCoordinate(AxisIndex::new(0)),
        -3,
    )
    .unwrap();
    assert_eq!(
        modulation.domain(),
        ModulationIndexDomain::AbsoluteGridCoordinate(AxisIndex::new(0))
    );
    assert_eq!(modulation.origin(), -3);
}

#[test]
fn identity_modulation_rejects_unrepresentable_sizes_before_allocation() {
    use nmr::raw::TransformValidationError;
    assert_eq!(
        PeriodicLaneModulation::identity(0),
        Err(TransformValidationError::ZeroInputLanes)
    );
    // Each case must fail arithmetic/layout checks without requesting huge memory.
    for lanes in [
        usize::MAX,
        usize::MAX / size_of::<Complex64>() + 1,
        isize::MAX as usize / size_of::<Complex64>() + 1,
        isize::MAX as usize / size_of::<Complex64>(),
    ] {
        assert_eq!(
            PeriodicLaneModulation::identity(lanes),
            Err(TransformValidationError::SizeOverflow)
        );
    }
    for lanes in [1, 2, 7] {
        let identity = PeriodicLaneModulation::identity(lanes).unwrap();
        assert_eq!(identity.input_lanes(), lanes);
        assert_eq!(identity.period(), 1);
        assert_eq!(
            identity.multipliers(),
            vec![Complex64::new(1.0, 0.0); lanes]
        );
    }
    assert_eq!(
        PeriodicLaneModulation::try_new(
            usize::MAX,
            2,
            Vec::<Complex64>::new(),
            ModulationIndexDomain::ObservationOrdinal,
            0
        ),
        Err(TransformValidationError::SizeOverflow)
    );
}
