use crate::acquisition::*;

use crate::axis::*;

use crate::processed::{ProcessedAxis, ProcessedDataset, ProcessedDescriptor, ProcessedOrigin};

use crate::processing::contracts::history::{
    ExecutionEnvironment, ExecutionSegment, HistoryInput, ProcessingHistory, ProcessingRecord,
};

use crate::processing::contracts::operation::*;

use crate::provenance::*;

use crate::reference::PolarityState;

use super::json::*;
use super::{ExternalAlgorithmDeclaration, REPORT_SCHEMA, ReportError};

macro_rules! tags { ($ty:ty, $($variant:path => $tag:literal),* $(,)?) => { impl Encode for $ty { fn encode(&self,j:&mut Json<'_>)->Result<(),ReportError>{ match self { $($variant => $tag),* }.encode(j) } } }; }
tags!(AxisRole, AxisRole::DirectAcquisition=>"direct-acquisition", AxisRole::IndirectAcquisition=>"indirect-acquisition", AxisRole::ArrayParameter=>"array-parameter", AxisRole::Signal=>"signal", AxisRole::Unknown=>"unknown");
tags!(AxisDomain, AxisDomain::Time=>"time", AxisDomain::Frequency=>"frequency", AxisDomain::Parameter=>"parameter", AxisDomain::Unknown=>"unknown");
tags!(AxisUnit, AxisUnit::Second=>"s", AxisUnit::Hertz=>"Hz", AxisUnit::Ppm=>"ppm", AxisUnit::Tesla=>"T", AxisUnit::TeslaPerMeter=>"T/m");
tags!(AxisQuantity, AxisQuantity::MagneticFieldGradientStrength=>"magnetic-field-gradient-strength", AxisQuantity::TimeDelay=>"time-delay");
tags!(SourceKind, SourceKind::Data=>"data", SourceKind::Parameters=>"parameters", SourceKind::SamplingSchedule=>"sampling-schedule", SourceKind::Other=>"other");
tags!(crate::raw::RawFormat, crate::raw::RawFormat::BrukerRaw=>"bruker-raw", crate::raw::RawFormat::JeolDelta=>"jeol-delta-raw", crate::raw::RawFormat::VarianRaw=>"varian-raw");
tags!(FourierExponentSign, FourierExponentSign::Negative=>"negative", FourierExponentSign::Positive=>"positive");
tags!(PolarityState, PolarityState::Ambiguous180=>"ambiguous-180", PolarityState::UserAssertedPositive=>"user-asserted-positive");
tags!(Projection, Projection::Real=>"real", Projection::RealAbsorptive=>"real-absorptive", Projection::RealSigned=>"real-signed", Projection::Magnitude=>"magnitude", Projection::UnphasedReal=>"unphased-real");
tags!(TimeDomainResidualPolicy, TimeDomainResidualPolicy::CorrectFully=>"correct-fully", TimeDomainResidualPolicy::IntegerOnlyRetainResidual=>"integer-only-retain-residual");
tags!(PhaseFailurePolicy, PhaseFailurePolicy::Fail=>"fail", PhaseFailurePolicy::ContinueUnphasedReal=>"continue-unphased-real");
tags!(AttemptFailure, AttemptFailure::NoUsableSignal=>"no-usable-signal", AttemptFailure::ObjectiveUndefined=>"objective-undefined", AttemptFailure::OptimizationDidNotConverge=>"optimization-did-not-converge");
tags!(ProcessingDiagnostic, ProcessingDiagnostic::ContinuedAfterPhaseFailure=>"continued-after-phase-failure", ProcessingDiagnostic::AmbiguousPolarity=>"ambiguous-polarity", ProcessingDiagnostic::UnknownCoordinateQuality=>"unknown-coordinate-quality");
impl Encode for CanonicalDigest {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        j.raw("\"")?;
        for b in self.as_bytes() {
            j.raw(&format!("{b:02x}"))?;
        }
        j.raw("\"")
    }
}
impl Encode for CanonicalDatasetDigests {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "sha256_descriptor" => self.descriptor(),
            "sha256_samples" => self.samples(),
            "sha256_dataset" => self.dataset()
        )
    }
}
impl Encode for SourceDigest {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::NotComputed => j.raw("null"),
            Self::Sha256(bytes) => {
                j.raw("\"")?;
                for b in bytes {
                    j.raw(&format!("{b:02x}"))?;
                }
                j.raw("\"")
            }
        }
    }
}
impl Encode for SourceFile {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "kind" => self.kind(),
            "role" => self.role(),
            "sha256" => self.digest(),
            "locator" => self.locator().and_then(|p|p.to_str()),
            "byte_length" => self.identity().map(|v|v.length())
        )
    }
}
impl Encode for AxisCoordinates {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Unknown => object!(j,"kind"=>"unknown"),
            Self::Uniform { start, step } => {
                object!(j,"kind"=>"uniform","start"=>start,"step"=>step,"evaluation"=>"step.mul_add(index,start)")
            }
            Self::Explicit(values) => object!(j,"kind"=>"explicit","values"=>values),
        }
    }
}
impl Encode for ComponentBasis {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::SharedComplex { axis, conjugated } => {
                object!(j,"kind"=>"shared-complex","axis"=>axis.index(),"conjugated"=>conjugated)
            }
            Self::Scalar => object!(j,"kind"=>"scalar"),
            Self::Cartesian => object!(j,"kind"=>"cartesian"),
            Self::Encoded(transform) => object!(j,"kind"=>"encoded","transform"=>transform),
        }
    }
}
impl Encode for FrequencyEvidence {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "observe_frequency_mhz" => self.observe_frequency_mhz(),
            "transmitter_offset_hz" => self.transmitter_offset_hz()
        )
    }
}
impl Encode for ProcessedAxis {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "role" => self.role(),
            "domain" => self.domain(),
            "unit" => self.unit(),
            "quantity" => self.quantity(),
            "points" => self.points(),
            "coordinates" => self.coordinates(),
            "components" => self.component_basis(),
            "nucleus" => self.nucleus(),
            "label" => self.label(),
            "spectral_width_hz" => self.spectral_width_hz(),
            "frequency_evidence" => self.frequency_evidence(),
            "spectrum_reference" => self.spectrum_reference()
        )
    }
}
impl Encode for crate::processed::SpectrumReference {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j, "reference_frequency_mhz" => self.reference_frequency_mhz(), "evidence" => self.evidence())
    }
}
impl Encode for ProcessedDescriptor {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"axes"=>self.axes(),"storage"=>"logical-row-major-component-inner")
    }
}
impl Encode for crate::Complex64 {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"real"=>self.re,"imaginary"=>self.im)
    }
}
impl Encode for ResolutionAuthority {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::FormatRule(rule) => object!(j,"kind"=>"format-rule","id"=>rule.as_str()),
            Self::CallerAssertion(id) => object!(j,"kind"=>"caller-assertion","id"=>id.as_str()),
            Self::UserConstructed => object!(j,"kind"=>"user-constructed"),
        }
    }
}
impl Encode for ModulationIndexDomain {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::AbsoluteGridCoordinate(axis) => {
                object!(j,"kind"=>"absolute-grid-coordinate","axis"=>axis.index())
            }
            Self::ObservationOrdinal => object!(j,"kind"=>"observation-ordinal"),
        }
    }
}
impl Encode for NormalizationFact {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::CanonicalLaneOrder { lanes } => {
                object!(j,"kind"=>"canonical-lane-order","lanes"=>lanes.get())
            }
            Self::PublicComplexConvention => object!(j,"kind"=>"public-r-plus-iI"),
            Self::TraceMappingBijection => object!(j,"kind"=>"trace-mapping-bijection"),
            Self::SourceCoefficients { active, values } => {
                object!(j,"kind"=>"source-coefficients","active"=>active,"values"=>values)
            }
            Self::PeriodicModulation { period, domain } => {
                object!(j,"kind"=>"periodic-modulation","period"=>period.get(),"domain"=>domain)
            }
            Self::DirectSampleEncoding => object!(j,"kind"=>"direct-sample-encoding"),
            Self::UserDefinedComponentTransform => {
                object!(j,"kind"=>"user-defined-component-transform")
            }
            Self::UserDefinedCartesianNormalization => {
                object!(j,"kind"=>"user-defined-cartesian-normalization")
            }
            Self::DigitalFilterDelay => object!(j,"kind"=>"digital-filter-delay"),
            Self::ChemicalShiftReference => object!(j,"kind"=>"chemical-shift-reference"),
        }
    }
}
impl Encode for DerivationStepId {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        self.as_str().encode(j)
    }
}
impl Encode for NormalizationEvidence {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"authority"=>self.authority(),"facts"=>self.facts(),"derivation_ids"=>self.derivation())
    }
}
impl Encode for ResolvedComponentTransform {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        let t = self.transform();
        let m = t.modulation();
        object!(
            j,
            "input_lanes" => self.input_lanes(),
            "row_major_coefficients" => t.coefficients(),
            "modulation_phase_major_multipliers" => m.multipliers(),
            "modulation_period" => m.period(),
            "modulation_domain" => m.domain(),
            "modulation_origin" => m.origin(),
            "evidence" => self.evidence()
        )
    }
}
impl Encode for ChemicalShiftReference {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "carrier_ppm" => self.carrier_ppm(),
            "reference_frequency_mhz" => self.reference_frequency_mhz(),
            "evidence" => self.evidence()
        )
    }
}
impl Encode for Window {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::LorentzToGauss { lb_hz, gb_hz } => {
                object!(j,"kind"=>"lorentz-to-gauss","lb_hz"=>lb_hz,"gb_hz"=>gb_hz)
            }
            Self::Exponential { lb_hz } => object!(j,"kind"=>"exponential","lb_hz"=>lb_hz),
            Self::SineBell {
                offset,
                end,
                power,
                first_point_scale,
            } => {
                object!(
                    j,
                    "kind" => "sine-bell",
                    "offset" => offset,
                    "end" => end,
                    "power" => power,
                    "first_point_scale" => first_point_scale
                )
            }
        }
    }
}
impl Encode for PhaseCorrection {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "p0_degrees" => self.p0_degrees(),
            "p1_degrees" => self.p1_degrees(),
            "pivot_fraction" => self.pivot_fraction(),
            "phase_grid" => "k/N"
        )
    }
}
impl Encode for DelaySource {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::AxisEvidence => object!(j,"kind"=>"axis-evidence"),
            Self::Explicit(points) => object!(j,"kind"=>"explicit","points"=>points),
        }
    }
}
impl Encode for DigitalFilterCorrection {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::AcknowledgeZeroDelayV1 => object!(j,"kind"=>"acknowledge-zero-delay-v1"),
            Self::FrequencyDomainPhaseRampV1(source) => {
                object!(j,"kind"=>"frequency-domain-phase-ramp-v1","source"=>source)
            }
            Self::TimeDomainShiftFoldV1 { source, policy } => {
                object!(j,"kind"=>"time-domain-shift-fold-v1","source"=>source,"policy"=>policy)
            }
        }
    }
}
impl Encode for FrequencyFrame {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Hertz => object!(j,"kind"=>"hertz"),
            Self::Ppm(source) => object!(j,"kind"=>"ppm","reference"=>source),
        }
    }
}
impl Encode for ReferenceSource {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::AxisEvidence => object!(j,"kind"=>"axis-evidence"),
            Self::Explicit(reference) => object!(j,"kind"=>"explicit","reference"=>reference),
        }
    }
}
impl Encode for BaselineProfile {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::PositivePeaksV1(_) => {
                object!(
                    j,
                    "kind" => "positive-peaks-v1",
                    "algorithm_id" => "asls.v1",
                    "lambda_at_2048_intervals" => crate::processing::contracts::profile::PositivePeaksV1::LAMBDA,
                    "grid_lambda_scaling" => "((n-1)/2048)^4",
                    "asymmetry" => crate::processing::contracts::profile::PositivePeaksV1::ASYMMETRY,
                    "max_solves" => crate::processing::contracts::profile::PositivePeaksV1::MAX_SOLVES,
                    "relative_l2_tolerance" => crate::processing::contracts::profile::PositivePeaksV1::TOLERANCE,
                    "scope" => "experimental"
                )
            }
        }
    }
}
impl Encode for ProcessingRequest {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::BaselineEstimate { axis, method } => {
                object!(j,"kind"=>"estimated-real-baseline","axis"=>axis,"method"=>crate::processing::SpectrumOperation::Baseline(*method))
            }
            Self::PhaseMethod { axis, method } => {
                object!(j,"kind"=>"automatic-phase-method","axis"=>axis,"method"=>method.algorithm_version())
            }
            Self::Explicit(op) => op.encode(j),
            Self::AutoPhase {
                axis,
                profile: _,
                polarity,
                failure_policy,
            } => {
                object!(
                    j,
                    "kind" => "auto-phase",
                    "axis" => axis,
                    "profile" => "normalized-acme.lorentzian-guard.v1",
                    "polarity" => polarity,
                    "failure_policy" => failure_policy,
                    "scope" => "experimental"
                )
            }
        }
    }
}
impl Encode for ProcessingOperation {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Spectrum { axis, operation } => {
                object!(j,"kind"=>"spectrum","axis"=>axis,"operation"=>operation)
            }
            Self::Window { axis, window } => {
                object!(j,"kind"=>"window","axis"=>axis,"window"=>window)
            }
            Self::ZeroFill { axis, zero_fill } => {
                object!(j,"kind"=>"zero-fill","axis"=>axis,"target_points"=>zero_fill.target_points())
            }
            Self::StandardZeroFill { axis } => object!(j,"kind"=>"standard-zero-fill","axis"=>axis),
            Self::FourierTransform { axis, transform } => {
                object!(j,"kind"=>"fourier-transform","axis"=>axis,"exponent_sign"=>transform.sign())
            }
            Self::DigitalFilterCorrection { axis, correction } => {
                object!(j,"kind"=>"digital-filter-correction","axis"=>axis,"correction"=>correction)
            }
            Self::PhaseCorrection { axis, correction } => {
                object!(j,"kind"=>"phase-correction","axis"=>axis,"correction"=>correction)
            }
            Self::BaselineCorrection { axis, profile } => {
                object!(j,"kind"=>"baseline-correction","axis"=>axis,"profile"=>profile)
            }
            Self::ComponentTransform { axis } => {
                object!(j,"kind"=>"component-transform","axis"=>axis)
            }
            Self::Projection {
                projection,
                polarity,
            } => object!(j,"kind"=>"projection","projection"=>projection,"polarity"=>polarity),
            Self::ResolveFrequencyFrame { axis, frame } => {
                object!(j,"kind"=>"resolve-frequency-frame","axis"=>axis,"frame"=>frame)
            }
            Self::ReverseAxis { axis } => object!(j,"kind"=>"reverse-axis","axis"=>axis),
        }
    }
}
impl Encode for ResolvedOperation {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::EstimatedBaseline {
                values,
                coefficients,
            } => {
                object!(j,"kind"=>"estimated-real-baseline","values"=>values,"coefficients"=>coefficients)
            }
            Self::AutomaticPhase {
                correction,
                objective,
                evaluations,
            } => {
                object!(j,"kind"=>"automatic-phase","correction"=>correction,"objective"=>objective,"evaluations"=>evaluations)
            }
            Self::Spectrum(operation) => object!(j,"kind"=>"spectrum","operation"=>operation),
            Self::AcknowledgeZeroDelayV1 { evidence } => {
                object!(j,"kind"=>"acknowledge-zero-delay-v1","evidence"=>evidence)
            }
            Self::Window(window) => object!(j,"kind"=>"window","window"=>window),
            Self::ZeroFill { target_points } => {
                object!(j,"kind"=>"zero-fill","target_points"=>target_points)
            }
            Self::FourierTransform(sign) => {
                object!(j,"kind"=>"fourier-transform","exponent_sign"=>sign)
            }
            Self::FrequencyDomainPhaseRampV1 { delay, sign } => {
                object!(j,"kind"=>"frequency-domain-phase-ramp-v1","delay_points"=>delay,"exponent_sign"=>sign)
            }
            Self::TimeDomainShiftFoldV1 {
                applied_delay,
                skip,
                fold,
                residual,
            } => {
                object!(
                    j,
                    "kind" => "time-domain-shift-fold-v1",
                    "applied_delay_points" => applied_delay,
                    "skip_points" => skip,
                    "fold_points" => fold,
                    "residual_points" => residual
                )
            }
            Self::PhaseCorrection(phase) => {
                object!(j,"kind"=>"phase-correction","correction"=>phase)
            }
            Self::ComponentTransform {
                transform,
                observation_ordinals,
                grid_origin,
            } => {
                object!(
                    j,
                    "kind" => "component-transform",
                    "transform" => transform,
                    "observation_ordinals" => observation_ordinals.as_deref(),
                    "grid_origin" => grid_origin
                )
            }
            Self::Projection {
                projection,
                polarity,
            } => object!(j,"kind"=>"projection","projection"=>projection,"polarity"=>polarity),
            Self::ResolveFrequencyFrame { frame, reference } => {
                object!(j,"kind"=>"resolve-frequency-frame","frame"=>frame,"reference"=>reference)
            }
            Self::ReverseAxis => object!(j,"kind"=>"reverse-axis"),
            Self::BaselineCorrection(profile) => {
                object!(j,"kind"=>"baseline-correction","profile"=>profile)
            }
        }
    }
}
impl Encode for ProcessingRecord {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Attempted {
                requested,
                failure,
                diagnostics,
            } => {
                object!(j,"status"=>"attempted","requested"=>requested,"failure"=>failure,"diagnostics"=>diagnostics)
            }
            Self::Applied {
                requested,
                resolved,
                input_descriptor,
                output_descriptor,
                algorithm_version,
                accumulation_order,
                diagnostics,
            } => {
                object!(
                    j,
                    "status" => "applied",
                    "requested" => requested,
                    "resolved" => resolved.as_ref(),
                    "algorithm_id" => algorithm_version,
                    "input_descriptor" => input_descriptor,
                    "output_descriptor" => output_descriptor,
                    "accumulation_order" => accumulation_order.map(|_|"lane-ascending"),
                    "diagnostics" => diagnostics
                )
            }
        }
    }
}
impl Encode for SampleNormalization {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "algorithm_id" => self.algorithm_version(),
            "source_block_scale_factors" => self.source_block_scale_factors(),
            "stored_imaginary_multiplier" => self.stored_imaginary_multiplier()
        )
    }
}
impl Encode for ProcessedReadTransform {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Bruker {
                nc_proc,
                float64,
                big_endian,
            } => {
                object!(j,"kind"=>"bruker","nc_proc"=>nc_proc,"float64"=>float64,"big_endian"=>big_endian)
            }
            Self::JcampDx { x_factor, y_factor } => {
                object!(j,"kind"=>"jcamp-dx","x_factor"=>x_factor,"y_factor"=>y_factor)
            }
            Self::JeolDelta2D {
                float64,
                big_endian,
                disk_points,
                crop_start,
                crop_points,
                submatrix_edge,
                imaginary_multiplier,
            } => {
                object!(j,"kind"=>"jeol-delta-2d","float64"=>float64,"big_endian"=>big_endian,"disk_points"=>disk_points.as_slice(),"crop_start"=>crop_start.as_slice(),"crop_points"=>crop_points.as_slice(),"submatrix_edge"=>submatrix_edge,"imaginary_multiplier"=>imaginary_multiplier)
            }
            Self::JeolDelta {
                float64,
                big_endian,
                disk_points,
                crop_start,
                crop_points,
                imaginary_multiplier,
                submatrix_edge,
                coordinate_scale,
                explicit_coordinates,
            } => {
                object!(
                    j,
                    "kind" => "jeol-delta",
                    "float64" => float64,
                    "big_endian" => big_endian,
                    "disk_points" => disk_points,
                    "crop_start" => crop_start,
                    "crop_points" => crop_points,
                    "imaginary_multiplier" => imaginary_multiplier,
                    "submatrix_edge" => submatrix_edge,
                    "coordinate_scale" => coordinate_scale,
                    "explicit_coordinates" => explicit_coordinates
                )
            }
        }
    }
}
impl Encode for SourceId {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"kind"=>self.kind(),"ordinal"=>self.ordinal())
    }
}
impl Encode for ProcessedReadRecord {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "algorithm_id" => self.algorithm_version(),
            "transform" => self.transform(),
            "components" => self.components(),
            "component_indices" => self.component_indices(),
            "output_digests" => self.output_digests()
        )
    }
}
impl Encode for HistoryInput {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Raw {
                digests,
                normalization,
                format,
                sources,
            } => {
                object!(
                    j,
                    "kind" => "raw",
                    "digests" => digests,
                    "normalization" => normalization,
                    "format" => format,
                    "sources" => sources
                )
            }
            Self::Processed {
                digests,
                sources,
                read_record,
            } => {
                object!(j,"kind"=>"processed","digests"=>digests,"sources"=>sources,"read_record"=>read_record)
            }
        }
    }
}
impl Encode for InputAxisRef {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"input_slot"=>self.input().index(),"axis"=>self.axis())
    }
}
impl Encode for ExecutionEnvironment {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "crate_version" => self.crate_version(),
            "architecture" => self.architecture(),
            "operating_system" => self.operating_system(),
            "build_sha256" => self.build_identifier(),
            "numerical_dependencies" => self.numerical_dependency_versions(),
            "fft_backend" => self.explicit_fft_backend(),
            "runtime_float_configuration" => self.floating_point_configuration()
        )
    }
}
impl Encode for ExecutionSegment {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "end_record" => self.end_record(),
            "output_digests" => self.output_digests(),
            "environment" => self.environment(),
            "accepted_archive" => self.accepted_archive()
        )
    }
}
impl Encode for ProcessingHistory {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "initial_descriptor" => self.initial_descriptor(),
            "inputs" => self.inputs(),
            "axis_lineage" => self.axis_lineage(),
            "records" => self.records(),
            "segments" => self.segments()
        )
    }
}
impl Encode for ExternalAlgorithmDeclaration {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "authority" => "caller-declaration-unverified",
            "algorithm" => self.algorithm,
            "version" => self.version,
            "statement" => self.statement,
            "parameter_format" => self.parameter_format,
            "parameters" => self.parameters
        )
    }
}
impl Encode for crate::external::ExternalAxisSource {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::New => object!(j, "kind" => "new"),
            Self::Parent(reference) => object!(j, "kind" => "parent", "reference" => reference),
        }
    }
}
impl Encode for crate::external::ExternalBoundary {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        let parent = self.parent();
        let provenance = parent.provenance();
        object!(j, "declaration" => self.declaration(), "axes" => self.axes(),
            "initial_descriptor" => self.descriptor(), "initial_digests" => self.canonical_digests(),
            "parent_descriptor" => parent.descriptor(), "parent_digests" => parent.canonical_digests(),
            "parent_sources" => provenance.sources(), "parent_history" => provenance.history(),
            "parent_library_derivation" => match provenance.origin() {ProcessedOrigin::Library(boundary)=>Some(boundary.as_ref()),_=>None},
            "parent_external_boundary" => match provenance.origin() { ProcessedOrigin::External(boundary) => Some(boundary.as_ref()), _ => None })
    }
}
pub(super) struct BuildIdentity;
impl Encode for crate::derivation::LibraryDerivation {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"algorithm_version"=>self.operation().algorithm_version(),"environment"=>self.environment(),"accepted_archive"=>self.accepted_archive(),"operation"=>self.operation(),"inputs"=>self.inputs(),"output_descriptor"=>self.descriptor(),"output_digests"=>self.canonical_digests())
    }
}
impl Encode for crate::derivation::DerivationOperation {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::LinearCombination { scale } => {
                object!(j,"kind"=>"linear-combination","scale"=>scale)
            }
            Self::NusReconstruction {
                direct_operations,
                direct_resolved,
                settings,
                noise_report,
                iterations,
                column_thresholds,
                column_relative_changes,
            } => {
                object!(j,"kind"=>"nus-reconstruction","direct_operations"=>direct_operations,"direct_resolved"=>direct_resolved,"max_iterations"=>settings.max_iterations,"noise_standard_deviation"=>settings.noise_standard_deviation,"noise_report"=>noise_report.as_ref(),"iterations"=>iterations,"column_thresholds"=>column_thresholds,"column_relative_changes"=>column_relative_changes)
            }
        }
    }
}
impl Encode for crate::derivation::DerivationInput {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Raw { snapshot, metadata } => {
                object!(j,"kind"=>"raw","snapshot"=>snapshot.as_ref(),"metadata"=>metadata)
            }
            Self::Processed(p) => {
                let provenance = p.provenance();
                object!(j,"kind"=>"processed","descriptor"=>p.descriptor(),"digests"=>p.canonical_digests(),"metadata"=>p.metadata(),"sources"=>provenance.sources(),"history"=>provenance.history(),"read_record"=>provenance.read_record(),
                    "library_derivation"=>match provenance.origin() {ProcessedOrigin::Library(b)=>Some(b.as_ref()),_=>None},
                    "external_boundary"=>match provenance.origin() {ProcessedOrigin::External(b)=>Some(b.as_ref()),_=>None})
            }
        }
    }
}
impl Encode for crate::dataset::DatasetMetadata {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        let identity = self.identity();
        object!(j,"subject"=>identity.subject(),"acquisition"=>identity.acquisition(),"source_label"=>identity.source_label(),"warnings"=>self.warnings(),"accepted_archive"=>self.accepted_archive(),"selection_path"=>self.selection().map(|s|s.path().to_string_lossy()))
    }
}
impl Encode for crate::ReadWarning {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::ExperimentalVendorSemantics { format, details } => {
                object!(j,"kind"=>"experimental-vendor-semantics","format"=>format!("{format:?}"),"details"=>details)
            }
            Self::MissingOptionalSource {
                kind,
                role,
                path,
                impact,
            } => {
                object!(j,"kind"=>"missing-optional-source","source_kind"=>kind,"role"=>role,"path"=>path.to_string_lossy(),"impact"=>format!("{impact:?}"))
            }
            Self::MissingMetadata {
                field,
                axis,
                impact,
            } => {
                object!(j,"kind"=>"missing-metadata","field"=>format!("{field:?}"),"axis"=>axis,"impact"=>format!("{impact:?}"))
            }
        }
    }
}
impl Encode for BuildIdentity {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "source_protocol" => "nmr.source-files.v1",
            "source_sha256" => env!("NMR_SOURCE_SHA256"),
            "build_protocol" => "nmr.build-inputs.v1",
            "build_sha256" => env!("NMR_BUILD_SHA256"),
            "packaged_dependency_lock_sha256" => env!("NMR_PACKAGED_LOCK_SHA256"),
            "resolved_transitive_dependency_identity" => Option::<&str>::None,
            "rustc" => env!("NMR_RUSTC_IDENTITY"),
            "target" => env!("NMR_BUILD_TARGET"),
            "profile" => env!("NMR_BUILD_PROFILE"),
            "opt_level" => env!("NMR_BUILD_OPT_LEVEL"),
            "debug" => env!("NMR_BUILD_DEBUG"),
            "compile_target_features" => env!("NMR_BUILD_CARGO_CFG_TARGET_FEATURE"),
            "encoded_rustflags_utf8_hex" => env!("NMR_RUSTFLAGS_UTF8_HEX"),
            "float_storage" => "IEEE-754-binary64",
            "runtime_rounding_denormals_cpu_features" => Option::<&str>::None
        )
    }
}
pub(super) struct Report<'a> {
    pub(super) dataset: &'a ProcessedDataset,
    pub(super) declarations: &'a [ExternalAlgorithmDeclaration],
}
impl Encode for Report<'_> {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        let p = self.dataset.provenance();
        let origin = match p.origin() {
            ProcessedOrigin::Library(_) => "library-derived",
            ProcessedOrigin::Imported => "imported",
            ProcessedOrigin::DeclaredRaw { .. } => "caller-declared-raw",
            ProcessedOrigin::DerivedRaw(_) => "library-derived-raw",
            ProcessedOrigin::Unknown => "unknown",
            ProcessedOrigin::External(_) => "external-derived",
        };
        let origin_axis_lineage = match p.origin() {
            ProcessedOrigin::DerivedRaw(origin) => Some(origin.axis_lineage()),
            ProcessedOrigin::DeclaredRaw { axis_lineage, .. } => Some(axis_lineage.as_slice()),
            _ => None,
        };
        let raw_origin_snapshot = match p.origin() {
            ProcessedOrigin::DerivedRaw(origin) => Some(origin.snapshot()),
            ProcessedOrigin::DeclaredRaw { snapshot, .. } => Some(snapshot.as_ref()),
            _ => None,
        };
        let experimental_vendor_semantics = p.read_record().is_some_and(|record| {
            matches!(
                record.transform(),
                ProcessedReadTransform::JeolDelta { .. }
                    | ProcessedReadTransform::JeolDelta2D { .. }
            )
        }) || p.history().is_some_and(|history| {
            history.inputs().iter().any(|input| {
                matches!(
                    input,
                    HistoryInput::Raw {
                        format: Some(crate::raw::RawFormat::JeolDelta),
                        ..
                    }
                ) || matches!(
                    input,
                    HistoryInput::Raw {
                        format: Some(crate::raw::RawFormat::BrukerRaw),
                        ..
                    }
                ) && raw_origin_snapshot
                    .is_some_and(|snapshot| snapshot.sampling_schedule().is_some())
            })
        });
        object!(
            j,
            "schema" => REPORT_SCHEMA,
            "crate_version" => env!("CARGO_PKG_VERSION"),
            "origin" => origin,
            "known_experimental_vendor_semantics" => experimental_vendor_semantics,
            "build" => BuildIdentity,
            "output_descriptor" => self.dataset.descriptor(),
            "output_digests" => self.dataset.canonical_digests(),
            "sources" => p.sources(),
            "read_record" => p.read_record(),
            "library_history" => p.history(),
            "origin_axis_lineage" => origin_axis_lineage,
            "raw_origin_snapshot" => raw_origin_snapshot,
            "library_derivation" => match p.origin() {ProcessedOrigin::Library(boundary)=>Some(boundary.as_ref()),_=>None},
            "external_boundary" => match p.origin() { ProcessedOrigin::External(boundary) => Some(boundary.as_ref()), _ => None },
            "external_algorithm_declarations" => self.declarations,
            "automatic_json_replay_supported" => false
        )
    }
}
impl Encode for DirectSamples {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Real => "real",
            Self::Complex => "complex",
        }
        .encode(j)
    }
}
impl Encode for RawAxisKind {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Parameter => object!(j,"kind"=>"parameter"),
            Self::Direct(samples) => object!(j,"kind"=>"direct","samples"=>samples),
            Self::Indirect(components) => object!(j,"kind"=>"indirect","components"=>components),
        }
    }
}
impl Encode for IndirectComponents {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::SharedComplex {
                conjugated,
                evidence,
            } => {
                object!(j,"kind"=>"shared-complex","conjugated"=>conjugated,"evidence"=>evidence.evidence())
            }
            Self::Scalar => object!(j,"kind"=>"scalar"),
            Self::Cartesian(evidence) => {
                object!(j,"kind"=>"cartesian","evidence"=>evidence.evidence())
            }
            Self::Encoded(transform) => object!(j,"kind"=>"encoded","transform"=>transform),
        }
    }
}
impl Encode for GroupDelayState {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        match self {
            Self::Unknown => object!(j,"kind"=>"unknown"),
            Self::NotApplicable => object!(j,"kind"=>"not-applicable"),
            Self::Pending(delay) => {
                object!(j,"kind"=>"pending","delay_points"=>delay.delay_points(),"evidence"=>delay.evidence())
            }
        }
    }
}
impl Encode for crate::raw::RawAxis {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "role" => self.role(),
            "domain" => self.domain(),
            "kind" => self.kind(),
            "unit" => self.unit(),
            "quantity" => self.quantity(),
            "points" => self.points(),
            "coordinates" => self.coordinates(),
            "nucleus" => self.nucleus(),
            "label" => self.label(),
            "spectral_width_hz" => self.spectral_width_hz(),
            "frequency_evidence" => self.frequency_evidence(),
            "chemical_shift_reference" => self.chemical_shift_reference(),
            "group_delay" => self.group_delay()
        )
    }
}
impl Encode for crate::raw::SamplingCoordinate {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        self.as_slice().encode(j)
    }
}
impl Encode for crate::raw::SamplingSchedule {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"grid_shape"=>self.grid(),"coordinates_in_acquisition_order"=>self.coordinates(), "caller_declaration" => self.declaration())
    }
}
impl Encode for crate::SamplingDeclaration {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j, "assertion" => self.assertion().as_str(), "source" => self.source(),
            "grid" => self.grid(), "original_indices" => self.indices(),
            "index_base" => usize::from(self.index_base() == crate::SamplingIndexBase::One),
            "indirect_lanes" => self.indirect_lanes())
    }
}
impl Encode for crate::processed::RawDatasetSnapshot {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(
            j,
            "axes" => self.descriptor().axes(),
            "layout_evidence" => self.descriptor().layout_evidence(),
            "source_normalization" => self.sample_normalization(),
            "sampling_schedule" => self.sampling_schedule(),
            "absolute_origin" => self.absolute_origin(),
            "digests" => self.canonical_digests(),
            "sources" => self.sources()
        )
    }
}

impl Encode for crate::processing::SpectrumOperation {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        use crate::processing::{
            BinAggregation, Normalization as N, RealBaseline as B, SpectrumOperation as O,
        };
        match self {
            O::RetainRange { start, end } => {
                object!(j,"kind"=>"retain-range","start"=>start,"end"=>end)
            }
            O::Reference { delta_ppm } => {
                object!(j,"kind"=>"reference-shift","delta_ppm"=>delta_ppm)
            }
            O::Reverse | O::Invert | O::Magnitude => object!(j,"kind"=>self.algorithm_version()),
            O::Affine { scale, real_offset } => {
                object!(j,"kind"=>"affine","scale"=>scale,"real_offset"=>real_offset)
            }
            O::MovingAverage { window } => object!(j,"kind"=>"moving-average","window"=>window),
            O::SavitzkyGolay { window, order } => {
                object!(j,"kind"=>"savitzky-golay","window"=>window,"order"=>order)
            }
            O::Baseline(B::Offset) => object!(j,"kind"=>"real-baseline-offset"),
            O::Baseline(B::Polynomial { order }) => {
                object!(j,"kind"=>"real-baseline-polynomial","order"=>order)
            }
            O::Baseline(B::Asls {
                lambda,
                asymmetry,
                iterations,
            }) => {
                object!(j,"kind"=>"real-baseline-index-asls","lambda"=>lambda,"asymmetry"=>asymmetry,"iterations"=>iterations)
            }
            O::Normalize(N::MaxPeak) => object!(j,"kind"=>"normalize-max-peak"),
            O::Normalize(N::Constant(divisor)) => {
                object!(j,"kind"=>"normalize-constant","divisor"=>divisor)
            }
            O::Normalize(N::TotalArea { singleton_width }) => {
                object!(j,"kind"=>"normalize-total-area","singleton_width"=>singleton_width)
            }
            O::Bin { width, aggregation } => {
                object!(j,"kind"=>"bin","width"=>width,"aggregation"=>if *aggregation == BinAggregation::Sum { "sum" } else { "mean" })
            }
            O::Slice { index, component } => {
                object!(j,"kind"=>"slice","index"=>index,"component"=>component)
            }
            O::Sum { component } => object!(j,"kind"=>"sum-dimension","component"=>component),
            O::Skyline { component } => object!(j,"kind"=>"skyline","component"=>component),
        }
    }
}

impl Encode for crate::processing::NusNoiseReport {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"method"=>self.method_version(),"sigma"=>self.sigma,
            "frequency_ranges"=>self.frequency_ranges,"scalar_samples"=>self.scalar_samples,
            "effective_observations"=>self.effective_observations,"block_dispersion"=>self.block_dispersion,
            "radial_moment_error"=>self.radial_moment_error,"isotropy_error"=>self.isotropy_error,
            "observation_correlation"=>self.observation_correlation)
    }
}

impl Encode for (usize, usize) {
    fn encode(&self, j: &mut Json<'_>) -> Result<(), ReportError> {
        object!(j,"start"=>self.0,"end"=>self.1)
    }
}
