use super::{
    operation::*,
    spectrum,
    state::{PlanState, StateError},
};

pub(crate) fn validate_resolution(
    state: &PlanState,
    axis: usize,
    method: RealBaseline,
    resolved: &ResolvedOperation,
) -> Result<(), StateError> {
    let ResolvedOperation::EstimatedBaseline {
        values,
        coefficients,
    } = resolved
    else {
        return Err(StateError::Mapping("invalid estimated baseline"));
    };
    if axis != 0
        || state.axes.len() != 1
        || values.len() != state.axes[0].axis.points()
        || values.iter().chain(coefficients).any(|v| !v.is_finite())
    {
        return Err(StateError::Mapping("baseline values disagree with input"));
    }
    let count = match method {
        RealBaseline::Offset => 1,
        RealBaseline::Polynomial { order } => {
            order.checked_add(1).ok_or(StateError::SizeOverflow)?
        }
        RealBaseline::Asls { .. } => 0,
    };
    if coefficients.len() != count {
        return Err(StateError::Mapping("invalid baseline coefficients"));
    }
    if count > 0 {
        for (i, v) in values.iter().enumerate() {
            let fitted = spectrum::evaluate(
                coefficients,
                2.0 * i as f64 / (values.len() - 1).max(1) as f64 - 1.0,
            );
            if !fitted.is_finite()
                || (fitted - v).abs() > 1e-10 * v.abs().max(fitted.abs()).max(f64::MIN_POSITIVE)
            {
                return Err(StateError::Mapping(
                    "baseline values and coefficients disagree",
                ));
            }
        }
    }
    Ok(())
}
