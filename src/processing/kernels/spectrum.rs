use super::{buffer::*, tensor::*};
use crate::ExecutionContext;
use crate::processing::contracts::{
    error::ProcessingError, operation::*, prepared_step::PreparedStep,
};

pub(crate) use crate::processing::contracts::spectrum::evaluate;
use crate::processing::contracts::spectrum::{bin_points, uniform_step};

pub(in crate::processing) fn work(
    operation: &SpectrumOperation,
    points: usize,
    samples: usize,
) -> Result<u128, ProcessingError> {
    let n = points as u128;
    let factor = match operation {
        SpectrumOperation::SavitzkyGolay { window, order } => {
            (*window as u128) * (*order as u128 + 1).pow(2) * 8
        }
        SpectrumOperation::MovingAverage { window } => *window as u128 + 4,
        SpectrumOperation::Baseline(RealBaseline::Polynomial { order }) => {
            n + (*order as u128 + 1).pow(2) * 8
        }
        SpectrumOperation::Baseline(RealBaseline::Asls { iterations, .. }) => {
            *iterations as u128 * 64
        }
        SpectrumOperation::Baseline(RealBaseline::Offset) => n + 16,
        _ => 16,
    };
    (samples as u128)
        .checked_mul(factor)
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn working_bytes(points: usize) -> Result<usize, ProcessingError> {
    // Gather, normalized signal, output, banded solver and bounded degree-12 QR.
    points
        .checked_mul(64 * 8)
        .and_then(|v| v.checked_add(16 * 16 * 8))
        .ok_or(ProcessingError::SizeOverflow)
}

pub(in crate::processing) fn execute(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
    operation: &SpectrumOperation,
) -> Result<Vec<f64>, ProcessingError> {
    let selected = step.axis();
    let axis = &step.before.axes()[selected];
    let shape = storage_shape(&step.before)?;
    let next_shape = storage_shape(&step.after)?;
    let n = axis.points();
    let c = axis.component_count();
    let estimate = work(operation, n, current.len())?;
    control.charge(estimate)?;
    use SpectrumOperation as Op;
    if matches!(operation, Op::Reverse) {
        return reverse_axis_unmetered(control, current, step);
    }
    let mut output = try_zeroed(checked_product(&next_shape)?)?;
    if operation.removes_axis() {
        let remaining = 1 - selected;
        let surviving = &step.before.axes()[remaining];
        let k = surviving.component_count();
        let output_components = step.after.axes()[0].component_count();
        let shared_orientation = match surviving.component_basis() {
            crate::processed::ComponentBasis::SharedComplex { conjugated, .. } => {
                Some(if *conjugated { -1.0 } else { 1.0 })
            }
            _ => None,
        };
        let (component, fixed) = match operation {
            Op::Slice { index, component } => (*component, Some(*index)),
            Op::Sum { component } | Op::Skyline { component } => (*component, None),
            _ => unreachable!(),
        };
        for p in 0..surviving.points() {
            control.check_cancelled()?;
            let at = |row: usize, lane: usize| -> Result<f64, ProcessingError> {
                let mut coord = [0; 2];
                if let Some(orientation) = shared_orientation {
                    // The fixed axis stores both fields. Move them together,
                    // conjugating only when the surviving axis requires it.
                    coord[selected] = row * c + lane;
                    coord[remaining] = p;
                    let value = current[flatten(&shape, &coord)?];
                    Ok(if lane == 1 {
                        orientation * value
                    } else {
                        value
                    })
                } else {
                    coord[selected] = row * c + component;
                    coord[remaining] = p * k + lane;
                    Ok(current[flatten(&shape, &coord)?])
                }
            };
            let mut winner = fixed.unwrap_or(0);
            if matches!(operation, Op::Skyline { .. }) {
                let norm = |row| -> Result<f64, ProcessingError> {
                    let mut norm: f64 = 0.0;
                    for lane in 0..k {
                        norm = norm.hypot(at(row, lane)?);
                    }
                    Ok(norm)
                };
                let mut largest = norm(0)?;
                for row in 1..n {
                    if row % 4096 == 0 {
                        control.check_cancelled()?;
                    }
                    let next = norm(row)?;
                    if next > largest {
                        largest = next;
                        winner = row;
                    }
                }
            }
            for lane in 0..output_components {
                output[p * output_components + lane] = if matches!(operation, Op::Sum { .. }) {
                    let mut sum = crate::internal::numeric::ScaledNeumaier::default();
                    for row in 0..n {
                        if row % 4096 == 0 {
                            control.check_cancelled()?;
                        }
                        sum.add(at(row, lane)?);
                    }
                    sum.total()
                } else {
                    at(winner, lane)?
                };
            }
        }
    } else {
        let next_c = step.after.axes()[selected].component_count();
        for line in 0..other_line_count(&shape, selected)? {
            control.check_cancelled()?;
            let other = other_coordinate(&shape, selected, line)?;
            let mut lanes = Vec::with_capacity(c);
            for lane in 0..c {
                let mut values = try_vec_capacity(n)?;
                for p in 0..n {
                    values.push(sample_at(current, &shape, selected, &other, p * c + lane)?);
                }
                lanes.push(values);
            }
            match operation {
                Op::RetainRange { start, end } => {
                    for values in &mut lanes {
                        *values = values[*start..*end].to_vec();
                    }
                }
                Op::Reference { .. } => {}
                Op::Invert => {
                    for lane in &mut lanes {
                        for v in lane {
                            *v = -*v;
                        }
                    }
                }
                Op::Affine { scale, real_offset } => {
                    let mut index = 0;
                    let all_real = step
                        .before
                        .axes()
                        .iter()
                        .enumerate()
                        .filter(|(a, _)| *a != selected)
                        .all(|(_, a)| {
                            let real = other[index] % a.component_count() == 0;
                            index += 1;
                            real
                        });
                    for (lane, values) in lanes.iter_mut().enumerate() {
                        for v in values {
                            *v *= scale;
                            if lane == 0 && all_real {
                                *v += real_offset;
                            }
                        }
                    }
                }
                Op::MovingAverage { window } => {
                    for values in &mut lanes {
                        let old = values.clone();
                        for (p, value) in values.iter_mut().enumerate() {
                            if p % 128 == 0 {
                                control.check_cancelled()?;
                            }
                            let start = p.saturating_sub(window / 2);
                            let end = p.saturating_add(window / 2 + 1).min(n);
                            *value = old[start..end]
                                .iter()
                                .map(|v| v / (end - start) as f64)
                                .sum();
                        }
                    }
                }
                Op::SavitzkyGolay { window, order } => {
                    if *window > 1 {
                        for values in &mut lanes {
                            let old = values.clone();
                            for (p, value) in values.iter_mut().enumerate() {
                                control.check_cancelled()?;
                                let start = p.saturating_sub(window / 2).min(n - window);
                                let rows: Vec<_> = (0..*window)
                                    .map(|i| {
                                        (
                                            2.0 * i as f64 / (*window - 1) as f64 - 1.0,
                                            old[start + i],
                                        )
                                    })
                                    .collect();
                                let coefficients = polynomial_fit(&rows, *order, control)?;
                                *value = evaluate(
                                    &coefficients,
                                    2.0 * (p - start) as f64 / (*window - 1) as f64 - 1.0,
                                );
                            }
                        }
                    }
                }
                Op::Baseline(method) => {
                    let baseline = estimate_baseline(&lanes[0], *method, control)?;
                    for (v, b) in lanes[0].iter_mut().zip(baseline) {
                        *v -= b;
                    }
                }
                Op::Normalize(mode) => {
                    let divisor = match mode {
                        Normalization::Constant(value) => *value,
                        Normalization::MaxPeak => (0..n)
                            .map(|p| lanes.iter().fold(0.0_f64, |a, l| a.hypot(l[p])))
                            .fold(0.0, f64::max),
                        Normalization::TotalArea { singleton_width } => {
                            let width = if n == 1 {
                                singleton_width.unwrap()
                            } else {
                                uniform_step(axis)?.abs()
                            };
                            lanes[0].iter().map(|v| v.abs() * width).sum()
                        }
                    };
                    if divisor == 0.0 {
                        return Err(ProcessingError::InvalidParameter(
                            "no usable signal to normalize",
                        ));
                    }
                    if !divisor.is_finite() {
                        return Err(ProcessingError::NumericalInvariantViolation);
                    }
                    for values in &mut lanes {
                        for v in values {
                            *v /= divisor;
                        }
                    }
                }
                Op::Bin { width, aggregation } => {
                    let per = bin_points(axis, *width)?;
                    for values in &mut lanes {
                        *values = values
                            .chunks(per)
                            .map(|chunk| {
                                let divisor = if *aggregation == BinAggregation::Mean {
                                    chunk.len() as f64
                                } else {
                                    1.0
                                };
                                chunk.iter().map(|v| v / divisor).sum()
                            })
                            .collect();
                    }
                }
                Op::Magnitude => {
                    let magnitudes = (0..n)
                        .map(|p| lanes.iter().fold(0.0_f64, |a, l| a.hypot(l[p])))
                        .collect();
                    lanes = vec![magnitudes];
                }
                Op::Reverse | Op::Slice { .. } | Op::Sum { .. } | Op::Skyline { .. } => {
                    unreachable!()
                }
            }
            for (lane, values) in lanes.iter().enumerate() {
                for (p, &v) in values.iter().enumerate() {
                    set_sample(
                        &mut output,
                        &next_shape,
                        selected,
                        &other,
                        p * next_c + lane,
                        v,
                    )?;
                }
            }
        }
    }
    if output.iter().any(|v| !v.is_finite()) {
        return Err(ProcessingError::NumericalInvariantViolation);
    }
    Ok(output)
}

pub(in crate::processing) fn reverse_axis_unmetered(
    control: &mut ExecutionContext<'_>,
    current: &[f64],
    step: &PreparedStep,
) -> Result<Vec<f64>, ProcessingError> {
    let shape = storage_shape(&step.before)?;
    let axis = step.axis();
    let c = step.before.axes()[axis].component_count();
    let n = step.before.axes()[axis].points();
    let mut output = try_zeroed(current.len())?;
    for (i, v) in output.iter_mut().enumerate() {
        if i % 4096 == 0 {
            control.check_cancelled()?;
        }
        let mut coord = unflatten(&shape, i)?;
        coord[axis] = (n - 1 - coord[axis] / c) * c + coord[axis] % c;
        *v = current[flatten(&shape, &coord)?];
    }
    Ok(output)
}

pub(crate) fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 0 {
        sorted[n / 2 - 1] / 2.0 + sorted[n / 2] / 2.0
    } else {
        sorted[n / 2]
    }
}

/// Reorthogonalized QR avoids squared conditioning of normal equations.
pub(in crate::processing) fn polynomial_fit(
    rows: &[(f64, f64)],
    order: usize,
    control: &mut ExecutionContext<'_>,
) -> Result<Vec<f64>, ProcessingError> {
    let m = order + 1;
    if rows.len() < m {
        return Err(ProcessingError::InvalidParameter(
            "insufficient polynomial anchors",
        ));
    }
    let mut q: Vec<Vec<f64>> = Vec::with_capacity(m);
    let mut r = vec![vec![0.0; m]; m];
    let mut b = vec![0.0; m];
    for k in 0..m {
        control.check_cancelled()?;
        let mut v: Vec<f64> = rows.iter().map(|(x, _)| x.powi(k as i32)).collect();
        for _ in 0..2 {
            for j in 0..k {
                let dot: f64 = v.iter().zip(&q[j]).map(|(a, b)| a * b).sum();
                r[j][k] += dot;
                for (v, q) in v.iter_mut().zip(&q[j]) {
                    *v -= dot * q;
                }
            }
        }
        let norm = v.iter().fold(0.0_f64, |a, v| a.hypot(*v));
        if !norm.is_finite() || norm <= 1e-13 {
            return Err(ProcessingError::InvalidParameter(
                "rank-deficient polynomial anchors",
            ));
        }
        r[k][k] = norm;
        for v in &mut v {
            *v /= norm;
        }
        b[k] = v.iter().zip(rows).map(|(q, (_, y))| q * y).sum();
        q.push(v);
    }
    for k in (0..m).rev() {
        for j in k + 1..m {
            b[k] -= r[k][j] * b[j];
        }
        b[k] /= r[k][k];
    }
    if b.iter().any(|v| !v.is_finite()) {
        return Err(ProcessingError::NumericalInvariantViolation);
    }
    Ok(b)
}

pub(in crate::processing) fn estimate_baseline(
    y: &[f64],
    method: RealBaseline,
    control: &mut ExecutionContext<'_>,
) -> Result<Vec<f64>, ProcessingError> {
    Ok(fit_baseline(y, method, control)?.0)
}

pub(in crate::processing) fn fit_baseline(
    y: &[f64],
    method: RealBaseline,
    control: &mut ExecutionContext<'_>,
) -> Result<(Vec<f64>, Vec<f64>), ProcessingError> {
    let n = y.len();
    let mut fitted_coefficients = Vec::new();
    let scale = y.iter().map(|v| v.abs()).fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok((
            try_zeroed(n)?,
            match method {
                RealBaseline::Offset => vec![0.0],
                RealBaseline::Polynomial { order } => vec![0.0; order + 1],
                RealBaseline::Asls { .. } => vec![],
            },
        ));
    }
    let normalized: Vec<_> = y.iter().map(|v| v / scale).collect();
    let y = &normalized;
    let mut sorted = y.clone();
    sorted.sort_by(f64::total_cmp);
    let result = match method {
        RealBaseline::Offset => {
            let center = median(&sorted);
            let mut deviations: Vec<_> = sorted.iter().map(|v| (v - center).abs()).collect();
            deviations.sort_by(f64::total_cmp);
            let sigma = median(&deviations) / 0.674_489_75;
            let baseline = if sigma == 0.0 {
                center
            } else {
                let retained: Vec<_> = sorted
                    .iter()
                    .copied()
                    .filter(|v| (v - center).abs() <= 3.0 * sigma)
                    .collect();
                retained.iter().map(|v| v / retained.len() as f64).sum()
            };
            fitted_coefficients.push(baseline * scale);
            vec![baseline; n]
        }
        RealBaseline::Polynomial { order } => {
            let threshold = sorted[n / 2];
            let rows: Vec<_> = y
                .iter()
                .enumerate()
                .filter(|(_, v)| **v <= threshold)
                .map(|(i, v)| (2.0 * i as f64 / (n - 1).max(1) as f64 - 1.0, *v))
                .collect();
            let coefficients = polynomial_fit(&rows, order, control)?;
            fitted_coefficients = coefficients.iter().map(|v| v * scale).collect();
            (0..n)
                .map(|i| evaluate(&coefficients, 2.0 * i as f64 / (n - 1).max(1) as f64 - 1.0))
                .collect()
        }
        RealBaseline::Asls {
            lambda,
            asymmetry,
            iterations,
        } => {
            let mut weights = vec![1.0; n];
            let mut baseline = vec![0.0; n];
            for _ in 0..iterations {
                control.check_cancelled()?;
                let mut diagonal = weights.clone();
                let mut first = vec![0.0; n];
                for i in 0..n - 2 {
                    diagonal[i] += lambda;
                    diagonal[i + 1] += 4.0 * lambda;
                    diagonal[i + 2] += lambda;
                    first[i + 1] -= 2.0 * lambda;
                    first[i + 2] -= 2.0 * lambda;
                }
                let mut l1 = vec![0.0; n];
                let mut l2 = vec![0.0; n];
                for i in 0..n {
                    if i % 4096 == 0 {
                        control.check_cancelled()?;
                    }
                    if i >= 2 {
                        l2[i] = lambda / diagonal[i - 2];
                    }
                    if i >= 1 {
                        l1[i] = (first[i] - l2[i] * l1[i - 1]) / diagonal[i - 1];
                    }
                    let d = diagonal[i] - l1[i] * l1[i] - l2[i] * l2[i];
                    if !d.is_finite() || d <= 0.0 {
                        return Err(ProcessingError::NumericalInvariantViolation);
                    }
                    diagonal[i] = d.sqrt();
                    baseline[i] = weights[i] * y[i];
                    if i >= 1 {
                        baseline[i] -= l1[i] * baseline[i - 1];
                    }
                    if i >= 2 {
                        baseline[i] -= l2[i] * baseline[i - 2];
                    }
                    baseline[i] /= diagonal[i];
                }
                for i in (0..n).rev() {
                    if i + 1 < n {
                        baseline[i] -= l1[i + 1] * baseline[i + 1];
                    }
                    if i + 2 < n {
                        baseline[i] -= l2[i + 2] * baseline[i + 2];
                    }
                    baseline[i] /= diagonal[i];
                    weights[i] = if y[i] > baseline[i] {
                        asymmetry
                    } else {
                        1.0 - asymmetry
                    };
                }
            }
            baseline
        }
    };
    Ok((
        result.into_iter().map(|v| v * scale).collect(),
        fitted_coefficients,
    ))
}
