use super::{ComponentEvidence, ResolvedComponentTransform};

/// Raw samples on the unique direct acquisition axis.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectSamples {
    /// One real-valued sample per point.
    Real,
    /// One canonical `R + iI` complex sample per point.
    Complex,
}

/// Raw component meaning on an indirect acquisition axis.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum IndirectComponents {
    /// One scalar lane.
    Scalar,
    /// One lane sharing the direct axis's complex pair, rather than independent
    /// Cartesian components. `conjugated` reverses its imaginary orientation.
    SharedComplex {
        /// Whether the indirect imaginary unit is opposite to the direct unit.
        conjugated: bool,
        /// Evidence for the shared pairing and relative orientation.
        evidence: ComponentEvidence,
    },
    /// Two already-normalized Cartesian lanes.
    Cartesian(ComponentEvidence),
    /// Input lanes awaiting the resolved transform.
    Encoded(ResolvedComponentTransform),
}

/// Canonical kind and component semantics of a raw axis.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum RawAxisKind {
    /// The unique, fastest direct acquisition axis.
    Direct(DirectSamples),
    /// An indirect acquisition axis.
    Indirect(IndirectComponents),
    /// A scalar arrayed parameter axis.
    Parameter,
}

impl RawAxisKind {
    /// Returns the raw lane count derived from the semantic kind.
    pub fn lane_count(&self) -> usize {
        match self {
            Self::Direct(_)
            | Self::Parameter
            | Self::Indirect(
                IndirectComponents::Scalar | IndirectComponents::SharedComplex { .. },
            ) => 1,
            Self::Indirect(IndirectComponents::Cartesian(_)) => 2,
            Self::Indirect(IndirectComponents::Encoded(transform)) => transform.input_lanes(),
        }
    }
}

/// Processed component meaning on one axis.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentBasis {
    /// One scalar component.
    Scalar,
    /// Uses another axis's Cartesian pair without allocating duplicate components.
    SharedComplex {
        /// Axis owning the stored real/imaginary components.
        axis: super::AxisIndex,
        /// Whether this axis uses the opposite imaginary orientation.
        conjugated: bool,
    },
    /// Two Cartesian components ordered real, then imaginary.
    Cartesian,
    /// Raw lanes awaiting a resolved transform.
    Encoded(ResolvedComponentTransform),
}

impl ComponentBasis {
    /// Returns the stored component count. A shared complex axis has one stored
    /// component on this axis and uses its owner's two Cartesian components.
    pub fn component_count(&self) -> usize {
        match self {
            Self::Scalar | Self::SharedComplex { .. } => 1,
            Self::Cartesian => 2,
            Self::Encoded(transform) => transform.input_lanes(),
        }
    }
}
