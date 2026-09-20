use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processing::{
    FourierTransform, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan,
};
use nmr::raw::{
    DirectSamples, GroupDelayState, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata,
};
use nmr::{Complex64, Dataset, ExecutionContext};

pub(crate) fn input() -> Dataset {
    let axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        64,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.001,
        },
    )
    .unwrap()
    .with_group_delay(GroupDelayState::NotApplicable)
    .unwrap();
    RawDatasetBuilder::new(vec![axis], RawMetadata::default())
        .unwrap()
        .dense(
            (0..64)
                .map(|i| Complex64::from_polar((-0.02 * i as f64).exp(), 0.2 * i as f64))
                .collect(),
        )
        .unwrap()
        .into()
}
pub(crate) fn spectrum(control: &mut ExecutionContext<'_>) -> Dataset {
    ProcessingPlan::new(vec![Op::FourierTransform {
        axis: 0,
        transform: FourierTransform::default(),
    }])
    .unwrap()
    .apply_with_context(&input(), ProcessingOptions::default(), control)
    .unwrap()
}
