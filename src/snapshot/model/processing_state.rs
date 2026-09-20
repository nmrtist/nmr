#[allow(unused_imports)]
use crate::processing::contracts::state::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for AxisState {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_state.AxisState.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                    (*self.model_parts().5).to_wire(budget)?,
                    (*self.model_parts().6).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_state.AxisState.v1", 7)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for PlanState {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_state.PlanState.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_state.PlanState.v1", 3)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessingDelayState {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::NotApplicable => record(
                    "processing_state.ProcessingDelayState.v1.NotApplicable",
                    vec![],
                    budget,
                ),
                Self::Unknown => record(
                    "processing_state.ProcessingDelayState.v1.Unknown",
                    vec![],
                    budget,
                ),
                Self::Pending(v0) => record(
                    "processing_state.ProcessingDelayState.v1.Pending",
                    vec![v0.to_wire(budget)?],
                    budget,
                ),
                Self::Corrected { delay, evidence } => record(
                    "processing_state.ProcessingDelayState.v1.Corrected",
                    vec![delay.to_wire(budget)?, evidence.to_wire(budget)?],
                    budget,
                ),
            }
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let Value::Record(tag, values) = node else {
                return Err(SnapshotError::Structure);
            };
            match tag.as_ref() {
                "processing_state.ProcessingDelayState.v1.NotApplicable" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::NotApplicable)
                }
                "processing_state.ProcessingDelayState.v1.Unknown" if values.is_empty() => {
                    let mut f = values.into_iter();
                    Ok(Self::Unknown)
                }
                "processing_state.ProcessingDelayState.v1.Pending" if values.len() == 1 => {
                    let mut f = values.into_iter();
                    Ok(Self::Pending(next(&mut f, budget)?))
                }
                "processing_state.ProcessingDelayState.v1.Corrected" if values.len() == 2 => {
                    let mut f = values.into_iter();
                    Ok(Self::Corrected {
                        delay: next(&mut f, budget)?,
                        evidence: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};
