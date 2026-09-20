use super::*;

/// Supported raw acquisition formats.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RawFormat {
    /// Bruker TopSpin raw `fid`/`ser` plus acquisition status parameters.
    BrukerRaw,
    /// JEOL Delta JDF raw acquisition.
    JeolDelta,
    /// Varian raw `fid` plus `procpar`.
    VarianRaw,
}
/// Typed diffusion acquisition facts projected from vendor parameters.
///
/// This value does not contain a fitted diffusion coefficient, an inverse
/// Laplace transform, a derived b-value, or an assumed gradient-shape factor.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffusionAcquisition {
    gradient_axis: usize,
    gradient_parameter: String,
    gradient_pulse_duration_seconds: f64,
    gradient_pulse_duration_parameter: String,
    diffusion_time_seconds: f64,
    diffusion_time_parameter: String,
    recovery_delay_seconds: Option<f64>,
    recovery_delay_parameter: Option<String>,
    gradient_shape: Option<String>,
    gradient_shape_parameter: Option<String>,
}

impl DiffusionAcquisition {
    /// Creates checked diffusion acquisition facts with exact source parameter names.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gradient_axis: usize,
        gradient_parameter: String,
        gradient_pulse_duration_seconds: f64,
        gradient_pulse_duration_parameter: String,
        diffusion_time_seconds: f64,
        diffusion_time_parameter: String,
        recovery_delay: Option<(f64, String)>,
        gradient_shape: Option<(String, String)>,
    ) -> Result<Self, ValidationError> {
        let (recovery_delay_seconds, recovery_delay_parameter) = recovery_delay.unzip();
        let (gradient_shape, gradient_shape_parameter) = gradient_shape.unzip();
        let value = Self {
            gradient_axis,
            gradient_parameter,
            gradient_pulse_duration_seconds,
            gradient_pulse_duration_parameter,
            diffusion_time_seconds,
            diffusion_time_parameter,
            recovery_delay_seconds,
            recovery_delay_parameter,
            gradient_shape,
            gradient_shape_parameter,
        };
        value.validate()?;
        Ok(value)
    }
    /// Returns the zero-based gradient parameter axis.
    pub fn gradient_axis(&self) -> usize {
        self.gradient_axis
    }
    /// Returns the vendor parameter supplying gradient coordinates.
    pub fn gradient_parameter(&self) -> &str {
        &self.gradient_parameter
    }
    /// Returns the gradient pulse duration in seconds.
    pub fn gradient_pulse_duration_seconds(&self) -> f64 {
        self.gradient_pulse_duration_seconds
    }
    /// Returns the vendor parameter supplying the gradient pulse duration.
    pub fn gradient_pulse_duration_parameter(&self) -> &str {
        &self.gradient_pulse_duration_parameter
    }
    /// Returns the diffusion interval in seconds.
    pub fn diffusion_time_seconds(&self) -> f64 {
        self.diffusion_time_seconds
    }
    /// Returns the vendor parameter supplying the diffusion interval.
    pub fn diffusion_time_parameter(&self) -> &str {
        &self.diffusion_time_parameter
    }
    /// Returns an additional recovery delay in seconds when typed evidence exists.
    pub fn recovery_delay_seconds(&self) -> Option<f64> {
        self.recovery_delay_seconds
    }
    /// Returns the vendor parameter supplying the recovery delay.
    pub fn recovery_delay_parameter(&self) -> Option<&str> {
        self.recovery_delay_parameter.as_deref()
    }
    /// Returns the source gradient-shape label without assigning a coefficient.
    pub fn gradient_shape(&self) -> Option<&str> {
        self.gradient_shape.as_deref()
    }
    /// Returns the vendor parameter supplying the gradient-shape label.
    pub fn gradient_shape_parameter(&self) -> Option<&str> {
        self.gradient_shape_parameter.as_deref()
    }

    fn validate(&self) -> Result<(), ValidationError> {
        if !self.gradient_pulse_duration_seconds.is_finite()
            || self.gradient_pulse_duration_seconds <= 0.0
            || !self.diffusion_time_seconds.is_finite()
            || self.diffusion_time_seconds <= 0.0
            || self
                .recovery_delay_seconds
                .is_some_and(|v| !v.is_finite() || v < 0.0)
        {
            return Err(ValidationError::InvalidNumber(
                "diffusion acquisition metadata",
            ));
        }
        for text in [
            Some(self.gradient_parameter.as_str()),
            Some(self.gradient_pulse_duration_parameter.as_str()),
            Some(self.diffusion_time_parameter.as_str()),
            self.recovery_delay_parameter.as_deref(),
            self.gradient_shape.as_deref(),
            self.gradient_shape_parameter.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if text.trim().is_empty() {
                return Err(ValidationError::EmptySourceParameter);
            }
        }
        if self.recovery_delay_seconds.is_some() != self.recovery_delay_parameter.is_some()
            || self.gradient_shape.is_some() != self.gradient_shape_parameter.is_some()
        {
            return Err(ValidationError::IncompleteSourceEvidence);
        }
        Ok(())
    }
}
/// Common acquisition metadata.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawMetadata {
    pub(crate) title: Option<String>,
    pub(crate) solvent: Option<String>,
    pub(crate) temperature_kelvin: Option<f64>,
    pub(crate) scans: Option<u64>,
    pub(crate) pulse_program: Option<String>,
    pub(crate) diffusion: Option<DiffusionAcquisition>,
}
impl RawMetadata {
    /// Creates checked portable acquisition metadata.
    pub fn new(
        title: Option<String>,
        solvent: Option<String>,
        temperature_kelvin: Option<f64>,
        scans: Option<u64>,
        pulse_program: Option<String>,
    ) -> Result<Self, ValidationError> {
        let value = Self {
            title: normalize_text(title),
            solvent: normalize_text(solvent),
            temperature_kelvin,
            scans,
            pulse_program: normalize_text(pulse_program),
            diffusion: None,
        };
        value.validate()?;
        Ok(value)
    }
    /// Returns the experiment title when supplied by the vendor metadata.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    /// Returns the solvent label when known.
    pub fn solvent(&self) -> Option<&str> {
        self.solvent.as_deref()
    }
    /// Returns the finite non-negative sample temperature in kelvin.
    pub fn temperature_kelvin(&self) -> Option<f64> {
        self.temperature_kelvin
    }
    /// Returns the positive acquisition scan count without normalizing samples.
    pub fn scans(&self) -> Option<u64> {
        self.scans
    }
    /// Returns the vendor pulse-program or sequence name when known.
    pub fn pulse_program(&self) -> Option<&str> {
        self.pulse_program.as_deref()
    }
    /// Adds typed diffusion acquisition facts without deriving analysis results.
    pub fn with_diffusion(
        mut self,
        diffusion: Option<DiffusionAcquisition>,
    ) -> Result<Self, ValidationError> {
        self.diffusion = diffusion;
        self.validate()?;
        Ok(self)
    }
    /// Returns typed diffusion acquisition facts when source evidence is complete.
    pub fn diffusion(&self) -> Option<&DiffusionAcquisition> {
        self.diffusion.as_ref()
    }
    /// Validates numeric acquisition metadata.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self
            .temperature_kelvin
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(ValidationError::InvalidNumber("temperature"));
        }
        if self.scans == Some(0) {
            return Err(ValidationError::InvalidNumber("scans"));
        }
        if let Some(diffusion) = &self.diffusion {
            diffusion.validate()?;
        }
        Ok(())
    }
}

pub(super) fn normalize_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}
/// Opaque vendor metadata retained alongside canonical data.
#[derive(Clone, Debug, PartialEq)]
pub struct VendorMetadata(VendorMetadataInner);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum VendorMetadataInner {
    Bruker(bruker::Parameters),
    Jeol(jeol::Parameters),
    Varian(varian::Parameters),
    None,
}

impl VendorMetadata {
    /// Creates empty metadata for a caller-constructed acquisition.
    pub fn none() -> Self {
        Self(VendorMetadataInner::None)
    }

    /// Returns Bruker metadata when the input was a Bruker acquisition.
    pub fn as_bruker(&self) -> Option<&bruker::Parameters> {
        match &self.0 {
            VendorMetadataInner::Bruker(value) => Some(value),
            _ => None,
        }
    }

    /// Returns JEOL metadata when the input was a JEOL acquisition.
    pub fn as_jeol(&self) -> Option<&jeol::Parameters> {
        match &self.0 {
            VendorMetadataInner::Jeol(value) => Some(value),
            _ => None,
        }
    }

    /// Returns Varian metadata when the input was a Varian acquisition.
    pub fn as_varian(&self) -> Option<&varian::Parameters> {
        match &self.0 {
            VendorMetadataInner::Varian(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn bruker(value: bruker::Parameters) -> Self {
        Self(VendorMetadataInner::Bruker(value))
    }

    pub(crate) fn jeol(value: jeol::Parameters) -> Self {
        Self(VendorMetadataInner::Jeol(value))
    }

    pub(crate) fn varian(value: varian::Parameters) -> Self {
        Self(VendorMetadataInner::Varian(value))
    }

    pub(super) fn sample_normalization(&self) -> crate::provenance::SampleNormalization {
        match &self.0 {
            VendorMetadataInner::Varian(parameters) => {
                let factors = if parameters.block_headers().is_empty() {
                    vec![1.0]
                } else {
                    parameters
                        .block_headers()
                        .iter()
                        .map(|header| 2f64.powi(i32::from(header.scale())))
                        .collect()
                };
                let stored_imaginary_multiplier = parameters.file_header().map_or(1, |header| {
                    let complex = if header.version() == 0 {
                        header.status() & 0x40 != 0
                    } else {
                        header.status() & 0x10 != 0
                    };
                    if complex { -1 } else { 1 }
                });
                crate::provenance::SampleNormalization::reader(factors, stored_imaginary_multiplier)
            }
            VendorMetadataInner::Jeol(parameters) => {
                crate::provenance::SampleNormalization::reader(
                    vec![1.0],
                    parameters.sample_transform().direct_imaginary_multiplier(),
                )
            }
            VendorMetadataInner::Bruker(_) | VendorMetadataInner::None => {
                crate::provenance::SampleNormalization::identity()
            }
        }
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl DiffusionAcquisition {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &usize,
        &String,
        &f64,
        &String,
        &f64,
        &String,
        &Option<f64>,
        &Option<String>,
        &Option<String>,
        &Option<String>,
    ) {
        (
            &self.gradient_axis,
            &self.gradient_parameter,
            &self.gradient_pulse_duration_seconds,
            &self.gradient_pulse_duration_parameter,
            &self.diffusion_time_seconds,
            &self.diffusion_time_parameter,
            &self.recovery_delay_seconds,
            &self.recovery_delay_parameter,
            &self.gradient_shape,
            &self.gradient_shape_parameter,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            usize,
            String,
            f64,
            String,
            f64,
            String,
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            gradient_axis,
            gradient_parameter,
            gradient_pulse_duration_seconds,
            gradient_pulse_duration_parameter,
            diffusion_time_seconds,
            diffusion_time_parameter,
            recovery_delay_seconds,
            recovery_delay_parameter,
            gradient_shape,
            gradient_shape_parameter,
        ) = parts;
        let value = Self {
            gradient_axis,
            gradient_parameter,
            gradient_pulse_duration_seconds,
            gradient_pulse_duration_parameter,
            diffusion_time_seconds,
            diffusion_time_parameter,
            recovery_delay_seconds,
            recovery_delay_parameter,
            gradient_shape,
            gradient_shape_parameter,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl RawMetadata {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &Option<String>,
        &Option<String>,
        &Option<f64>,
        &Option<u64>,
        &Option<String>,
        &Option<DiffusionAcquisition>,
    ) {
        (
            &self.title,
            &self.solvent,
            &self.temperature_kelvin,
            &self.scans,
            &self.pulse_program,
            &self.diffusion,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            Option<String>,
            Option<String>,
            Option<f64>,
            Option<u64>,
            Option<String>,
            Option<DiffusionAcquisition>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (title, solvent, temperature_kelvin, scans, pulse_program, diffusion) = parts;
        let value = Self {
            title,
            solvent,
            temperature_kelvin,
            scans,
            pulse_program,
            diffusion,
        };

        value
            .validate()
            .map_err(|e| crate::internal::ModelError::Validation(e.to_string()))?;

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl VendorMetadata {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(&self) -> (&VendorMetadataInner,) {
        (&self.0,)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (VendorMetadataInner,),
    ) -> Result<Self, crate::internal::ModelError> {
        let (f0,) = parts;
        let value = Self(f0);

        Ok(value)
    }
}
