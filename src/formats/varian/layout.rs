/// Verified dimensions and numeric representation of a Varian `fid` payload.
pub(super) struct LayoutPlan {
    pub(super) traces_per_block: usize,
    pub(super) stored_values: usize,
    pub(super) bytes_per_value: usize,
    pub(super) trace_bytes: usize,
    pub(super) block_bytes: usize,
    pub(super) block_header_bytes: usize,
    pub(super) direct_points: usize,
    pub(super) trace_count: usize,
    pub(super) is_float: bool,
    pub(super) is_32_bit_integer: bool,
    pub(super) complex: bool,
    pub(super) scale_factors: Vec<f64>,
}
