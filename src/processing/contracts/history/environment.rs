use crate::provenance::CanonicalDatasetDigests;

/// One completed execution segment. Digests describe this execution's output.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionSegment {
    end_record: usize,
    output: CanonicalDatasetDigests,
    environment: ExecutionEnvironment,
    accepted_archive: bool,
}

/// Available execution facts; unavailable build and dependency details remain unknown.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionEnvironment {
    crate_version: String,
    architecture: String,
    operating_system: String,
    numerical_dependencies: Option<String>,
    explicit_fft_backend: String,
    build_identifier: Option<String>,
    floating_point_configuration: Option<String>,
}

impl ExecutionEnvironment {
    pub(crate) fn current() -> Self {
        Self {
            crate_version: env!("CARGO_PKG_VERSION").into(),
            architecture: std::env::consts::ARCH.into(),
            operating_system: std::env::consts::OS.into(),
            numerical_dependencies: Some("rustfft=6.4.1".into()),
            explicit_fft_backend: crate::processing::kernels::fft::BACKEND.into(),
            build_identifier: Some(env!("NMR_BUILD_SHA256").into()),
            floating_point_configuration: None,
        }
    }

    /// nmr package version used for this execution.
    pub fn crate_version(&self) -> &str {
        &self.crate_version
    }
    /// Target architecture; this does not identify enabled CPU features.
    pub fn architecture(&self) -> &str {
        &self.architecture
    }
    /// Target operating system.
    pub fn operating_system(&self) -> &str {
        &self.operating_system
    }
    /// SHA-256 of source inputs, compiler identity and captured build configuration.
    pub fn build_identifier(&self) -> Option<&str> {
        self.build_identifier.as_deref()
    }
    /// Returns captured exact dependency versions when available; currently RustFFT only.
    pub fn numerical_dependency_versions(&self) -> Option<&str> {
        self.numerical_dependencies.as_deref()
    }
    /// Returns the configured explicit-processing FFT backend; a segment may not use FFT.
    pub fn explicit_fft_backend(&self) -> &str {
        &self.explicit_fft_backend
    }
    /// Complete CPU-feature and floating-point configuration is not captured.
    pub fn floating_point_configuration(&self) -> Option<&str> {
        self.floating_point_configuration.as_deref()
    }
}

impl ExecutionSegment {
    /// Whether this execution fact was accepted from an archive.
    pub fn accepted_archive(&self) -> bool {
        self.accepted_archive
    }
    /// Exclusive record index ending this segment.
    pub fn end_record(&self) -> usize {
        self.end_record
    }
    /// Actual canonical output of this segment, not a tolerance assertion.
    pub fn output_digests(&self) -> CanonicalDatasetDigests {
        self.output
    }
    /// Available execution environment facts.
    pub fn environment(&self) -> &ExecutionEnvironment {
        &self.environment
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ExecutionEnvironment {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &String,
        &String,
        &String,
        &Option<String>,
        &String,
        &Option<String>,
        &Option<String>,
    ) {
        (
            &self.crate_version,
            &self.architecture,
            &self.operating_system,
            &self.numerical_dependencies,
            &self.explicit_fft_backend,
            &self.build_identifier,
            &self.floating_point_configuration,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (
            String,
            String,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
        ),
    ) -> Result<Self, crate::internal::ModelError> {
        let (
            crate_version,
            architecture,
            operating_system,
            numerical_dependencies,
            explicit_fft_backend,
            build_identifier,
            floating_point_configuration,
        ) = parts;
        let value = Self {
            crate_version,
            architecture,
            operating_system,
            numerical_dependencies,
            explicit_fft_backend,
            build_identifier,
            floating_point_configuration,
        };

        Ok(value)
    }
}

// Crate-private model decomposition; no wire tags or encoding policy.
impl ExecutionSegment {
    #[allow(clippy::type_complexity)]
    pub(crate) fn model_parts(
        &self,
    ) -> (
        &usize,
        &CanonicalDatasetDigests,
        &ExecutionEnvironment,
        &bool,
    ) {
        (
            &self.end_record,
            &self.output,
            &self.environment,
            &self.accepted_archive,
        )
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn from_model_parts(
        parts: (usize, CanonicalDatasetDigests, ExecutionEnvironment, bool),
    ) -> Result<Self, crate::internal::ModelError> {
        let (end_record, output, environment, accepted_archive) = parts;
        let value = Self {
            end_record,
            output,
            environment,
            accepted_archive,
        };

        Ok(value)
    }
}
impl ExecutionSegment {
    pub(super) fn new(end_record: usize, output: CanonicalDatasetDigests) -> Self {
        Self {
            accepted_archive: false,
            end_record,
            output,
            environment: ExecutionEnvironment::current(),
        }
    }
    pub(super) fn accept_archive(&mut self) {
        self.accepted_archive = true;
    }
}
