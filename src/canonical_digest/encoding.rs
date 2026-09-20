use crate::acquisition::*;
use crate::axis::AxisCoordinates;
use crate::axis::AxisDomain;
use crate::axis::AxisQuantity;
use crate::axis::AxisUnit;
use crate::execution::CancellationToken;
use crate::execution::ExecutionError;
use sha2::{Digest, Sha256};

use super::CanonicalDigest;

pub(super) fn encode_axis(
    out: &mut Encoder<'_>,
    axis: &crate::raw::RawAxis,
) -> Result<(), ExecutionError> {
    match axis.kind() {
        RawAxisKind::Direct(DirectSamples::Real) => out.u8(0),
        RawAxisKind::Direct(DirectSamples::Complex) => out.u8(1),
        RawAxisKind::Indirect(IndirectComponents::Scalar) => out.u8(2),
        RawAxisKind::Indirect(IndirectComponents::Cartesian(evidence)) => {
            out.u8(3);
            encode_evidence(out, evidence.evidence())?;
        }
        RawAxisKind::Indirect(IndirectComponents::Encoded(transform)) => {
            out.u8(4);
            encode_transform(out, transform)?;
        }
        RawAxisKind::Parameter => out.u8(5),
        RawAxisKind::Indirect(IndirectComponents::SharedComplex {
            conjugated,
            evidence,
        }) => {
            out.u8(6);
            out.u8(u8::from(*conjugated));
            encode_evidence(out, evidence.evidence())?;
        }
    }
    out.u8(match axis.domain() {
        AxisDomain::Time => 0,
        AxisDomain::Frequency => 1,
        AxisDomain::Parameter => 2,
        AxisDomain::Unknown => 3,
    });
    out.option_u8(axis.unit().map(|value| match value {
        AxisUnit::Second => 0,
        AxisUnit::Hertz => 1,
        AxisUnit::Ppm => 2,
        AxisUnit::Tesla => 3,
        AxisUnit::TeslaPerMeter => 4,
    }));
    out.option_u8(axis.quantity().map(|value| match value {
        AxisQuantity::MagneticFieldGradientStrength => 0,
        AxisQuantity::TimeDelay => 1,
    }));
    out.usize(axis.points());
    match axis.coordinates() {
        AxisCoordinates::Unknown => out.u8(0),
        AxisCoordinates::Uniform { start, step } => {
            out.u8(1);
            out.f64(*start);
            out.f64(*step);
        }
        AxisCoordinates::Explicit(values) => {
            out.u8(2);
            out.usize(values.len());
            for &value in values {
                out.check()?;
                out.f64(value);
            }
        }
    }
    out.option_str(axis.nucleus());
    out.option_f64(axis.spectral_width_hz());
    match axis.frequency_evidence() {
        None => out.u8(0),
        Some(value) => {
            out.u8(1);
            out.option_f64(value.observe_frequency_mhz());
            out.option_f64(value.transmitter_offset_hz());
        }
    }
    match axis.chemical_shift_reference() {
        None => out.u8(0),
        Some(value) => {
            out.u8(1);
            out.f64(value.carrier_ppm());
            out.f64(value.reference_frequency_mhz());
            encode_evidence(out, value.evidence())?;
        }
    }
    match axis.group_delay() {
        GroupDelayState::NotApplicable => out.u8(0),
        GroupDelayState::Unknown => out.u8(1),
        GroupDelayState::Pending(value) => {
            out.u8(2);
            out.f64(value.delay_points());
            encode_evidence(out, value.evidence())?;
        }
    }
    out.check()
}

pub(super) fn encode_transform(
    out: &mut Encoder<'_>,
    value: &ResolvedComponentTransform,
) -> Result<(), ExecutionError> {
    let transform = value.transform();
    out.usize(transform.input_lanes());
    for &coefficient in transform.coefficients() {
        out.check()?;
        out.complex(coefficient);
    }
    let modulation = transform.modulation();
    out.usize(modulation.period());
    for &multiplier in modulation.multipliers() {
        out.check()?;
        out.complex(multiplier);
    }
    match modulation.domain() {
        ModulationIndexDomain::AbsoluteGridCoordinate(axis) => {
            out.u8(0);
            out.usize(axis.index());
        }
        ModulationIndexDomain::ObservationOrdinal => out.u8(1),
    }
    out.i64(modulation.origin());
    encode_evidence(out, value.evidence())?;
    out.check()
}

pub(super) fn encode_evidence(
    out: &mut Encoder<'_>,
    value: &NormalizationEvidence,
) -> Result<(), ExecutionError> {
    match value.authority() {
        ResolutionAuthority::FormatRule(id) => {
            out.u8(0);
            out.str(id.as_str());
        }
        ResolutionAuthority::CallerAssertion(id) => {
            out.u8(1);
            out.str(id.as_str());
        }
        ResolutionAuthority::UserConstructed => out.u8(2),
    }
    out.usize(value.facts().len());
    for fact in value.facts() {
        out.check()?;
        match fact {
            NormalizationFact::CanonicalLaneOrder { lanes } => {
                out.u8(0);
                out.usize(lanes.get());
            }
            NormalizationFact::PublicComplexConvention => out.u8(1),
            NormalizationFact::TraceMappingBijection => out.u8(2),
            NormalizationFact::SourceCoefficients { active, values } => {
                out.u8(3);
                out.u8(u8::from(*active));
                out.usize(*values);
            }
            NormalizationFact::PeriodicModulation { period, domain } => {
                out.u8(4);
                out.usize(period.get());
                match domain {
                    ModulationIndexDomain::AbsoluteGridCoordinate(axis) => {
                        out.u8(0);
                        out.usize(axis.index());
                    }
                    ModulationIndexDomain::ObservationOrdinal => out.u8(1),
                }
            }
            NormalizationFact::DirectSampleEncoding => out.u8(5),
            NormalizationFact::UserDefinedComponentTransform => out.u8(6),
            NormalizationFact::UserDefinedCartesianNormalization => out.u8(7),
            NormalizationFact::DigitalFilterDelay => out.u8(8),
            NormalizationFact::ChemicalShiftReference => out.u8(9),
        }
    }
    out.usize(value.derivation().len());
    for step in value.derivation() {
        out.check()?;
        out.str(step.as_str());
    }
    out.check()
}

pub(super) struct Encoder<'a> {
    hash: Sha256,
    cancellation: Option<&'a CancellationToken>,
}
impl<'a> Encoder<'a> {
    #[cfg(test)]
    pub(super) fn new(protocol: &[u8]) -> Self {
        Self::new_checked(protocol, None)
    }
    pub(super) fn new_checked(
        protocol: &[u8],
        cancellation: Option<&'a CancellationToken>,
    ) -> Self {
        let mut value = Self {
            hash: Sha256::new(),
            cancellation,
        };
        value.bytes(protocol);
        value
    }
    pub(super) fn finish(self) -> CanonicalDigest {
        CanonicalDigest::new(self.hash.finalize().into())
    }
    pub(super) fn check(&self) -> Result<(), ExecutionError> {
        self.cancellation.map_or(Ok(()), CancellationToken::check)
    }
    pub(super) fn finish_checked(self) -> Result<CanonicalDigest, ExecutionError> {
        self.check()?;
        Ok(self.finish())
    }
    pub(super) fn bytes(&mut self, value: &[u8]) {
        for chunk in value.chunks(32768) {
            if self.check().is_err() {
                return;
            }
            self.hash.update(chunk);
        }
    }
    pub(super) fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }
    pub(super) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }
    pub(super) fn i64(&mut self, value: i64) {
        self.bytes(&value.to_le_bytes());
    }
    pub(super) fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }
    pub(super) fn f64(&mut self, value: f64) {
        self.u64(value.to_bits());
    }
    pub(super) fn complex(&mut self, value: crate::Complex64) {
        self.f64(value.re);
        self.f64(value.im);
    }
    pub(super) fn str(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes(value.as_bytes());
    }
    pub(super) fn option_str(&mut self, value: Option<&str>) {
        match value {
            None => self.u8(0),
            Some(value) => {
                self.u8(1);
                self.str(value);
            }
        }
    }
    pub(super) fn option_f64(&mut self, value: Option<f64>) {
        match value {
            None => self.u8(0),
            Some(value) => {
                self.u8(1);
                self.f64(value);
            }
        }
    }
    pub(super) fn option_u64(&mut self, value: Option<u64>) {
        match value {
            None => self.u8(0),
            Some(value) => {
                self.u8(1);
                self.u64(value);
            }
        }
    }
    pub(super) fn option_u8(&mut self, value: Option<u8>) {
        match value {
            None => self.u8(0),
            Some(value) => {
                self.u8(1);
                self.u8(value);
            }
        }
    }
}
