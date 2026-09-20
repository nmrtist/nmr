use crate::axis::AxisDomain;
use crate::axis::AxisRole;
use crate::processed::ComponentBasis;
use crate::processing::contracts::operation::*;

use super::super::calibration::rebuild_axis;
use super::super::{AxisState, AxisUpdate, StateError};
use super::AxisTransition;

pub(super) fn component_transform(
    axis_state: &mut AxisState,
    axis_index: usize,
    rank: usize,
    direct_cartesian: bool,
    observation_ordinals: Option<std::sync::Arc<[usize]>>,
    absolute_origin: &[usize],
    invalid: &impl Fn(&'static str) -> StateError,
) -> Result<AxisTransition, StateError> {
    let axis = &axis_state.axis;
    Ok(AxisTransition::Applied({
        if axis.role() != AxisRole::IndirectAcquisition || axis.domain() != AxisDomain::Time {
            return Err(invalid(
                "requires an encoded indirect time-domain acquisition axis",
            ));
        }
        let ComponentBasis::Encoded(transform) = axis.component_basis() else {
            return Err(invalid("requires a resolved component transform"));
        };
        let transform = transform.clone();
        if rank != 2 || axis_index != 0 || !direct_cartesian {
            return Err(invalid(
                "component transform requires a Cartesian direct axis",
            ));
        }
        axis_state.axis = rebuild_axis(
            axis,
            AxisUpdate {
                component_basis: Some(ComponentBasis::Cartesian),
                ..AxisUpdate::default()
            },
        )?;
        let observation_ordinals = matches!(
            transform.transform().modulation().domain(),
            crate::acquisition::ModulationIndexDomain::ObservationOrdinal
        )
        .then_some(observation_ordinals)
        .flatten();
        let grid_origin = match transform.transform().modulation().domain() {
            crate::acquisition::ModulationIndexDomain::AbsoluteGridCoordinate(referenced) => {
                i64::try_from(absolute_origin[referenced.index()])
                    .map_err(|_| StateError::SizeOverflow)?
            }
            crate::acquisition::ModulationIndexDomain::ObservationOrdinal => 0,
        };
        ResolvedOperation::ComponentTransform {
            transform,
            observation_ordinals,
            grid_origin,
        }
    }))
}
