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
        row_strides: [usize; 2],
        slow_lanes: usize,
        fast_lanes: usize,
    },
}

/// Strides for stored F1/F2 coordinates (including their quadrature lanes).
/// AQSEQ changes disk nesting, never the logical F1=acqu3s/F2=acqu2s identity.
pub(super) fn three_dimensional_strides(
    direct: &super::parameters::ParameterFile,
    source: &crate::raw::InputSource,
    stored_f1: usize,
    stored_f2: usize,
) -> Result<[usize; 2], crate::ReadError> {
    match direct.optional_integer("AQSEQ", source)?.unwrap_or(0) {
        0 => Ok([stored_f2, 1]), // 3-2-1: F2 varies fastest.
        1 => Ok([1, stored_f1]), // 3-1-2: F1 varies fastest.
        value => Err(crate::ReadError::unsupported_feature(
            source.clone(),
            crate::raw::UnsupportedFeatureCode::NON_SEPARABLE_COMPONENT_LAYOUT,
            None,
            vec![format!(
                "Bruker 3D AQSEQ={value}; supported orders are 0 (3-2-1) and 1 (3-1-2)"
            )],
        )),
    }
}
