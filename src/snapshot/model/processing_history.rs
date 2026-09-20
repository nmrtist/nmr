#[allow(unused_imports)]
use crate::processing::contracts::history::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for ComponentAccumulationOrder {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::LaneAscending => record(
                    "processing_history.ComponentAccumulationOrder.v1.LaneAscending",
                    vec![],
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
                "processing_history.ComponentAccumulationOrder.v1.LaneAscending"
                    if values.is_empty() =>
                {
                    let mut f = values.into_iter();
                    Ok(Self::LaneAscending)
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ExecutionEnvironment {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_history.ExecutionEnvironment.v1",
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
            let mut f = fields(node, "processing_history.ExecutionEnvironment.v1", 7)?;
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
    impl Codec for ExecutionSegment {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_history.ExecutionSegment.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_history.ExecutionSegment.v1", 4)?;
            Self::from_model_parts((
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
                next(&mut f, budget)?,
            ))
            .map_err(SnapshotError::model)
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for HistoryInput {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Raw {
                    digests,
                    normalization,
                    format,
                    sources,
                } => record(
                    "processing_history.HistoryInput.v1.Raw",
                    vec![
                        digests.to_wire(budget)?,
                        normalization.to_wire(budget)?,
                        format.to_wire(budget)?,
                        sources.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::Processed {
                    digests,
                    sources,
                    read_record,
                } => record(
                    "processing_history.HistoryInput.v1.Processed",
                    vec![
                        digests.to_wire(budget)?,
                        sources.to_wire(budget)?,
                        read_record.to_wire(budget)?,
                    ],
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
                "processing_history.HistoryInput.v1.Raw" if values.len() == 4 => {
                    let mut f = values.into_iter();
                    Ok(Self::Raw {
                        digests: next(&mut f, budget)?,
                        normalization: next(&mut f, budget)?,
                        format: next(&mut f, budget)?,
                        sources: next(&mut f, budget)?,
                    })
                }
                "processing_history.HistoryInput.v1.Processed" if values.len() == 3 => {
                    let mut f = values.into_iter();
                    Ok(Self::Processed {
                        digests: next(&mut f, budget)?,
                        sources: next(&mut f, budget)?,
                        read_record: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
    #[allow(unused_mut, unused_variables)]
    impl Codec for ProcessingHistory {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_history.ProcessingHistory.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                    (*self.model_parts().2).to_wire(budget)?,
                    (*self.model_parts().3).to_wire(budget)?,
                    (*self.model_parts().4).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_history.ProcessingHistory.v1", 5)?;
            Self::from_model_parts((
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
    impl Codec for ProcessingRecord {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            match self {
                Self::Attempted {
                    requested,
                    failure,
                    diagnostics,
                } => record(
                    "processing_history.ProcessingRecord.v1.Attempted",
                    vec![
                        requested.to_wire(budget)?,
                        failure.to_wire(budget)?,
                        diagnostics.to_wire(budget)?,
                    ],
                    budget,
                ),
                Self::Applied {
                    requested,
                    resolved,
                    input_descriptor,
                    output_descriptor,
                    algorithm_version,
                    accumulation_order,
                    diagnostics,
                } => record(
                    "processing_history.ProcessingRecord.v1.Applied",
                    vec![
                        requested.to_wire(budget)?,
                        resolved.to_wire(budget)?,
                        input_descriptor.to_wire(budget)?,
                        output_descriptor.to_wire(budget)?,
                        algorithm_version.to_wire(budget)?,
                        accumulation_order.to_wire(budget)?,
                        diagnostics.to_wire(budget)?,
                    ],
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
                "processing_history.ProcessingRecord.v1.Attempted" if values.len() == 3 => {
                    let mut f = values.into_iter();
                    Ok(Self::Attempted {
                        requested: next(&mut f, budget)?,
                        failure: next(&mut f, budget)?,
                        diagnostics: next(&mut f, budget)?,
                    })
                }
                "processing_history.ProcessingRecord.v1.Applied" if values.len() == 7 => {
                    let mut f = values.into_iter();
                    Ok(Self::Applied {
                        requested: next(&mut f, budget)?,
                        resolved: next(&mut f, budget)?,
                        input_descriptor: next(&mut f, budget)?,
                        output_descriptor: next(&mut f, budget)?,
                        algorithm_version: next(&mut f, budget)?,
                        accumulation_order: next(&mut f, budget)?,
                        diagnostics: next(&mut f, budget)?,
                    })
                }
                _ => Err(SnapshotError::Structure),
            }
        }
    }
};
