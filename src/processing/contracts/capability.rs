use crate::acquisition::{GroupDelayState, RawAxisKind};
use crate::raw::RawDataset;

/// Capabilities derived from canonical descriptor and sampling state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawCapabilities {
    pub(in crate::processing) rank: usize,
    pub(in crate::processing) dense: bool,
    pub(in crate::processing) ppm_axes: Box<[bool]>,
    pub(in crate::processing) component_transform_axes: Box<[bool]>,
    pub(in crate::processing) direct_group_delay: GroupDelayCapability,
}

/// Direct-axis correction capability derived from raw correction state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupDelayCapability {
    /// No correction applies.
    NotApplicable,
    /// Applicability or delay value is unavailable.
    Unknown,
    /// A resolved correction remains to be applied.
    Pending,
}

impl RawCapabilities {
    /// Returns descriptor rank.
    pub fn rank(&self) -> usize {
        self.rank
    }
    /// Returns whether this input is dense and rank one or two.
    pub fn supports_dense_processing(&self) -> bool {
        self.dense && (1..=2).contains(&self.rank)
    }
    /// Returns whether one axis has a canonical chemical-shift reference.
    pub fn has_ppm_reference(&self, axis: usize) -> bool {
        self.ppm_axes.get(axis).copied().unwrap_or(false)
    }
    /// Returns whether one axis has a pending component transform.
    pub fn has_component_transform(&self, axis: usize) -> bool {
        self.component_transform_axes
            .get(axis)
            .copied()
            .unwrap_or(false)
    }
    /// Returns the derived direct-axis delay capability.
    pub fn direct_group_delay(&self) -> GroupDelayCapability {
        self.direct_group_delay
    }
}

impl RawDataset {
    /// Derives optional processing capabilities without cached booleans.
    pub fn capabilities(&self) -> RawCapabilities {
        let axes = self.descriptor().axes();
        RawCapabilities {
            rank: axes.len(),
            dense: !self.data().is_sparse(),
            ppm_axes: axes
                .iter()
                .map(|axis| axis.chemical_shift_reference().is_some())
                .collect(),
            component_transform_axes: axes
                .iter()
                .map(|axis| {
                    matches!(
                        axis.kind(),
                        RawAxisKind::Indirect(crate::acquisition::IndirectComponents::Encoded(_))
                    )
                })
                .collect(),
            direct_group_delay: match axes.last().map(|axis| axis.group_delay()) {
                Some(GroupDelayState::NotApplicable) => GroupDelayCapability::NotApplicable,
                Some(GroupDelayState::Unknown) => GroupDelayCapability::Unknown,
                Some(GroupDelayState::Pending(_)) => GroupDelayCapability::Pending,
                None => GroupDelayCapability::Unknown,
            },
        }
    }
}
