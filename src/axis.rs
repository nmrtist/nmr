//! Vendor-neutral mathematical axis vocabulary.

use thiserror::Error;

/// Failure to obtain a logical point's physical coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum CoordinateError {
    /// Calibration is not known.
    #[error("axis coordinates are unknown")]
    Unknown,
    /// The index is outside the logical axis.
    #[error("coordinate index {index} exceeds {points} points")]
    OutOfBounds {
        /// Requested index.
        index: usize,
        /// Logical point count.
        points: usize,
    },
}

pub(crate) fn coordinate(
    coordinates: &AxisCoordinates,
    points: usize,
    index: usize,
) -> Result<f64, CoordinateError> {
    if index >= points {
        return Err(CoordinateError::OutOfBounds { index, points });
    }
    match coordinates {
        AxisCoordinates::Unknown => Err(CoordinateError::Unknown),
        AxisCoordinates::Uniform { start, step } => Ok(step.mul_add(index as f64, *start)),
        AxisCoordinates::Explicit(values) => Ok(values[index]),
    }
}

/// Allocation-free coordinate traversal with independently evaluated uniform points.
pub struct CoordinateIter<'a> {
    coordinates: &'a AxisCoordinates,
    indices: std::ops::Range<usize>,
}
impl<'a> CoordinateIter<'a> {
    pub(crate) fn new(
        coordinates: &'a AxisCoordinates,
        points: usize,
    ) -> Result<Self, CoordinateError> {
        if matches!(coordinates, AxisCoordinates::Unknown) {
            return Err(CoordinateError::Unknown);
        }
        Ok(Self {
            coordinates,
            indices: 0..points,
        })
    }
}
impl Iterator for CoordinateIter<'_> {
    type Item = f64;
    fn next(&mut self) -> Option<f64> {
        let index = self.indices.next()?;
        Some(match self.coordinates {
            AxisCoordinates::Uniform { start, step } => step.mul_add(index as f64, *start),
            AxisCoordinates::Explicit(values) => values[index],
            AxisCoordinates::Unknown => unreachable!("checked coordinate iterator"),
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}
impl ExactSizeIterator for CoordinateIter<'_> {}
impl std::iter::FusedIterator for CoordinateIter<'_> {}

/// Mathematical domain of an axis, independent of dataset state.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisDomain {
    /// Time coordinates.
    Time,
    /// Frequency coordinates.
    Frequency,
    /// A generic experimental parameter.
    Parameter,
    /// The coordinate quantity is not established.
    Unknown,
}

/// Scientific role of an axis.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisRole {
    /// The directly detected signal dimension.
    DirectAcquisition,
    /// An incremented indirect signal dimension.
    IndirectAcquisition,
    /// An arrayed experiment parameter.
    ArrayParameter,
    /// A signal axis whose acquisition role is no longer relevant or known.
    Signal,
    /// The source does not establish the role.
    Unknown,
}

impl AxisRole {
    pub(crate) fn is_signal(self) -> bool {
        matches!(
            self,
            Self::DirectAcquisition | Self::IndirectAcquisition | Self::Signal
        )
    }
}

/// Physical unit of axis coordinates.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisUnit {
    /// Seconds.
    Second,
    /// Hertz.
    Hertz,
    /// Parts per million.
    Ppm,
    /// Tesla.
    Tesla,
    /// Tesla per meter, the SI unit of magnetic-field gradient strength.
    TeslaPerMeter,
}

/// Physical quantity represented by a parameter axis.
///
/// Units alone are not sufficient to identify an arrayed experiment. For
/// example, seconds may represent a relaxation delay or another timing
/// parameter. Readers set a quantity only from typed source evidence.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisQuantity {
    /// Magnetic-field gradient strength.
    MagneticFieldGradientStrength,
    /// An arrayed time delay whose more specific experimental meaning is not
    /// established by the portable model.
    TimeDelay,
}

/// Coordinates for logical points.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum AxisCoordinates {
    /// Coordinates are absent or cannot be interpreted portably.
    Unknown,
    /// Finite uniformly spaced coordinates, evaluated as `step.mul_add(index as f64, start)`.
    /// Adjacent generated coordinates must remain distinct. Validation is constant
    /// time when the spacing exceeds rounding resolution, otherwise linear in points.
    Uniform {
        /// Coordinate of the first logical point.
        start: f64,
        /// Signed interval between adjacent logical points.
        step: f64,
    },
    /// One finite coordinate for every logical point.
    Explicit(Vec<f64>),
}

/// Direction derived from known coordinates.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisDirection {
    /// Coordinates increase with logical index.
    Ascending,
    /// Coordinates decrease with logical index.
    Descending,
    /// Coordinates are unknown, contain one point, or need not be monotonic.
    Unknown,
}

/// Distinct frequency quantities preserved as evidence.
///
/// This does not claim to encode a complete chemical-shift reference.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrequencyEvidence {
    observe_frequency_mhz: Option<f64>,
    transmitter_offset_hz: Option<f64>,
}

impl FrequencyEvidence {
    /// Creates checked frequency evidence.
    pub fn new(
        observe_frequency_mhz: Option<f64>,
        transmitter_offset_hz: Option<f64>,
    ) -> Result<Self, AxisValidationError> {
        let value = Self {
            observe_frequency_mhz,
            transmitter_offset_hz,
        };
        value.validate()?;
        Ok(value)
    }

    /// Returns the observe frequency in MHz.
    pub fn observe_frequency_mhz(&self) -> Option<f64> {
        self.observe_frequency_mhz
    }

    /// Returns the transmitter offset in Hz.
    pub fn transmitter_offset_hz(&self) -> Option<f64> {
        self.transmitter_offset_hz
    }

    pub(crate) fn validate(&self) -> Result<(), AxisValidationError> {
        for (name, value, positive) in [
            ("observe frequency", self.observe_frequency_mhz, true),
            ("transmitter offset", self.transmitter_offset_hz, false),
        ] {
            if value.is_some_and(|v| !v.is_finite() || positive && v <= 0.0) {
                return Err(AxisValidationError::InvalidNumber(name));
            }
        }
        Ok(())
    }
}

/// Axis-only validation failures shared by raw and processed models.
#[non_exhaustive]
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AxisValidationError {
    /// The axis has zero logical points.
    #[error("axis has zero points")]
    ZeroPoints,
    /// Coordinates have invalid length, finiteness, or monotonicity.
    #[error("axis coordinates are invalid for the declared domain")]
    InvalidCoordinates,
    /// Domain, unit, and coordinate knowledge disagree.
    #[error("axis domain, unit, and coordinates disagree")]
    DomainContractMismatch,
    /// A physical quantity disagrees with the role, domain, or unit.
    #[error("axis quantity, role, domain, and unit disagree")]
    QuantityContractMismatch,
    /// A named numeric value is invalid.
    #[error("invalid numeric value for {0}")]
    InvalidNumber(&'static str),
}

pub(crate) fn validate_quantity(
    role: AxisRole,
    domain: AxisDomain,
    unit: Option<AxisUnit>,
    quantity: Option<AxisQuantity>,
) -> Result<(), AxisValidationError> {
    let Some(quantity) = quantity else {
        return Ok(());
    };
    if role != AxisRole::ArrayParameter || domain != AxisDomain::Parameter {
        return Err(AxisValidationError::QuantityContractMismatch);
    }
    let matches = match quantity {
        AxisQuantity::MagneticFieldGradientStrength => unit == Some(AxisUnit::TeslaPerMeter),
        AxisQuantity::TimeDelay => unit == Some(AxisUnit::Second),
    };
    matches
        .then_some(())
        .ok_or(AxisValidationError::QuantityContractMismatch)
}

pub(crate) fn validate_axis(
    domain: AxisDomain,
    unit: Option<AxisUnit>,
    points: usize,
    coordinates: &AxisCoordinates,
) -> Result<(), AxisValidationError> {
    if points == 0 {
        return Err(AxisValidationError::ZeroPoints);
    }
    match coordinates {
        AxisCoordinates::Unknown => {}
        AxisCoordinates::Uniform { start, step } => {
            if !start.is_finite() || !step.is_finite() || points > 1 && *step == 0.0 {
                return Err(AxisValidationError::InvalidCoordinates);
            }
            validate_uniform(*start, *step, points)?;
        }
        AxisCoordinates::Explicit(values) => {
            if values.len() != points || values.iter().any(|value| !value.is_finite()) {
                return Err(AxisValidationError::InvalidCoordinates);
            }
        }
    }

    let contract_matches = match domain {
        AxisDomain::Time => unit == Some(AxisUnit::Second),
        AxisDomain::Frequency => {
            matches!(unit, Some(AxisUnit::Hertz | AxisUnit::Ppm))
                && !matches!(coordinates, AxisCoordinates::Unknown)
        }
        AxisDomain::Parameter => true,
        AxisDomain::Unknown => unit.is_none() && matches!(coordinates, AxisCoordinates::Unknown),
    };
    if !contract_matches {
        return Err(AxisValidationError::DomainContractMismatch);
    }
    if matches!(domain, AxisDomain::Time | AxisDomain::Frequency)
        && points > 1
        && !matches!(coordinates, AxisCoordinates::Unknown)
        && direction(coordinates, points) == AxisDirection::Unknown
    {
        return Err(AxisValidationError::InvalidCoordinates);
    }
    Ok(())
}

fn validate_uniform(start: f64, step: f64, points: usize) -> Result<(), AxisValidationError> {
    let invalid = AxisValidationError::InvalidCoordinates;
    // Beyond 2^53 adjacent integer indices cease to be distinct as f64.
    if (points - 1) as u128 > (1_u128 << 53) {
        return Err(invalid);
    }
    let last = step.mul_add((points - 1) as f64, start);
    if !last.is_finite() {
        return Err(invalid);
    }
    if points == 1 {
        return Ok(());
    }
    // A fused evaluation rounds only once. The largest rounding cell over
    // this monotone affine interval is bounded by the ULP at its largest
    // endpoint magnitude. A strictly larger step cannot collapse two points.
    let exponent = ((start.abs().max(last.abs()).to_bits() >> 52) & 0x7ff) as u32;
    let resolution = if exponent > 52 {
        f64::from_bits(u64::from(exponent - 52) << 52)
    } else {
        f64::from_bits(1_u64 << exponent.saturating_sub(1))
    };
    if step.abs() > resolution || (step.abs() == resolution && (start / resolution).fract() == 0.0)
    {
        return Ok(());
    }
    // At rounding boundaries even equal spacing can collapse ties to even.
    // Check the generated sequence instead of rejecting all narrow spacings.
    let mut previous = start;
    for index in 1..points {
        let current = step.mul_add(index as f64, start);
        if (step > 0.0 && current <= previous) || (step < 0.0 && current >= previous) {
            return Err(invalid);
        }
        previous = current;
    }
    Ok(())
}

pub(crate) fn direction(coordinates: &AxisCoordinates, points: usize) -> AxisDirection {
    if points <= 1 {
        return AxisDirection::Unknown;
    }
    match coordinates {
        AxisCoordinates::Uniform { step, .. } if *step > 0.0 => AxisDirection::Ascending,
        AxisCoordinates::Uniform { step, .. } if *step < 0.0 => AxisDirection::Descending,
        AxisCoordinates::Explicit(values) => {
            let ascending = values.windows(2).all(|pair| pair[0] < pair[1]);
            let descending = values.windows(2).all(|pair| pair[0] > pair[1]);
            match (ascending, descending) {
                (true, false) => AxisDirection::Ascending,
                (false, true) => AxisDirection::Descending,
                _ => AxisDirection::Unknown,
            }
        }
        _ => AxisDirection::Unknown,
    }
}

pub(crate) fn coordinate_span(coordinates: &AxisCoordinates, points: usize) -> Option<f64> {
    if points <= 1 {
        return None;
    }
    let span = match coordinates {
        AxisCoordinates::Uniform { step, .. } => step.abs() * (points - 1) as f64,
        AxisCoordinates::Explicit(values) => (values.last()? - values.first()?).abs(),
        AxisCoordinates::Unknown => return None,
    };
    span.is_finite().then_some(span)
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl FrequencyEvidence {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&Option<f64>, &Option<f64>) {
        (&self.observe_frequency_mhz, &self.transmitter_offset_hz)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (Option<f64>, Option<f64>),
    ) -> Result<Self, crate::internal::ModelError> {
        let (observe_frequency_mhz, transmitter_offset_hz) = parts;
        let value = Self {
            observe_frequency_mhz,
            transmitter_offset_hz,
        };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}
