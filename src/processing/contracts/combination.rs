use super::{error::ProcessingError, state::PlanState};
use crate::processed::ProcessedDescriptor;
use crate::{acquisition::ComponentBasis, axis::AxisCoordinates};

pub(crate) fn compatible(
    a: &ProcessedDescriptor,
    b: &ProcessedDescriptor,
) -> Result<(), ProcessingError> {
    if a.axes().len() != 1 || b.axes().len() != 1 {
        return Err(ProcessingError::InvalidParameter(
            "linear combination requires rank one",
        ));
    }
    let ax = &a.axes()[0];
    let bx = &b.axes()[0];
    if ax.domain() != crate::axis::AxisDomain::Frequency
        || bx.domain() != ax.domain()
        || ax.unit().is_none()
        || ax.unit() != bx.unit()
        || ax.nucleus() != bx.nucleus()
        || ax.component_basis() != bx.component_basis()
        || matches!(ax.component_basis(), ComponentBasis::Encoded(_))
    {
        return Err(ProcessingError::InvalidParameter(
            "incompatible spectrum units, nucleus or components",
        ));
    }
    for axis in [ax, bx] {
        if matches!(axis.coordinates(), AxisCoordinates::Unknown) {
            return Err(ProcessingError::MissingCapability {
                capability: "interpolation coordinates",
                axis: Some(0),
            });
        }
    }
    Ok(())
}

pub(crate) fn combined_state(
    descriptor: &ProcessedDescriptor,
    a: &PlanState,
    b: &PlanState,
) -> PlanState {
    let mut state = PlanState::from_descriptor(descriptor);
    // Output coordinates are A's coordinates, so their carrier calibration is A's.
    // A filter statement is retained only when it describes both inputs.
    state.axes[0].chemical_shift_reference = a.axes[0].chemical_shift_reference.clone();
    if a.axes[0].group_delay == b.axes[0].group_delay {
        state.axes[0].group_delay = a.axes[0].group_delay.clone();
    }
    state
}
