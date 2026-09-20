#[allow(unused_imports)]
use crate::processing::methods::nus::*;
// Snapshot v1 conversion schema; field order is frozen.
const _: () = {
    #[allow(unused_imports)]
    use crate::snapshot::{
        SnapshotError,
        wire::{Budget, Codec, Value, fields, next, record},
    };
    #[allow(unused_mut, unused_variables)]
    impl Codec for NusSettings {
        fn to_wire<'a>(&'a self, budget: &mut Budget<'_, '_>) -> Result<Value<'a>, SnapshotError> {
            record(
                "processing_nus.NusSettings.v1",
                vec![
                    (*self.model_parts().0).to_wire(budget)?,
                    (*self.model_parts().1).to_wire(budget)?,
                ],
                budget,
            )
        }
        fn from_wire(
            node: Value<'static>,
            budget: &mut Budget<'_, '_>,
        ) -> Result<Self, SnapshotError> {
            let mut f = fields(node, "processing_nus.NusSettings.v1", 2)?;
            Self::from_model_parts((next(&mut f, budget)?, next(&mut f, budget)?))
                .map_err(SnapshotError::model)
        }
    }
};

impl crate::snapshot::wire::Codec for NusNoiseReport {
    fn to_wire<'a>(
        &'a self,
        budget: &mut crate::snapshot::wire::Budget<'_, '_>,
    ) -> Result<crate::snapshot::wire::Value<'a>, crate::snapshot::SnapshotError> {
        use crate::snapshot::wire::{Value, record};
        let source = match self.source {
            NusNoiseSource::Explicit => 0usize,
            NusNoiseSource::SplitObservationsV1 => 1,
            NusNoiseSource::SplitHoldoutV1 => 2,
            NusNoiseSource::JeolInteriorV1 => 3,
            NusNoiseSource::JeolInteriorHoldoutV1 => 4,
        };
        record(
            "processing_nus.NusNoiseReport.v1",
            vec![
                Value::Unsigned(source as u64),
                self.sigma.to_wire(budget)?,
                self.frequency_ranges.to_wire(budget)?,
                self.scalar_samples.to_wire(budget)?,
                self.effective_observations.to_wire(budget)?,
                self.block_dispersion.to_wire(budget)?,
                self.radial_moment_error.to_wire(budget)?,
                self.isotropy_error.to_wire(budget)?,
                self.observation_correlation.to_wire(budget)?,
            ],
            budget,
        )
    }
    fn from_wire(
        node: crate::snapshot::wire::Value<'static>,
        budget: &mut crate::snapshot::wire::Budget<'_, '_>,
    ) -> Result<Self, crate::snapshot::SnapshotError> {
        use crate::snapshot::{
            SnapshotError,
            wire::{fields, next},
        };
        let mut f = fields(node, "processing_nus.NusNoiseReport.v1", 9)?;
        let source: usize = next(&mut f, budget)?;
        Ok(Self {
            source: match source {
                0 => NusNoiseSource::Explicit,
                1 => NusNoiseSource::SplitObservationsV1,
                2 => NusNoiseSource::SplitHoldoutV1,
                3 => NusNoiseSource::JeolInteriorV1,
                4 => NusNoiseSource::JeolInteriorHoldoutV1,
                _ => return Err(SnapshotError::Structure),
            },
            sigma: next(&mut f, budget)?,
            frequency_ranges: next(&mut f, budget)?,
            scalar_samples: next(&mut f, budget)?,
            effective_observations: next(&mut f, budget)?,
            block_dispersion: next(&mut f, budget)?,
            radial_moment_error: next(&mut f, budget)?,
            isotropy_error: next(&mut f, budget)?,
            observation_correlation: next(&mut f, budget)?,
        })
    }
}
