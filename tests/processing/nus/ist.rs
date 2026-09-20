use nmr::processing::{
    IstError, IstInput, IstOptions, PhaseCovariantGroupRetainedIstV1, WorkLedger,
};

fn schedule() -> Vec<usize> {
    (0..512).step_by(4).collect()
}

fn nonuniform_schedule() -> Vec<usize> {
    (0..128).map(|index| index * 73 % 512).collect()
}

fn spectral_input(spectrum: &[f64], f2_points: usize, direct: usize) -> IstInput {
    let measured = nonuniform_schedule();
    let mut components = vec![0.0; 128 * f2_points * 2];
    for (row, &logical) in measured.iter().enumerate() {
        let mut value = nmr::Complex64::new(0.0, 0.0);
        for (frequency, &amplitude) in spectrum.iter().enumerate() {
            let phase = 2.0 * std::f64::consts::PI * frequency as f64 * logical as f64 / 512.0;
            value += nmr::Complex64::from_polar(amplitude / 512.0, phase);
        }
        let offset = (row * f2_points + direct) * 2;
        components[offset] = value.re;
        components[offset + 1] = value.im;
    }
    IstInput::new(f2_points, 1, measured, components).unwrap()
}

fn reconstructed_spectrum(output: &nmr::processing::IstOutput, direct: usize) -> Vec<f64> {
    let mut spectrum = Vec::with_capacity(512);
    for frequency in 0..512 {
        let mut value = nmr::Complex64::new(0.0, 0.0);
        for logical in 0..512 {
            let offset = (logical * output.f2_points() + direct) * 2;
            let sample =
                nmr::Complex64::new(output.components()[offset], output.components()[offset + 1]);
            let phase = -2.0 * std::f64::consts::PI * frequency as f64 * logical as f64 / 512.0;
            value += sample * nmr::Complex64::from_polar(1.0, phase);
        }
        spectrum.push(value.re);
    }
    spectrum
}

fn cosine_and_slope(actual: &[f64], oracle: &[f64]) -> (f64, f64) {
    let dot: f64 = actual.iter().zip(oracle).map(|(a, b)| a * b).sum();
    let actual_norm: f64 = actual.iter().map(|value| value * value).sum::<f64>().sqrt();
    let oracle_square: f64 = oracle.iter().map(|value| value * value).sum();
    (
        dot / (actual_norm * oracle_square.sqrt()),
        dot / oracle_square,
    )
}

fn assert_peak(actual: &[f64], oracle: &[f64], start: usize, end: usize, strong: f64) {
    let actual_roi = &actual[start..end];
    let oracle_roi = &oracle[start..end];
    let (cosine, slope) = cosine_and_slope(actual_roi, oracle_roi);
    let actual_integral: f64 = actual_roi
        .windows(2)
        .map(|pair| pair[0] / 2.0 + pair[1] / 2.0)
        .sum();
    let oracle_integral: f64 = oracle_roi
        .windows(2)
        .map(|pair| pair[0] / 2.0 + pair[1] / 2.0)
        .sum();
    let actual_position = start
        + actual_roi
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .unwrap()
            .0;
    let oracle_position = start
        + oracle_roi
            .iter()
            .enumerate()
            .max_by(|left, right| left.1.total_cmp(right.1))
            .unwrap()
            .0;
    let actual_height = actual[actual_position];
    let oracle_height = oracle[oracle_position];
    assert!(cosine >= 0.995, "cosine={cosine}");
    assert!((slope - 1.0).abs() <= 0.05, "slope={slope}");
    assert!(
        ((actual_integral - oracle_integral) / oracle_integral).abs() <= 0.05,
        "integrals actual={actual_integral} oracle={oracle_integral}"
    );
    assert!(
        (actual_height - oracle_height).abs() <= 1e-12 * strong + 0.05 * oracle_height.abs(),
        "heights actual={actual_height} oracle={oracle_height}"
    );
    assert!(actual_position.abs_diff(oracle_position) <= 1);
}

fn impulse_input(fields: usize, values: &[f64]) -> IstInput {
    let measured = schedule();
    let mut components = vec![0.0; 128 * fields * 2];
    for field in 0..fields {
        components[field * 2] = values[field];
    }
    IstInput::new(1, fields, measured, components).unwrap()
}

fn run(input: &IstInput) -> Result<nmr::processing::IstOutput, IstError> {
    let mut work = WorkLedger::new(40_000_000_000);
    PhaseCovariantGroupRetainedIstV1::new().reconstruct(
        input,
        &mut work,
        IstOptions::default().noiseless(),
    )
}

#[test]
fn fixed_profile_shape_and_work_limit_are_explicit() {
    assert_eq!(PhaseCovariantGroupRetainedIstV1::N_LOGICAL, 512);
    assert_eq!(PhaseCovariantGroupRetainedIstV1::M_MEASURED, 128);
    assert_eq!(PhaseCovariantGroupRetainedIstV1::INDIRECT_ZERO_FILL, 1024);
    assert_eq!(IstOptions::default().reconstruction_work(), 35_000_000_000);
}

#[test]
fn malformed_schedule_component_and_iteration_boundaries_fail_closed() {
    assert_eq!(
        IstInput::new(1, 1, vec![0; 128], vec![0.0; 256]).unwrap_err(),
        IstError::InvalidInput
    );
    assert_eq!(
        IstInput::new(1, 3, schedule(), vec![0.0; 128 * 3 * 2]).unwrap_err(),
        IstError::InvalidInput
    );
    assert_eq!(
        IstOptions::new().max_iterations(2049).unwrap_err(),
        IstError::InvalidOptions
    );
}

#[test]
fn zero_signal_and_deterministic_pure_noise_are_not_reconstructed() {
    assert_eq!(
        run(&impulse_input(1, &[0.0])).unwrap_err(),
        IstError::NoRecoverableSignal
    );

    let mut state = 0x1234_5678_u64;
    let mut components = Vec::with_capacity(256);
    for _ in 0..256 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        components.push(((state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0);
    }
    let noise = IstInput::new(1, 1, schedule(), components).unwrap();
    let result = PhaseCovariantGroupRetainedIstV1.reconstruct(
        &noise,
        &mut WorkLedger::new(u128::MAX),
        IstOptions::new()
            .noise_standard_deviation((1.0_f64 / 3.0).sqrt())
            .unwrap(),
    );
    assert_eq!(result.unwrap_err(), IstError::NoRecoverableSignal);
}

#[test]
fn reconstruction_retains_every_measured_f64_bit() {
    let input = impulse_input(1, &[1.0]);
    let output = run(&input).unwrap();
    assert!(output.iterations() <= 768);
    for (measured, &coordinate) in input.measured_indices().iter().enumerate() {
        for component in 0..2 {
            let source = measured * 2 + component;
            let target = coordinate * 2 + component;
            assert_eq!(
                input.components()[source].to_bits(),
                output.components()[target].to_bits()
            );
        }
    }
}

#[test]
fn group_rotation_commutes_with_reconstruction() {
    let angle = 0.37_f64;
    let (cosine, sine) = (angle.cos(), angle.sin());
    let original = impulse_input(2, &[1.0, 2.0]);
    let rotated = impulse_input(2, &[cosine - 2.0 * sine, sine + 2.0 * cosine]);
    let reconstructed = run(&original).unwrap();
    let reconstructed_rotated = run(&rotated).unwrap();
    for logical in 0..512 {
        let first = reconstructed.components()[logical * 4];
        let second = reconstructed.components()[logical * 4 + 2];
        let expected_first = cosine * first - sine * second;
        let expected_second = sine * first + cosine * second;
        let actual_first = reconstructed_rotated.components()[logical * 4];
        let actual_second = reconstructed_rotated.components()[logical * 4 + 2];
        assert!((actual_first - expected_first).abs() <= 1e-10);
        assert!((actual_second - expected_second).abs() <= 1e-10);
    }
}

#[test]
fn output_and_working_limits_are_independent_and_checked_before_work_charge() {
    let input = impulse_input(1, &[1.0]);
    let profile = PhaseCovariantGroupRetainedIstV1::new();
    let mut work = WorkLedger::new(u128::MAX);
    let output_error = profile
        .reconstruct(
            &input,
            &mut work,
            IstOptions::new()
                .noiseless()
                .max_output_bytes(512 * 2 * 8 - 1)
                .max_working_bytes(usize::MAX),
        )
        .unwrap_err();
    assert_eq!(output_error, IstError::OutputLimit);
    assert_eq!(work.used(), 0);

    let working_error = profile
        .reconstruct(
            &input,
            &mut work,
            IstOptions::new()
                .noiseless()
                .max_output_bytes(usize::MAX)
                .max_working_bytes(1),
        )
        .unwrap_err();
    assert_eq!(working_error, IstError::WorkingLimit);
    assert_eq!(work.used(), 0);
}

#[test]
fn nonoverlapping_hundred_to_one_peaks_pass_independent_acceptance() {
    let mut oracle = vec![0.0; 512];
    oracle[73] = 1.0;
    oracle[307] = 0.01;
    let input = spectral_input(&oracle, 8, 3);
    let output = run(&input).unwrap();
    let actual = reconstructed_spectrum(&output, 3);

    assert_peak(&actual, &oracle, 69, 78, 1.0);
    assert_peak(&actual, &oracle, 303, 312, 1.0);
    assert!(
        actual[303..312]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            > 0.0
    );
}

#[test]
fn broad_and_off_grid_cluster_passes_cluster_level_acceptance() {
    let mut oracle = vec![0.0; 512];
    let center = 181.35_f64;
    for (frequency, amplitude) in oracle.iter_mut().enumerate() {
        *amplitude = (-0.5 * ((frequency as f64 - center) / 2.3).powi(2)).exp();
    }
    let input = spectral_input(&oracle, 8, 5);
    let output = run(&input).unwrap();
    let actual = reconstructed_spectrum(&output, 5);

    assert_peak(&actual, &oracle, 169, 194, 1.0);
}

#[test]
fn cancellation_during_ist_iterations_keeps_entered_work() {
    let input = impulse_input(1, &[1.0]);
    let token = nmr::CancellationToken::new();
    let signal = token.clone();
    let mut observations = 0;
    let mut progress = move |event: nmr::execution::ProgressEvent| {
        if event.stage == nmr::execution::ExecutionStage::Reconstruction && event.completed > 0 {
            observations += 1;
            if observations == 3 {
                signal.cancel();
            }
        }
    };
    let mut work = WorkLedger::new(40_000_000_000);
    let error = PhaseCovariantGroupRetainedIstV1
        .reconstruct_with_context(
            &input,
            IstOptions::default().noiseless(),
            &mut nmr::ExecutionContext::new(&mut work)
                .with_cancellation(token)
                .with_progress(&mut progress),
        )
        .unwrap_err();
    assert_eq!(error, IstError::Cancelled);
    assert!(work.used() > 0 && work.used() < 40_000_000_000);
}
