/// Verified mapping from one logical trace to Bruker's on-disk rows.
pub(super) enum LayoutPlan {
    OneD {
        payload_bytes: usize,
    },
    TwoD {
        stride: usize,
        payload_bytes: usize,
        direct_points: usize,
        indirect_lanes: usize,
        acquisition_rows: Option<Vec<Option<usize>>>,
    },
    ThreeD {
        stride: usize,
        payload_bytes: usize,
        direct_points: usize,
        stored_second_indirect: usize,
        slow_lanes: usize,
        fast_lanes: usize,
    },
}
