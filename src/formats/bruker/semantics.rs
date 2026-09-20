use super::context::ErrorContext;
use super::context::parameter_error;
use super::parameters::ParameterFile;
use crate::AcquisitionDescriptor;
use crate::Complex64;
use crate::ReadError;
use crate::acquisition::AxisIndex;
use crate::acquisition::IndirectComponents;
use crate::acquisition::LinearComponentTransform;
use crate::acquisition::ModulationIndexDomain;
use crate::acquisition::NormalizationEvidence;
use crate::acquisition::NormalizationFact;
use crate::acquisition::PeriodicLaneModulation;
use crate::acquisition::RawAxisKind;
use crate::acquisition::ResolvedComponentTransform;
use crate::acquisition::registry;
use crate::raw::InputSource;
use crate::raw::ParameterError;
use crate::raw::ParameterErrorKind;

pub(crate) fn resolved_descriptor(
    axes: Vec<crate::Axis>,
    metadata: crate::AcquisitionMetadata,
) -> Result<AcquisitionDescriptor, ReadError> {
    let rule = if axes.len() == 2
        && matches!(
            axes[0].kind(),
            RawAxisKind::Indirect(IndirectComponents::Scalar)
        ) {
        registry::BRUKER_QF_V1
    } else {
        registry::BRUKER_LAYOUT_V1
    };
    Ok(AcquisitionDescriptor::new_resolved(
        axes,
        metadata,
        NormalizationEvidence::from_format(
            rule,
            vec![NormalizationFact::TraceMappingBijection],
            vec![registry::FORMAT_LAYOUT_V1],
        )?,
    )?)
}

pub(crate) fn require_experimental_nus(
    enabled: bool,
    source: InputSource,
) -> Result<(), ReadError> {
    if enabled {
        Ok(())
    } else {
        Err(ReadError::unsupported_feature(source,
            crate::raw::UnsupportedFeatureCode::EXPERIMENTAL_VENDOR_SEMANTICS, None,
            vec!["Bruker NUS schedule/preallocation interpretations have incomplete independent evidence; set allow_experimental_vendor_semantics(true) to opt in".into()]))
    }
}

pub(crate) fn indirect_layout(
    fn_mode: i64,
    axis: AxisIndex,
    source: &(impl ErrorContext + ?Sized),
) -> Result<(IndirectComponents, usize), ReadError> {
    let components = match fn_mode {
        1 => IndirectComponents::Scalar,
        4 => {
            let transform = LinearComponentTransform::try_new(
                2,
                vec![
                    Complex64::new(1.0, 0.0),
                    Complex64::new(0.0, 0.0),
                    Complex64::new(0.0, 0.0),
                    Complex64::new(1.0, 0.0),
                ],
                PeriodicLaneModulation::identity(2)?,
            )?;
            let evidence = NormalizationEvidence::from_format(
                registry::BRUKER_STATES_V1,
                vec![
                    NormalizationFact::CanonicalLaneOrder {
                        lanes: std::num::NonZeroUsize::new(2).expect("two is nonzero"),
                    },
                    NormalizationFact::TraceMappingBijection,
                ],
                vec![registry::BRUKER_COMPONENT_V1],
            )?;
            IndirectComponents::Encoded(ResolvedComponentTransform::resolved(transform, evidence))
        }
        5 => {
            let modulation = PeriodicLaneModulation::try_new(
                2,
                2,
                vec![
                    Complex64::new(1.0, 0.0),
                    Complex64::new(1.0, 0.0),
                    Complex64::new(-1.0, 0.0),
                    Complex64::new(-1.0, 0.0),
                ],
                ModulationIndexDomain::AbsoluteGridCoordinate(axis),
                0,
            )?;
            let transform = LinearComponentTransform::try_new(
                2,
                vec![
                    Complex64::new(1.0, 0.0),
                    Complex64::new(0.0, 0.0),
                    Complex64::new(0.0, 0.0),
                    Complex64::new(1.0, 0.0),
                ],
                modulation,
            )?;
            let evidence = NormalizationEvidence::from_format(
                registry::BRUKER_STATES_TPPI_V1,
                vec![
                    NormalizationFact::CanonicalLaneOrder {
                        lanes: std::num::NonZeroUsize::new(2).expect("two is nonzero"),
                    },
                    NormalizationFact::TraceMappingBijection,
                    NormalizationFact::PeriodicModulation {
                        period: std::num::NonZeroUsize::new(2).expect("two is nonzero"),
                        domain: ModulationIndexDomain::AbsoluteGridCoordinate(axis),
                    },
                ],
                vec![registry::BRUKER_COMPONENT_V1],
            )?;
            IndirectComponents::Encoded(ResolvedComponentTransform::resolved(transform, evidence))
        }
        6 => {
            let transform = LinearComponentTransform::try_new(
                2,
                vec![
                    Complex64::new(0.0, 1.0),
                    Complex64::new(0.0, 1.0),
                    Complex64::new(-1.0, 0.0),
                    Complex64::new(1.0, 0.0),
                ],
                PeriodicLaneModulation::identity(2)?,
            )?;
            let evidence = NormalizationEvidence::from_format(
                registry::BRUKER_ECHO_ANTI_ECHO_V1,
                vec![
                    NormalizationFact::CanonicalLaneOrder {
                        lanes: std::num::NonZeroUsize::new(2).expect("two is nonzero"),
                    },
                    NormalizationFact::TraceMappingBijection,
                ],
                vec![registry::BRUKER_COMPONENT_V1],
            )?;
            IndirectComponents::Encoded(ResolvedComponentTransform::resolved(transform, evidence))
        }
        value => Err(ReadError::unsupported(
            source.input_source(),
            format!(
                "FnMODE={value}; validated indirect modes are 1 (QF), 4 (States), 5 (States-TPPI), and 6 (Echo/AntiEcho)"
            ),
        ))?,
    };
    let lanes = match &components {
        IndirectComponents::Scalar | IndirectComponents::SharedComplex { .. } => 1,
        IndirectComponents::Cartesian(_) => 2,
        IndirectComponents::Encoded(transform) => transform.input_lanes(),
    };
    Ok((components, lanes))
}

pub(crate) fn indirect_stored_points(
    parameters: &ParameterFile,
    source: &(impl ErrorContext + ?Sized),
    lanes: usize,
) -> Result<usize, ReadError> {
    let stored = parameters.usize("TD", source)?;
    if stored < lanes || stored % lanes != 0 {
        return Err(parameter_error(
            source,
            Some("TD"),
            ParameterErrorKind::Invalid,
            format!(
                "value {stored}: expected a trace count divisible by the FnMODE lane count {lanes}"
            ),
        )
        .into());
    }
    Ok(stored)
}

pub(crate) struct ResolvedGroupDelay {
    pub(super) points: f64,
    pub(super) rule: crate::acquisition::RuleId,
}

pub(crate) fn group_delay(
    parameters: &ParameterFile,
    source: &(impl ErrorContext + ?Sized),
) -> Result<Option<ResolvedGroupDelay>, ParameterError> {
    match parameters.optional_float("GRPDLY", source)? {
        None | Some(-1.0) => {
            // Validate both present fields before checking the supported table.
            // Checked conversions prevent negative/oversized integers aliasing a key.
            let dspfvs = parameters.optional_integer("DSPFVS", source)?;
            let decim = parameters.optional_integer("DECIM", source)?;
            let points = dspfvs
                .and_then(|value| u32::try_from(value).ok())
                .zip(decim.and_then(|value| u32::try_from(value).ok()))
                .and_then(|(version, decimation)| bruker_group_delay_fallback(version, decimation));
            Ok(points.map(|points| ResolvedGroupDelay {
                points,
                rule: registry::BRUKER_DSP_TABLE_V1,
            }))
        }
        Some(points) if points >= 0.0 => Ok(Some(ResolvedGroupDelay {
            points,
            rule: registry::BRUKER_DIRECT_V1,
        })),
        Some(value) => Err(parameter_error(
            source,
            Some("GRPDLY"),
            ParameterErrorKind::Invalid,
            format!("value {value}: expected -1 sentinel or a non-negative value"),
        )),
    }
}

// Historical parameter lookup contract; values are logical complex sample points.
// Only these DSPFVS/DECIM pairs are supported, with no extrapolation or zero default.
pub(crate) fn bruker_group_delay_fallback(dspfvs: u32, decim: u32) -> Option<f64> {
    const DECIM: [u32; 21] = [
        2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048,
    ];
    const V10: [f64; 21] = [
        44.75,
        33.5,
        66.625,
        59.083333333333336,
        68.5625,
        60.375,
        69.53125,
        61.020833333333336,
        70.015625,
        61.34375,
        70.2578125,
        61.505208333333336,
        70.37890625,
        61.5859375,
        70.439453125,
        61.626302083333336,
        70.4697265625,
        61.646484375,
        70.48486328125,
        61.656575520833336,
        70.492431640625,
    ];
    const V11: [f64; 21] = [
        46.0,
        36.5,
        48.0,
        50.166666666666664,
        53.25,
        69.5,
        72.25,
        70.16666666666667,
        72.75,
        70.5,
        73.0,
        70.66666666666667,
        72.5,
        71.33333333333333,
        72.25,
        71.66666666666667,
        72.125,
        71.83333333333333,
        72.0625,
        71.91666666666667,
        72.03125,
    ];
    const V12: [f64; 21] = [
        46.0,
        36.5,
        48.0,
        50.166666666666664,
        53.25,
        69.5,
        71.625,
        70.16666666666667,
        72.125,
        70.5,
        72.375,
        70.66666666666667,
        72.5,
        71.33333333333333,
        72.25,
        71.66666666666667,
        72.125,
        71.83333333333333,
        72.0625,
        71.91666666666667,
        72.03125,
    ];
    const DECIM13: [u32; 12] = [2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96];
    const V13: [f64; 12] = [
        2.75,
        2.8333333333333335,
        2.875,
        2.9166666666666665,
        2.9375,
        2.9583333333333335,
        2.96875,
        2.9791666666666665,
        2.984375,
        2.9895833333333335,
        2.9921875,
        2.9947916666666665,
    ];
    match dspfvs {
        10..=12 => {
            let index = DECIM.iter().position(|value| *value == decim)?;
            Some(match dspfvs {
                10 => V10[index],
                11 => V11[index],
                12 => V12[index],
                _ => unreachable!(),
            })
        }
        13 => DECIM13
            .iter()
            .position(|value| *value == decim)
            .map(|index| V13[index]),
        _ => None,
    }
}
