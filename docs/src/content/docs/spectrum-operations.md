---
title: Spectrum operations and estimation
description: Explicit spectrum operations, staged estimation, dense and sparse paths, and offline derived data.
---

Use this reference when adding operations to an existing processed Dataset.
For a runnable raw-to-spectrum example, see [Processing and export](/processing/).
Keep a complex result until phase estimation is complete; scalar JCAMP-DX
spectra cannot supply the imaginary component required by phase estimators.

## Spectrum operations

Keep the `Dataset`. Axes, component bases, source evidence, warnings, acquisition
coordinates and observation order live there. `as_dense_processed()` is a checked
view; the full tensor layout is logical point followed by its component on each
axis. A rank-two hypercomplex tensor therefore stores
`[F1 point, F1 component, F2 point, F2 component]`. Do not treat its storage as
one complex number per logical point.

`ProcessingPlan::preflight` exposes resolved parameters, output descriptor,
resource estimates and `estimated_work()`. `execute_with_context` consumes the
preparation. Reuse the same `ExecutionContext` across all segments and estimators.
Errors name the local operation position. The host owns recipe identities,
representative selection, task scheduling, caching, undo, and UI state.

Use `ProcessingOperation::Spectrum { axis, operation }` for these operations:

| SpectrumOperation | Scientific meaning and boundary |
|---|---|
| `Reference { delta_ppm }` | Add `target-at` to an existing ppm axis; retain intensities and explicit/nonuniform coordinates. Repeated reference is supported. Hz-to-ppm conversion still requires `ResolveFrequencyFrame` with calibration evidence. |
| `Reverse`, `Invert` | Reverse intensities only, or negate all selected component fields. `ReverseAxis` additionally reverses coordinates and is a different operation. |
| `Affine { scale, real_offset }` | Multiply every component; add the offset only to the all-real tensor field. |
| `MovingAverage { window }` | Effective odd window, clipped edges with the actual count as divisor. N<3 is identity. |
| `SavitzkyGolay { window, order }` | Nearest complete window at edges, evaluated at the actual offset. Reorthogonalized QR; bounded degree 12, explicit failure on rank deficiency. |
| `Baseline(RealBaseline)` | Subtract from the selected real channel, retaining its imaginary partner. Other Cartesian fields are processed as independent selected-axis traces. |
| `Normalize(MaxPeak)` | Divide each selected complex trace by its largest complex norm. |
| `Normalize(TotalArea { singleton_width })` | Divide by `sum(abs(real))*abs(dx)`. Uniform frequency coordinates required; singletons require an explicit positive width. |
| `Normalize(Constant(divisor))` | Finite nonzero divisor, including negative values. Zero data with a data-dependent divisor returns an explicit no-usable-signal error. |
| `Bin { width, aggregation }` | Uniform input; `per=max(1,round(width/abs(dx)))`. Complex sum/mean, including the last short group. Output coordinates are exact group means and may be nonuniform. |
| `Magnitude` | Reduce only the selected Cartesian axis using a stable norm. Other Cartesian components survive. |
| `Slice { index, component }`, `Sum { component }`, `Skyline { component }` | Remove one of two axes. Explicitly choose its component; retain every component of the surviving axis. Skyline selects the original complex value by norm, first on ties. Fixed position and eliminated component are recorded. |
| `RetainRange { start, end }` | Keep a nonempty logical interval without removing the axis. Physical coordinates, absolute time origin and provenance survive. Suitable for a host's selected FID interval before window/zero-fill/FFT. |

Smoothing, binning and area normalization reject nonuniform coordinates when
their definition requires uniform spacing. Operations reject unresolved encoded
lanes: submit `ComponentTransform` first. A reduction explicitly selects the
eliminated-axis component to avoid silently discarding hypercomplex information;
repeat for the other component when both output fields are needed.

`Window::lorentz_to_gauss(lb_hz, gb_hz)` uses elapsed time `t=i*dt` and
`exp(pi*lb*t - (pi*gb*t)^2/(4*ln(2)))`. `lb` is signed; `gb` is nonnegative.
`lb=0` is Gaussian broadening. An identity window needs no dwell; a nonidentity
window needs positive uniform time calibration. Nonfinite weights are errors.

## Estimation and application are separate

`PhaseMethod::{AbsorptivePeak, Entropy, NegativeMinimization, PeakRegression,
RobustConsensus}` each has a distinct algorithm identifier. They are separate
from `NormalizedAcmeV1`, whose restricted semantics remain unchanged.

```rust
use nmr::processing::{PhaseMethod, ProcessingOptions, WorkLedger};
use nmr::ExecutionContext;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spectrum = nmr::read("data/complex-spectrum/pdata/1")?;
    let options = ProcessingOptions::new();
    let mut ledger = WorkLedger::new(1_000_000_000);
    let mut context = ExecutionContext::new(&mut ledger);
    let estimate = PhaseMethod::RobustConsensus.prepare(&spectrum, 0, options)?
        .estimate_with_context(&mut context)?;
    println!("{:?}, objective={}, evaluations={}", estimate.correction(),
        estimate.objective(), estimate.evaluations());
    let corrected = estimate.apply_with_context(&spectrum, options, &mut context)?;
    assert!(corrected.as_processed().is_some());
    Ok(())
}
```

The example requires a supported complex processed Bruker result with both
`1r` and `1i` in the selected directory; replace the path with your own input.

Phase analysis takes a rank-one Cartesian frequency spectrum in Hz or ppm.
Choose a representative pseudo-series slice explicitly. `estimate.correction()`
can then be submitted as one explicit `PhaseCorrection` on the whole series.
`estimate.apply` binds to the analyzed input and records the method and diagnostic.
Replay applies that recorded correction without another optimization.

### Phase method definitions

The estimator's internal endpoint convention is
`exp(-i*(p0+p1*i/(N-1)))`; returned `PhaseCorrection` converts exactly once to
the public positive exponent, degrees and `i/N - pivot` convention.
AbsorptivePeak rotates the dominant magnitude point with p1=0. Entropy minimizes
absolute-derivative Shannon entropy after maximum-complex-magnitude normalization,
plus 1000 times the sum of negative real squares on that normalized trace.
NegativeMinimization minimizes negative/total real energy.
PeakRegression fits unwrapped phases of separated local magnitude peaks above
5% of the maximum; insufficient peaks are an error. RobustConsensus considers
the distinct entropy, negative and usable regression solutions and refines a
dominant-peak-constrained entropy/negative-energy objective. The three objective
methods search the complete trace, with endpoint p1 in ±4π (±720°). First-order
phase is not equivalent modulo 360°: a turn changes the intermediate samples.
The deterministic grid has 48 zero-order values in [-π,π), spaced 7.5°, and
49 first-order values in [-4π,4π], spaced 15°. Each search refines its best grid
candidate with at most 100 four-direction iterations, starting at 10° and
halving the step after an iteration without improvement, stopping below 1e-5 rad.
Consensus evaluates at most four slope seeds, clipped to the same interval,
and at most 100 two-direction refinement iterations. Thus Entropy and
NegativeMinimization each evaluate at most 2752 candidates; RobustConsensus
evaluates at most `2*2752 + 4 + 2*100 = 5708`. The execution guard and preflight
work bound use this same structural ceiling. Preflight also reserves normalization
and application work; actual evaluation counts and work describe executed search.

The search profiles are `phase-derivative-entropy.v1`, `phase-negative-energy.v1`
and `phase-robust-consensus.v1`. AbsorptivePeak and PeakRegression use their v1
algorithms. The search recovers the noisy 500° four-Lorentzian
case with real NRMS 0.236406 (<0.35); the other seven retained gates and the
RobustConsensus/AsLS quality gates remain required. The independent inputs and
original thresholds live in `tests/processing/phase/reference_signals.rs` and are enforced by
`tests/processing/phase/quality_gates.rs`, including negative ramps and intensity scaling.
Run `cargo test --test processing --locked phase::quality_gates::` to execute these gates.
This is an empirical automatic estimate, not proof of absorptive phase.
The finite search interval and local refinement do not guarantee a global optimum,
recovery outside the interval, or fidelity on arbitrary experimental spectra.

### Estimate or apply a baseline

`RealBaseline::{Offset, Polynomial { order }, Asls { lambda, asymmetry,
iterations }}` can either run in a plan or use `method.prepare(...).estimate()`.
The latter returns `values()`, `coefficients()` and `method()` before application.
`BaselineEstimate::apply_with_context` records fitted values for exact replay.
Polynomial coefficients use ascending powers of normalized index
`t=2*i/(N-1)-1`; they are not ppm coefficients. Offset uses median/MAD clipped
mean, Polynomial fits lower-half anchors, and AsLS uses index second differences
with first-round unit weights. AsLS accepts lambda in [1,1e12], asymmetry in
[1e-6,0.5], iterations in [1,100]; submit 50000/0.001/20 explicitly for that
common choice. No method silently falls back to Offset. `PositivePeaksV1`
remains its separately versioned continuous-coordinate baseline profile.

## Combine two spectra

`LinearCombination::new(scale)?.prepare(&a, &b, options)?` computes
`a + scale*b` on a's grid. b is linearly interpolated channel by channel, and
contributes zero outside its range; a singleton contributes only at its exact
coordinate. Both directions and monotonic nonuniform grids are supported.
Nucleus, unit and component basis must agree. Its `ProcessedOrigin::Library`
retains both ordered inputs, source context and original histories.
`LibraryDerivation::replay` requires both original datasets in the same order.
Subsequent single-input history starts at this checked boundary. Applying the
archived result again requires no original files or parent sample buffers.

## Reconstruct with explicit noise evidence

`NusSettings { max_iterations, noise_standard_deviation }.prepare(&raw,
direct_plan, options)` prepares explicit direct-axis operations followed by
`GeneralGridPhaseCovariantGroupRetainedIstV1`. Only acquired observations enter F2 kernels.
The direct plan may decode indirect components explicitly, and must establish
F2 frequency data and Cartesian indirect components. `measured_indices()` retains
original observation order; `direct_steps()` previews the per-observation plan.
The returned dense dataset is mixed-domain, ready for a separate explicit F1 FFT.
Full-grid resolved F2 facts, requested operations, actual iterations, column
thresholds and relative changes are retained in `DerivationOperation` and JSON.

No missing-data padding, lane interpretation or descriptor rebuilding is delegated
to the host. This preparation executes F2 plus reconstruction as one checked
segment; an independently materialized sparse processed intermediate is not
exposed. Ordinary `ProcessingPlan` still rejects sparse raw input. Duplicates
survive reading and snapshots but are explicitly rejected for reconstruction.
Noise sigma is mandatory in processed F2 amplitude units; `Some(0.0)` is an
explicit noiseless assertion. Unknown noise never becomes zero. Full sampling
and zero data have documented immediate solutions. The separate fixed-grid
`PhaseCovariantGroupRetainedIstV1` kernel accepts exactly 512 logical points and
128 observations, with a fixed threshold continuation schedule. These are distinct
numerical profiles, not compatibility versions of the same API.

## Validation evidence

Analytic tests use independent signals, not old application output as truth.
The retained automatic phase and baseline black-box cases keep their original
thresholds. The Lorentz-to-Gauss line-shape gate independently measures 10.001154 Hz
for a 10 Hz Gaussian and 12.002772 Hz for a 12 Hz Lorentzian (0.02 Hz tolerance);
the Gaussian absorption and absolute peak also match the analytic Fourier integral.
Savitzky-Golay retains the edge cubic and attenuates alternating noise below 0.3
of its input amplitude in the interior. RobustConsensus achieves NRMS about 0.319565 on the difficult case,
close to its 0.32 threshold; this narrow margin limits the inference claim.
Configurable AsLS baseline NRMS is 0.0724–0.0914 on the three retained cases.
The general IST gate checks 4/8/15/512 grids, 100:1 signals, missing values,
coherent amplitude and normalized time-domain magnitude area to 1e-5. These
tone gates do not prove general off-grid or arbitrary-density NUS recovery.

`cargo test --test processing --test workflows --locked` runs the public and independent
gates. Snapshot tests use synthetic inputs rather than frozen binary archives.

Complete synthetic workflows are in `tests/support/workflows.rs`; run
`cargo test --test workflows --test snapshot --locked` for application flows and
offline restoration. See [host integration](/host-integration/) for snapshot APIs.
