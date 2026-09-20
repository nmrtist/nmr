//! Audited allocation bounds for the pinned explicit-processing FFT backend.
//! Recheck planning and scratch bounds when changing the dependency.

use crate::Complex64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FftError {
    SizeOverflow,
    InvalidLength,
    ScratchBound,
}
use rustfft::algorithm::{BluesteinsAlgorithm, Radix4, butterflies::*};
use rustfft::{Fft, FftDirection};
use std::mem::{align_of, size_of};
use std::sync::Arc;

pub(crate) const BACKEND: &str = "rustfft-6.4.1.radix4-bluestein.v1";

#[derive(Clone, Copy, Debug)]
pub(crate) struct FftBounds {
    pub(crate) planning: usize,
    pub(crate) retained: usize,
    pub(crate) scratch: usize,
}

fn add(a: usize, b: usize) -> Result<usize, FftError> {
    a.checked_add(b).ok_or(FftError::SizeOverflow)
}

fn complex_bytes(count: usize) -> Result<usize, FftError> {
    count
        .checked_mul(size_of::<Complex64>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(FftError::SizeOverflow)
}

// Two Arc counters plus enough padding for either header or payload alignment.
fn arc_bytes<T>() -> usize {
    size_of::<T>() + 2 * size_of::<usize>() + align_of::<T>().max(align_of::<usize>())
}

fn radix_bounds(points: usize) -> Result<FftBounds, FftError> {
    let base = [
        arc_bytes::<Butterfly1<f64>>(),
        arc_bytes::<Butterfly2<f64>>(),
        arc_bytes::<Butterfly4<f64>>(),
        arc_bytes::<Butterfly8<f64>>(),
        arc_bytes::<Butterfly16<f64>>(),
        arc_bytes::<Butterfly32<f64>>(),
    ]
    .into_iter()
    .max()
    .unwrap();
    let fixed = add(base, arc_bytes::<Radix4<f64>>())?;
    let bytes = complex_bytes(points)?;
    let twiddles = complex_bytes(points.checked_mul(2).ok_or(FftError::SizeOverflow)?)?;
    Ok(FftBounds {
        planning: add(add(twiddles, bytes)?, fixed)?,
        retained: add(twiddles, fixed)?,
        scratch: bytes,
    })
}

fn convolution_points(points: usize) -> Result<usize, FftError> {
    points
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .and_then(usize::checked_next_power_of_two)
        .ok_or(FftError::SizeOverflow)
}

pub(crate) fn bounds(points: usize) -> Result<FftBounds, FftError> {
    if points == 0 {
        return Err(FftError::InvalidLength);
    }
    if points.is_power_of_two() {
        return radix_bounds(points);
    }
    let inner = convolution_points(points)?;
    let radix = radix_bounds(inner)?;
    let n = complex_bytes(points)?;
    let m = complex_bytes(inner)?;
    let fixed = arc_bytes::<BluesteinsAlgorithm<f64>>();
    Ok(FftBounds {
        planning: add(
            radix.planning,
            add(add(add(m, add(m, m)?)?, add(n, n)?)?, fixed)?,
        )?,
        retained: add(radix.retained, add(add(m, n)?, fixed)?)?,
        scratch: add(m, m)?,
    })
}

pub(crate) fn working_bytes(points: usize, simultaneous_plans: usize) -> Result<usize, FftError> {
    let bound = bounds(points)?;
    let peak = bound.planning.max(add(bound.retained, bound.scratch)?);
    let retained = bound
        .retained
        .checked_mul(simultaneous_plans.saturating_sub(1))
        .ok_or(FftError::SizeOverflow)?;
    add(retained, peak)
}

/// Call only after checking the operation's numeric buffers plus `working_bytes`.
pub(crate) fn plan(points: usize, direction: FftDirection) -> Result<Arc<dyn Fft<f64>>, FftError> {
    let bound = bounds(points)?;
    #[cfg(test)]
    PLAN_CALLS.with(|calls| calls.set(calls.get() + 1));
    let fft: Arc<dyn Fft<f64>> = if points.is_power_of_two() {
        Arc::new(Radix4::new(points, direction))
    } else {
        let inner = Arc::new(Radix4::new(convolution_points(points)?, direction));
        Arc::new(BluesteinsAlgorithm::new(points, inner))
    };
    if complex_bytes(fft.get_inplace_scratch_len())? > bound.scratch {
        return Err(FftError::ScratchBound);
    }
    Ok(fft)
}

#[cfg(test)]
thread_local! { static PLAN_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
pub(crate) fn plan_calls() -> usize {
    PLAN_CALLS.with(std::cell::Cell::get)
}

pub(in crate::processing) fn fft_work(points: usize) -> u128 {
    (points as u128) * u128::from(usize::BITS - points.leading_zeros()) * 8
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::processing::contracts::error::ProcessingError;

    #[test]
    fn both_directions_match_an_independent_dft_across_algorithm_boundaries() {
        for points in [1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64] {
            for direction in [FftDirection::Forward, FftDirection::Inverse] {
                let original: Vec<_> = (0..points)
                    .map(|i| Complex64::new((i % 5) as f64 - 2.0, (i % 3) as f64 - 1.0))
                    .collect();
                let mut actual = original.clone();
                let fft = plan(points, direction).unwrap();
                let mut scratch = vec![Complex64::default(); fft.get_inplace_scratch_len()];
                fft.process_with_scratch(&mut actual, &mut scratch);
                for (q, sample) in actual.iter().enumerate() {
                    let sign = if direction == FftDirection::Forward {
                        -1.0
                    } else {
                        1.0
                    };
                    let expected: Complex64 = original
                        .iter()
                        .enumerate()
                        .map(|(n, &value)| {
                            let angle =
                                sign * std::f64::consts::TAU * (n * q) as f64 / points as f64;
                            value * Complex64::new(angle.cos(), angle.sin())
                        })
                        .sum();
                    assert!(
                        (*sample - expected).norm() <= 1e-10 * (1.0 + expected.norm()),
                        "N={points}, q={q}: {sample:?} vs {expected:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_or_overflowing_lengths_fail_before_creating_a_plan() {
        let before = plan_calls();
        assert!(matches!(
            plan(0, FftDirection::Forward),
            Err(FftError::InvalidLength)
        ));
        assert!(matches!(
            plan(usize::MAX, FftDirection::Forward),
            Err(FftError::SizeOverflow)
        ));
        assert_eq!(plan_calls(), before);
    }

    #[test]
    fn preflight_rejects_fft_and_shift_fold_budgets_before_any_plan_creation() {
        use crate::axis::{AxisCoordinates, AxisDomain, AxisUnit};
        use crate::processing::{
            DelaySource, DigitalFilterCorrection, FourierTransform, ProcessingOperation,
            ProcessingOptions, ProcessingPlan, TimeDomainResidualPolicy,
        };
        use crate::raw::{DirectSamples, RawAxis, RawAxisKind, RawDatasetBuilder, RawMetadata};
        for points in [2048, 2053] {
            let axis = RawAxis::new(
                RawAxisKind::Direct(DirectSamples::Complex),
                AxisDomain::Time,
                Some(AxisUnit::Second),
                points,
                AxisCoordinates::Uniform {
                    start: 0.0,
                    step: 0.001,
                },
            )
            .unwrap();
            let mut samples = vec![Complex64::default(); points];
            samples[0] = Complex64::new(1.0, 0.0);
            let input = crate::Dataset::from_raw(
                RawDatasetBuilder::new(vec![axis], RawMetadata::default())
                    .unwrap()
                    .dense(samples)
                    .unwrap(),
            );
            for operation in [
                ProcessingOperation::FourierTransform {
                    axis: 0,
                    transform: FourierTransform::default(),
                },
                ProcessingOperation::DigitalFilterCorrection {
                    axis: 0,
                    correction: DigitalFilterCorrection::TimeDomainShiftFoldV1 {
                        source: DelaySource::Explicit(0.75),
                        policy: TimeDomainResidualPolicy::CorrectFully,
                    },
                },
            ] {
                let before = plan_calls();
                let plan = ProcessingPlan::new(vec![operation]).unwrap();
                // The second budget fits current/output/gather buffers but not
                // the FFT plan and scratch: omitting the backend bound must fail.
                let prepared_bytes =
                    crate::processing::prepare::resources::prepared_retained_bytes(1, 1, true)
                        .unwrap()
                        + crate::processing::prepare::resources::reserved_raw_axis_bytes(
                            input.as_raw().unwrap(),
                            plan.operations(),
                        )
                        .unwrap()
                        + crate::processing::prepare::resources::state_storage_bytes(1, 0).unwrap()
                        + crate::processing::prepare::memory::raw_apply(input.as_raw().unwrap(), 1)
                            .unwrap();
                for budget in [1, points * size_of::<Complex64>() * 3 + prepared_bytes] {
                    assert!(matches!(
                        plan.preflight(&input, ProcessingOptions::new().max_working_bytes(budget))
                            .map_err(ProcessingError::into_root_cause),
                        Err(ProcessingError::LimitExceeded(
                            crate::resource::LimitExceeded {
                                resource: crate::resource::ResourceKind::WorkingBytes,
                                ..
                            }
                        ))
                    ));
                }
                assert_eq!(plan_calls(), before);
            }
            let before = plan_calls();
            let plan = ProcessingPlan::new(vec![ProcessingOperation::FourierTransform {
                axis: 0,
                transform: FourierTransform::default(),
            }])
            .unwrap();
            let prepared = plan
                .preflight(&input, ProcessingOptions::default())
                .unwrap();
            assert_eq!(plan_calls(), before);
            let output = prepared.execute().unwrap();
            assert_eq!(plan_calls(), before + 1);
            for pair in output
                .as_dense_processed()
                .unwrap()
                .samples()
                .chunks_exact(2)
            {
                assert!((pair[0] - 1.0).abs() < 1e-10 && pair[1].abs() < 1e-10);
            }
        }
    }
}
