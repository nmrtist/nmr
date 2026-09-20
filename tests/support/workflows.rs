//! Synthetic scientific inputs for integration tests.
use nmr::axis::{AxisCoordinates, AxisDomain, AxisUnit};
use nmr::processing::{ProcessingOperation as O, SpectrumOperation as S, *};
use nmr::raw::*;
use nmr::{Complex64, Dataset, ExecutionContext};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn raw_2d(parameter: bool, indices: Option<&[usize]>, n: usize) -> Result<Dataset> {
    let indirect = RawAxis::new(
        if parameter {
            RawAxisKind::Parameter
        } else {
            RawAxisKind::Indirect(IndirectComponents::Cartesian(
                ComponentEvidence::user_constructed(),
            ))
        },
        if parameter {
            AxisDomain::Parameter
        } else {
            AxisDomain::Time
        },
        Some(AxisUnit::Second),
        n,
        if parameter {
            AxisCoordinates::Explicit(vec![0.001, 0.002, 0.002])
        } else {
            AxisCoordinates::Uniform {
                start: 0.0,
                step: 0.01,
            }
        },
    )?;
    let direct = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        5,
        AxisCoordinates::Uniform {
            start: 0.002,
            step: 0.001,
        },
    )?;
    let row = |i: usize| {
        let phase = std::f64::consts::TAU * i as f64 / n as f64;
        let lanes = if parameter {
            vec![1.0 + i as f64]
        } else {
            vec![phase.cos(), phase.sin()]
        };
        lanes
            .into_iter()
            .flat_map(|lane| {
                (0..5).map(move |j| {
                    Complex64::from_polar(lane, std::f64::consts::TAU * j as f64 / 5.0)
                })
            })
            .collect::<Vec<_>>()
    };
    let builder = RawDatasetBuilder::new(vec![indirect, direct], RawMetadata::default())?;
    Ok(if let Some(indices) = indices {
        let coordinates = indices
            .iter()
            .map(|i| SamplingCoordinate::new(vec![*i]))
            .collect::<Vec<_>>();
        let traces = indices
            .iter()
            .enumerate()
            .map(|(ordinal, &i)| {
                SparseTrace::new(
                    ObservationOrdinal::new(ordinal),
                    coordinates[ordinal].clone(),
                    row(i),
                )
            })
            .collect();
        builder
            .sparse(traces, SamplingSchedule::new(vec![n], coordinates)?)?
            .into()
    } else {
        builder.dense((0..n).flat_map(row).collect())?.into()
    })
}

fn plan(operations: Vec<O>) -> ProcessingPlan {
    ProcessingPlan::new(operations).unwrap()
}
fn op(axis: usize, operation: S) -> O {
    O::Spectrum { axis, operation }
}
fn fft(axis: usize) -> O {
    O::FourierTransform {
        axis,
        transform: FourierTransform::default(),
    }
}

/// All numerical segments share one context. Only explicit operations run.
pub fn run() -> Result<Vec<(&'static str, Dataset)>> {
    let mut work = WorkLedger::new(u128::MAX);
    let mut context = ExecutionContext::new(&mut work);
    let options = ProcessingOptions::new();
    let mut artifacts = vec![];
    std::fs::create_dir_all("target")?;
    let directory = tempfile::Builder::new()
        .prefix("nmr-example-")
        .tempdir_in("target")?;
    let source = directory.path().join("spectrum.dx");
    std::fs::write(
        &source,
        "##TITLE=Synthetic public example\n##JCAMP-DX=5.00\n##DATA TYPE=NMR SPECTRUM\n##XUNITS=PPM\n##YUNITS=ARBITRARY UNITS\n##XFACTOR=1\n##YFACTOR=1\n##FIRSTX=4\n##LASTX=0\n##NPOINTS=5\n##DELTAX=-1\n##XYDATA=(X++(Y..Y))\n4 5 5 105 5 5\n##END=\n",
    )?;
    // Minimal synthetic experimental JDF; these bytes are not vendor evidence.
    let mut jdf = vec![0u8; 1360 + 64];
    jdf[..8].copy_from_slice(b"JEOL.NMR");
    jdf[8] = 1;
    jdf[9] = 1;
    jdf[10..12].copy_from_slice(&2u16.to_be_bytes());
    jdf[12] = 1;
    jdf[13] = 1;
    jdf[14] = 1;
    jdf[24] = 3;
    jdf[32] = 1;
    jdf[33] = 28;
    for (offset, value) in [(176, 4u32), (240, 3), (1284, 1360)] {
        jdf[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    jdf[336..344].copy_from_slice(&0.003f64.to_be_bytes());
    for (i, value) in [1.0f64, 2.0, 3.0, 4.0, 10.0, 20.0, 30.0, 40.0]
        .iter()
        .enumerate()
    {
        jdf[1360 + i * 8..1368 + i * 8].copy_from_slice(&value.to_le_bytes());
    }
    let jdf_path = directory.path().join("experimental.jdf");
    std::fs::write(&jdf_path, jdf)?;
    let experimental = nmr::ReadOptions::new()
        .allow_experimental_vendor_semantics(true)
        .read_with_context(&jdf_path, &mut context)?;
    assert!(
        experimental
            .warnings()
            .iter()
            .any(|w| matches!(w, nmr::ReadWarning::ExperimentalVendorSemantics { .. }))
    );
    std::fs::remove_file(&jdf_path)?;
    artifacts.push(("experimental-jeol", experimental));
    let imported = nmr::read_with_context(&source, &mut context)?;
    let estimate = RealBaseline::Offset
        .prepare(&imported, options)?
        .estimate_with_context(&mut context)?;
    assert_eq!(estimate.coefficients(), [5.0]);
    let corrected = estimate.apply_with_context(&imported, options, &mut context)?;
    let shifted = plan(vec![
        op(0, S::Reference { delta_ppm: -2.0 }),
        op(0, S::MovingAverage { window: 3 }),
    ])
    .apply_with_context(&corrected, options, &mut context)?;
    let combined = LinearCombination::new(-0.25)?
        .prepare(&shifted, &corrected, options)?
        .execute_with_context(&mut context)?;
    let mut bytes = vec![];
    nmr::snapshot::write_snapshot_with_context(
        &combined,
        &mut bytes,
        Default::default(),
        &mut context,
    )?;
    std::fs::remove_file(&source)?;
    let restored = nmr::snapshot::read_snapshot_with_context(
        &mut bytes.as_slice(),
        Default::default(),
        &mut context,
    )?
    .restore(nmr::snapshot::AcceptRecordedHistory);
    let offline =
        plan(vec![op(0, S::Invert)]).apply_with_context(&restored, options, &mut context)?;
    assert!(offline.selected_path().is_some());
    let all_spectrum = plan(vec![
        op(0, S::Baseline(RealBaseline::Polynomial { order: 0 })),
        op(
            0,
            S::Baseline(RealBaseline::Asls {
                lambda: 50000.0,
                asymmetry: 0.001,
                iterations: 20,
            }),
        ),
        op(
            0,
            S::Affine {
                scale: 2.0,
                real_offset: 1.0,
            },
        ),
        op(0, S::Reverse),
        op(0, S::Invert),
        op(
            0,
            S::SavitzkyGolay {
                window: 3,
                order: 2,
            },
        ),
        op(0, S::Normalize(Normalization::MaxPeak)),
        op(
            0,
            S::Normalize(Normalization::TotalArea {
                singleton_width: None,
            }),
        ),
        op(0, S::Normalize(Normalization::Constant(-2.0))),
        op(
            0,
            S::Bin {
                width: 2.0,
                aggregation: BinAggregation::Mean,
            },
        ),
    ])
    .apply_with_context(&corrected, options, &mut context)?;
    artifacts.push(("spectrum-operations", all_spectrum));
    for (name, method) in [
        ("polynomial-estimate", RealBaseline::Polynomial { order: 1 }),
        (
            "asls-estimate",
            RealBaseline::Asls {
                lambda: 50000.0,
                asymmetry: 0.001,
                iterations: 20,
            },
        ),
    ] {
        let fit = method
            .prepare(&imported, options)?
            .estimate_with_context(&mut context)?;
        artifacts.push((
            name,
            fit.apply_with_context(&imported, options, &mut context)?,
        ));
    }
    artifacts.extend([
        ("scalar-ppm", imported),
        ("baseline-estimate", corrected),
        ("binary-offline", offline),
    ]);

    let dense = raw_2d(false, None, 3)?;
    let mixed = plan(vec![
        O::Window {
            axis: 1,
            window: Window::lorentz_to_gauss(0.0, 20.0)?,
        },
        fft(1),
    ])
    .apply_with_context(&dense, options, &mut context)?;
    assert_eq!(
        mixed
            .as_processed()
            .unwrap()
            .descriptor()
            .component_counts(),
        [2, 2]
    );
    let two_d = plan(vec![
        fft(0),
        O::PhaseCorrection {
            axis: 0,
            correction: PhaseCorrection::zero_order_degrees(20.0)?,
        },
        op(1, S::Baseline(RealBaseline::Offset)),
    ])
    .apply_with_context(&mixed, options, &mut context)?;
    for (name, operation) in [
        ("sum-dimension", S::Sum { component: 0 }),
        ("skyline-dimension", S::Skyline { component: 0 }),
        ("axis-magnitude", S::Magnitude),
    ] {
        artifacts.push((
            name,
            plan(vec![op(0, operation)]).apply_with_context(&two_d, options, &mut context)?,
        ));
    }
    let row = plan(vec![op(
        0,
        S::Slice {
            index: 1,
            component: 0,
        },
    )])
    .apply_with_context(&two_d, options, &mut context)?;
    let external = row.derive_external_processed(
        row.as_processed().unwrap().descriptor().clone(),
        row.as_dense_processed().unwrap().samples().to_vec(),
        vec![nmr::external::ExternalAxisSource::Parent(
            nmr::provenance::InputAxisRef::new(nmr::provenance::InputSlot::new(0), 0),
        )],
        nmr::external::ExternalAlgorithmDeclaration::new(
            "host-demo-identity",
            "1",
            "Identity result demonstrating the boundary; no model fitting is performed",
        )?,
    )?;
    artifacts.extend([
        ("dense-raw", dense),
        ("mixed-domain", mixed),
        ("hypercomplex", two_d),
        ("cartesian-slice", row),
        ("external", external),
    ]);

    let pseudo = raw_2d(true, None, 3)?;
    let series = plan(vec![fft(1)]).apply_with_context(&pseudo, options, &mut context)?;
    let representative = plan(vec![op(
        0,
        S::Slice {
            index: 1,
            component: 0,
        },
    )])
    .apply_with_context(&series, options, &mut context)?;
    let phase = PhaseMethod::AbsorptivePeak
        .prepare(&representative, 0, options)?
        .estimate_with_context(&mut context)?;
    let phased_representative = phase.apply_with_context(&representative, options, &mut context)?;
    // Series selection is a host choice; this applies exactly one estimated correction.
    let series = plan(vec![O::PhaseCorrection {
        axis: 1,
        correction: phase.correction(),
    }])
    .apply_with_context(&series, options, &mut context)?;
    assert_eq!(
        series.as_processed().unwrap().descriptor().axes()[0].coordinates(),
        &AxisCoordinates::Explicit(vec![0.001, 0.002, 0.002])
    );
    artifacts.extend([
        ("pseudo-series", series),
        ("phase-estimate", phased_representative),
    ]);

    let sparse = raw_2d(false, Some(&[7, 0, 1, 3, 4, 6]), 8)?;
    let prepared = NusSettings {
        max_iterations: 1000,
        noise_standard_deviation: Some(0.0),
    }
    .prepare(&sparse, plan(vec![fft(1)]), options)?;
    assert_eq!(prepared.measured_indices(), [7, 0, 1, 3, 4, 6]);
    let reconstructed = prepared.execute_with_context(&mut context)?;
    let nus = plan(vec![fft(0)]).apply_with_context(&reconstructed, options, &mut context)?;
    artifacts.extend([
        ("sparse-raw", sparse),
        ("nus", nus),
        ("sparse-duplicates", raw_2d(false, Some(&[3, 1, 3]), 4)?),
    ]);

    // A retained FID interval uses absolute coordinates; the window starts at its first point.
    let fid_axis = RawAxis::new(
        RawAxisKind::Direct(DirectSamples::Complex),
        AxisDomain::Time,
        Some(AxisUnit::Second),
        8,
        AxisCoordinates::Uniform {
            start: 0.01,
            step: 0.001,
        },
    )?
    .with_group_delay(GroupDelayState::Pending(
        PendingGroupDelay::user_constructed(0.0)?,
    ))?
    .with_chemical_shift_reference(Some(
        nmr::acquisition::ChemicalShiftReference::user_constructed(4.7, 400.0)?,
    ))?;
    let fid: Dataset = RawDatasetBuilder::new(vec![fid_axis], RawMetadata::default())?
        .dense(vec![Complex64::new(1.0, 0.0); 8])?
        .into();
    let cropped = plan(vec![
        op(0, S::RetainRange { start: 2, end: 7 }),
        O::Window {
            axis: 0,
            window: Window::exponential(2.0)?,
        },
        O::StandardZeroFill { axis: 0 },
    ])
    .apply_with_context(&fid, options, &mut context)?;
    assert_eq!(
        cropped.as_processed().unwrap().descriptor().axes()[0].coordinate(0)?,
        0.012
    );
    artifacts.push(("retained-fid", cropped));
    let phase_axis = nmr::processed::ProcessedAxis::new(
        nmr::axis::AxisRole::Signal,
        AxisDomain::Frequency,
        Some(AxisUnit::Ppm),
        256,
        AxisCoordinates::Uniform {
            start: 0.0,
            step: 0.01,
        },
        nmr::processed::ComponentBasis::Cartesian,
    )?;
    let phase_values = (0..256)
        .flat_map(|i| {
            let mut z = Complex64::default();
            for (center, amplitude) in [(60.0, 1.0), (170.0, 0.7)] {
                let u = (i as f64 - center) / 3.0;
                z += Complex64::new(1.0, -u) * (amplitude / (1.0 + u * u));
            }
            z *= Complex64::from_polar(1.0, 0.4);
            [z.re, z.im]
        })
        .collect();
    let phase_input: Dataset = nmr::processed::ProcessedDataset::from_dense_samples(
        nmr::processed::ProcessedDescriptor::new(vec![phase_axis])?,
        phase_values,
        nmr::processed::ProcessedProvenance::new(nmr::processed::ProcessedOrigin::Unknown, vec![])?,
    )?
    .into();
    for (name, method) in [
        ("absorptive-peak", PhaseMethod::AbsorptivePeak),
        ("entropy", PhaseMethod::Entropy),
        ("negative-minimization", PhaseMethod::NegativeMinimization),
        ("peak-regression", PhaseMethod::PeakRegression),
        ("robust-consensus", PhaseMethod::RobustConsensus),
    ] {
        let phase = method
            .prepare(&phase_input, 0, options)?
            .estimate_with_context(&mut context)?;
        artifacts.push((
            name,
            phase.apply_with_context(&phase_input, options, &mut context)?,
        ));
    }
    for (_, dataset) in &artifacts {
        let mut bytes = vec![];
        nmr::snapshot::write_snapshot_with_context(
            dataset,
            &mut bytes,
            Default::default(),
            &mut context,
        )?;
        let restored = nmr::snapshot::read_snapshot_with_context(
            &mut bytes.as_slice(),
            Default::default(),
            &mut context,
        )?
        .restore(nmr::snapshot::AcceptRecordedHistory);
        assert_eq!(restored.canonical_digests(), dataset.canonical_digests());
        if restored.as_processed().is_some() {
            plan(vec![op(0, S::Invert)]).apply_with_context(&restored, options, &mut context)?;
        }
    }
    Ok(artifacts)
}
