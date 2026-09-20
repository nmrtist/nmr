---
title: Automatic NUS reconstruction
description: Public automatic noise evidence, reconstruction, limits and downstream integration.
---

The noise estimator and reconstruction are experimental. Successful convergence
does not establish recovered peak accuracy outside the tested domains below.
To try the complete synthetic workflow without external data, run:

```console
cargo run --release --example nus_auto --locked
```

Expect a noise report and `finite 2D frequency shape=[256, 128]`. The example
checks that every output sample is finite and does not write an NPZ file.
It grants its advertised work bound explicitly; elapsed time is machine-dependent.
For real inputs, [reader opt-in and format restrictions](/formats/) still apply.

## Public workflow

`AutoNusSettings` accepts a raw rank-two Dataset and an explicit direct-axis
`ProcessingPlan`. It processes only acquired observations, decodes declared
Cartesian/echo-antiecho components or preserves SharedComplex, estimates noise,
then reconstructs the full
indirect grid. The returned Dataset is dense with F1 in time and F2 in frequency.
Apply an ordinary F1 plan next. No sigma, noise region or observation matrix is
required from the caller. The host remains responsible for choosing its recipe
and granting a work/memory budget.

```rust
use nmr::processing::{AutoNusSettings, ProcessingOptions, ProcessingPlan,
    ProcessingOperation as Op, FourierTransform};
fn run(raw: &nmr::Dataset, direct_plan: ProcessingPlan) -> Result<nmr::Dataset, nmr::processing::ProcessingError> {
let prepared = AutoNusSettings::default()
    .prepare(raw, direct_plan, ProcessingOptions::new())?;
let analyzed = prepared.analyze()?;
let noise = analyzed.noise_report(); // actual sigma and diagnostics, before IST
println!("{}: sigma={}", noise.method_version(), noise.sigma);
let mixed = analyzed.execute()?; // consumes cached F2 observations, no second F2 pass
ProcessingPlan::new(vec![Op::FourierTransform {
    axis: 0, transform: FourierTransform::default(),
}])?.apply(&mixed)
}
```

For background jobs use the corresponding `*_with_context` methods with one
`ExecutionContext`. Inspect `prepared.resources()` and `estimated_work()` before
execution. `analyze_with_context` reserves capacity for the complete operation;
`execute_with_context` charges the remaining reconstruction against the same
ledger. Dropping the analyzed object produces no dataset. A cancelled,
over-budget or unconverged execution never returns a partial successful Dataset.
The default automatic ceiling is 1000 iterations; this is a ceiling, not an
instruction to run every iteration or evidence of convergence. The default
shared ledger may be too small for a large grid: hosts must explicitly grant
the advertised bound rather than silently removing the resource limit.

`NusSettings` retains its explicit sigma semantics: `Some(0.0)` asserts noiseless
input and `None` is an error. Automatic policy is a separate request type, not
a zero sentinel. The separate fixed-grid IST profile requires explicit noise.
General-grid IST permits blank/low-signal columns: start at the noise floor,
apply the same shrink-and-replace map, and require the usual three converged
rounds. Below-threshold zero-filled observations are a fixed point of that map;
measured values, including noise, remain bit-for-bit unchanged. Successful
pure-noise output is not a claim that peaks were recovered.

## Convergence method

The general-grid kernel uses the public IST entry point.
Threshold continuation is capped at the existing 0.98 multiplier so increasing
an iteration ceiling does not make continuation slower. Once a column reaches
its fixed floor, use the usual FISTA recurrence:

```text
t_next = (1 + sqrt(1 + 4*t*t))/2
x_next = T(y)
y_next = x_next + ((t-1)/t_next)*(x_next-x_previous)
```

`T` is the existing Fourier group soft-threshold / inverse Fourier / measured
sample replacement map. The threshold and measured-data constraint do not
change. Extrapolation starts only at the fixed floor, and never performs
arithmetic on measured samples. Convergence requires three consecutive relative
`||T(y)-y||/||y|| <= 1e-6` checks, not a small momentum step. Low/no-signal and
full-sampling cases retain their explicit behavior. The added previous-iterate
buffer, scans and arithmetic are included in preflight and work charging.

This adaptation has a direct mathematical interpretation: writing
`g(x)=(lambda/N)*sum_k ||FFT(x)[k]||`, its proximity operator is the existing
Fourier shrink map. The smooth Moreau envelope of `g` has gradient
`x-prox_g(x)` with Lipschitz constant 1. Projected gradient on that envelope
under the affine measured-data constraint gives exactly `T`. Thus the
[FISTA recurrence](https://www.cs.cmu.edu/~airg/readings/2012_02_21_a_fast_iterative_shrinkage-thresholding.pdf)
applies at a fixed threshold. This is not a claim to implement hmsIST's entire
algorithm or a different unconstrained LASSO objective.

An independent O(N^2) direct-DFT test checks the returned fixed-point residual
against `1.01e-6` (allowing the nonexpansive-map/output normalization difference),
and compares to an ordinary fixed-threshold iteration converged to `1e-10`,
requiring relative solution disagreement below `1e-3`. Analytic strong/weak peak
and retained-observation tests remain separate.

## Noise method and assumptions

`split-observation-cartesian-rms.v1` operates on the actual acquired-row F2
output, before any F1 zero filling or reconstruction. For each complex indirect
sample it groups every real Cartesian component: dimension `k=2` or `k=4`.
It does not discard imaginary values or infer phases. The assumed noise is
zero-mean Gaussian, isotropic across these components, approximately stationary
across F2, and independent between acquisition observations. Applying known
phase/component rotations preserves the grouped energy.

There are three disjoint folds in **original acquisition observation order**:
ordinal modulo 3 equal to 0 selects regions, 1 validates them, and 2 estimates
sigma. Divide the final F2 grid into 32 integer-boundary blocks; in each spectral
quarter, choose the block with lowest mean grouped energy on the selection fold.
Ties use the lowest bin index. The validation fold never reselects candidates
or retries rejected regions. Requiring coverage of all four quarters reduces the
risk of treating one coloured-noise trough as a global noise level.

For final-estimation scalar samples `y_j`,

```text
sigma = sqrt(sum_j y_j^2 / S)
```

All sums use a common scale to avoid squaring large input amplitudes. This is
an RMS estimate with unbiased variance under the stated zero-mean model; sigma
itself has the usual small finite-sample square-root bias. Selection on separate
observations avoids selecting unusually quiet realizations from the same noise
used to estimate sigma. It does not prove the selected regions contain no signal.

On held-out validation observations, with `r^2=sum_c y_c^2`, check:

- relative fourth-moment departure from `E[r^4]=k*(k+2)*sigma^4` at most 0.30;
- normalized Frobenius departure of the component second moment from
  `sigma^2*I` at most 0.30;
- each selected block RMS within 30% of the pooled validation RMS;
- squared vector mean at most 0.20 of total component variance;
- normalized adjacent-observation cross-moment Frobenius norm at most 0.30
  (checked across original rows, including fold boundaries).

The estimation and validation RMS must additionally agree within 30%.
These fixed quality gates are screening rules, not a confidence certificate or
proof of pure thermal noise. This three-fold method requires at least 48
observations and 32 direct bins. The short-acquisition policy below uses a
different split. Gaussian-looking signal, correlations at untested lags, nonstationary
noise hidden under peaks and acquisition drift can escape the checks. Broad,
overlapping or truncation-contaminated signals can correctly cause rejection.
No algorithm can distinguish arbitrary Gaussian-looking signal from noise
without additional assumptions or measurement evidence.

The report counts `S` scalar values but reports only `floor(M/3)` independent
**observation clusters**. Correlated frequency bins and Cartesian components
within a row are not counted as independent acquisitions. Zero filling therefore
does not increase this effective count. This is a conservative cluster count,
not a fitted effective degrees-of-freedom estimate for arbitrary coloured noise.

Because estimation occurs after the user's actual direct plan, FFT amplitude,
windows, digital-filter shift/fold, padding and component decoding are already
reflected in sigma. Changing the plan recomputes the evidence. For white noise
with raw component standard deviation `s`, an unnormalised FFT following weights
`w_j` has `sigma=s*sqrt(sum w_j^2)`; padding adds no variance. A unitary fractional
shift followed by disjoint folding contributes the folded samples' variances.
This explains the analytic propagation tests; it is not an assumption that
all experimental digital filters generate white noise.

The audited pre-reconstruction operations are delay correction, windows,
zero filling, Fourier transformation, explicit component decoding, fixed phase
rotation, frequency-frame changes and axis reversal. Other operations return
`NusNoiseError::UnsupportedOperation`. This restriction applies only before
reconstruction; normal supported phase estimation, baseline and magnitude
processing remain available afterward. Operations are never reordered silently.

`DegenerateInput` identifies all-zero processed input without asserting a
physical noiseless experiment. `InsufficientSamples`, `Quality` (with measured
metrics) and `UnreliableEvidence` identify failed automatic analysis. A failed
estimate never falls back to explicit zero noise.

## Evidence, snapshot and replay

The enclosing `DerivationOperation::NusReconstruction` stores `noise_report`,
requested settings, direct operations, resolved operations and actual iteration
and threshold diagnostics. Its captured raw input binds the input identity,
full grid, schedule order and component interpretation. Reports do not duplicate
these facts into separately editable input/plan state. The report's source and
method version distinguish an estimate from a caller assertion.

Snapshot remains v1. Automatic reports survive offline restoration and later F1
processing. Replay receives the original matching raw Dataset and reruns the
recorded automatic request; it does not silently substitute the recorded sigma
as independently measured noise. See [pre-release changes](/contributing/#pre-release-changes) before retaining snapshots.

## Acceptance and limits

The tests in `tests/processing/nus/automatic.rs` use Box-Muller Gaussian noise,
three raw sigmas (`1e-4`, `1`, `1e4`), six seeds and 96/128/192 observations on a
256-point indirect grid. Predefined relative gates are 10% per estimate, 3% mean
bias and 4% standard deviation across seeds/rates. With 128 unwindowed direct
points, each minimum-size held-out selection has 2048 real values; the IID
Gaussian RMS asymptotic relative standard deviation is about 1.57%.
The measured mean relative bias is approximately 0.23% and dispersion 0.75%.
Repeated amplitude scales agree within `1e-12` in normalized sigma.

A separate synthetic strong/weak bin-centered pair has ratio 20:1 and raw noise
sigma 0.001. The independent finite-Fourier-sum oracle has peak amplitude
`N*F2*A`. Peak amplitude and five-bin signed area must be within 5%; nearby
line-shape flanks and the largest false peak must be below 2% of the relevant
peak/weak-peak amplitude. These assertions pass; they do not establish broad-line
quantitation. Existing explicit IST broad/off-grid and weak-column references
continue to run independently.

Tests cover window/FFT variance, padding without independent-count growth,
fractional and integer delay, a non-power-of-two grid, phase rotations, sparse
solvent-like peaks spanning a million-fold amplitude range, correlated rows,
coloured F2 noise, anisotropic components, crowded contamination, all-zero data,
nonfinite input rejection, budgets, cancellation, snapshot and replay. Persistent
strong FID tones followed by rectangular padding can contaminate every candidate
region through sinc sidelobes: the current method explicitly rejects that case.
This is a known limitation, not evidence that the sidelobes are thermal noise.

## Short acquisitions and JEOL receiver filtering

For 32–47 acquired rows and at least 128 F2 bins,
`split-holdout-component-rms.v1` selects candidate blocks on even observation
ordinals and estimates/validates on odd ordinals. Each half retains at least 16
independent observation clusters. Selection remains independent of estimation;
validation and estimation share the held-out half. There is no retry or region
reselection after inspecting held-out values. The same moment, isotropy,
stationarity, mean and correlation gates apply. The validation-to-estimation
ratio is only an independent check in the three-fold method.

The report records `floor(M/2)` effective observations and at least 512 scalar
samples. It does not treat zero-filled bins as extra independent observations.
The two-fold estimate is conditional on passing validation, so the three-fold
independence claim does not apply. Tests across 96 count/seed combinations per
component layout bound relative sigma bias below 2% and dispersion below 4%,
and test correlated, anisotropic and signal-contaminated rejection separately.

When the raw direct axis has a JEOL-resolved digital-filter delay, automatic
estimation uses the versioned `jeol-interior-split-rms.v1` or
`jeol-interior-holdout-rms.v1` policy. A fixed 1/8-grid margin at each edge of the
centered F2 spectrum is excluded from candidate selection. This prevents the
receiver stopband from being selected as a low-noise passband. The margin is
fixed before examining data; held-out quality thresholds are unchanged.
The assumption is a stationary Gaussian interior passband with attenuated
edges, not knowledge of the exact receiver response. Sigma describes that
interior and supplies conservative thresholds in the suppressed edge bins.
Unusual filters or contaminated interior regions can still fail validation.
The filter authority and noise-policy version survive snapshots and replay.

SharedComplex reconstruction uses the existing complex IST kernel with one
complex field, preserving the owner axis and conjugation flag. It does not
manufacture a second indirect lane. Conjugating this channel reflects its
Fourier spectrum; isotropic shrinkage with symmetric frequency thresholds
commutes with that reflection. The ordinary F1 FFT applies the retained sign
convention afterward. Synthetic sparse Fourier sums test both conjugation
states, retained sample bits, weak peaks and agreement with a full-grid oracle.

## Running and application integration

Run the entirely synthetic public example:

```console
cargo run --release --example nus_auto --locked
cargo test --test processing nus:: --locked
```

The same example accepts one optional raw directory argument. It never writes
the acquisition, copies experimental data into the repository or reads processed
spectra as a reconstruction shortcut. Its real-data recipe decodes components,
applies source-evidenced shift/fold delay correction and a cosine
bell, then F2 FFT, NUS and F1 FFT. Setting `NMR_NUS_ANALYZE_ONLY` runs only the
cached direct/noise stage for diagnosis.

Application integration:

1. Select `AutoNusSettings` for the automatic NUS request; retain `NusSettings`
   only for explicit advanced input. Pass the Dataset to the adapter rather than assembling IST arrays manually.
2. Pass the compiled deterministic F2 base plan, including declared component
   decoding and the chosen delay correction. Keep frequency post-processing
   after NUS/F1, in a separate F1 plan.
3. Grant the preflight work/memory bound on the background task's shared
   `ExecutionContext`. Handle `NoiseEstimation` progress and cancellation.
4. Read the report from `AnalyzedNus` for visible diagnostics before reconstruction,
   then consume that same object to reuse processed observations. On success,
   retain the resulting Dataset and its derivation; do not maintain a separate
   mutable sigma cache keyed only by filename.
5. Apply the ordinary F1 plan, then supported frequency-domain post-processing
   and the two-dimensional display. Surface noise and convergence failures.
6. Use ordinary snapshot write/read and `AcceptRecordedHistory` for offline
   continuation. Use the derivation's replay with the matching original Dataset
   when recomputation is requested.

The underlying rationale is supported by the
[SMILE manual, sections 5.9–5.10](https://spin.niddk.nih.gov/bax-apps/software/SMILE/smile_manual.pdf)
(automatic noise is feasible, but its estimator is not specified), the
[hmsIST authors' tutorial](https://api.nmrhub.org/files/84/207/weizmann17-arthanari-HMSIST-tutorial.pdf)
and [2012 paper](https://pubmed.ncbi.nlm.nih.gov/22331404/) (direct-dimension
processing before indirect reconstruction), and
[NUScon](https://pmc.ncbi.nlm.nih.gov/articles/PMC10583271/) (assess weak peaks and
false peaks, not just apparent SNR). This implementation is not SMILE and does
not transplant SMILE's sigma multiplier into the existing IST kernel.
