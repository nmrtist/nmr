---
title: Scientific contracts and evidence
description: Numerical conventions, supported combinations, source rules and independent checks.
---

This page defines the numerical meaning of the supported 0.1.1 processing
operations. [Format support](/formats/) lists accepted layouts. Acceptance,
independent source evidence and agreement with a vendor application are separate
claims; the [source register](#format-rule-sources) below states their coverage.

## Fourier, window and phase conventions

Public complex values are `R + iI`. A time axis must declare seconds and a
positive uniform interval `dt` for Fourier transformation. If spectral-width
evidence `sw` is present, it must satisfy `abs(sw * dt - 1) <= 1e-9`;
otherwise the transform uses `sw = 1 / dt`. Explicit coordinate lists are
rejected even when their values happen to be equally spaced.

For the current transform length `N`, time samples at `t0 + n * dt`, centered
output index `k`, and exponent sign `sigma` (`-1` by default, or explicitly `+1`):

```text
q = k - floor(N / 2)
f[k] = q * sw / N                           [Hz]
F[k] = exp(i * sigma * 2*pi * f[k] * t0)
       * sum(n=0..N-1) x[n] * exp(i * sigma * 2*pi * q*n/N)
```

For `sw = 1/dt`, this equals the direct sum using physical times `t0 + n*dt`.
When source `sw` differs within the tolerance above, source `sw` sets the
frequency coordinates and time-origin phase. There is no `1/N` or `dt` amplitude normalization. The frequency axis ascends
for either exponent sign; changing the sign changes which signal frequency
appears at a given bin. Even lengths place `-sw/2` at the first bin and omit
`+sw/2`; odd lengths are symmetric about zero. A bin-centered tone of amplitude
`A` with matching negative-sign convention has amplitude `N*A`. Nonzero `t0`
is included in the phase, rather than resetting the retained first sample to
physical time zero. The pinned [RustFFT 6.4.1 transform interface](https://docs.rs/rustfft/6.4.1/rustfft/trait.Fft.html)
supplies an unnormalized FFT; centering and time-origin handling are this
library's additional contract.

End-only zero filling appends zero samples and preserves `t0` and `dt`.
`N` in the formulas is the length after filling; the number of measured points
continues to determine the acquired information and amplitude of an unweighted
on-bin signal. Filling samples the same finite Fourier sum more densely and
does not increase acquisition duration or physical resolving power.

The nonzero exponential window requires the same uniform time calibration:

```text
w[n] = exp(-pi * lb_hz * n / sw)
```

For `sw = 1/dt` this is `exp(-pi * lb_hz * n * dt)`; the effective `sw` selection
and consistency tolerance are the same as for FFT.
Its elapsed time starts at the first retained sample, including after a crop;
`t0` is not included in this window. Finite signed line broadening is accepted,
subject to finite numerical output. `lb_hz = 0` is an identity and needs no
time-coordinate calibration. The [NMRPipe EM reference](https://www.nmrscience.com/ref/nmrpipe/em.html)
documents the usual sweep-width formula. That source does not authorize
nonuniform-time processing in this library.

The powered sine bell is `sin(pi * (offset + (end-offset)*n/(N-1)))^power`,
using a zero fraction for `N=1`. Its explicit `first_point_scale` multiplies only
`n=0`. Windows apply the same weight to every component at that logical point.
FFT and exponential windowing add no implicit first-point factor of `0.5`.
Choose first-point weighting explicitly when the acquisition and comparison
require it. The window uses the axis length when the window step executes, so
moving it across zero filling can change the result.

`PhaseCorrection` rotates each complex component pair by:

```text
exp(i * pi/180 * (p0 + p1 * (k/N - pivot)))
```

Angles are in degrees; `k` is the current array index, `N` is its current
length, and `pivot` is a fraction of the full width. The denominator is `N`,
not `N-1`. To apply the same physical phase after reversing an axis, with the
same pivot, use `p1' = -p1` and
`p0' = p0 + p1 * ((N-1)/N - 2*pivot)`.
An executed rotation records what was applied; it does not certify a correctly
phased absorption spectrum.

## Supported operation combinations

| Request | Accepted meaning | Current boundary |
| --- | --- | --- |
| `Projection::Real` | Select component zero on every signal axis, preserving signed sample values | All signal axes must be Cartesian or shared-complex frequency axes; use `PolarityState::Ambiguous180`; no phase history or absorption-quality claim |
| `RealSigned`, `RealAbsorptive`, `UnphasedReal` | Policy projections retaining their phase/polarity preconditions | Recorded phase or failure is policy state, not proof of spectral correctness |
| `Magnitude` | Scaled L2 norm over all signal components at each logical point | Does not preserve signed absorption intensity or select one axis |
| `Window::exponential(0)`, original-length zero fill, Hz-to-Hz frame | Legitimate identities; accepted requests remain in history | Mathematical identities do not acknowledge unknown digital-filter delay |
| `ResolveFrequencyFrame` | Calibrated Hz-to-ppm coordinates, `ppm = carrier_ppm + f_hz/reference_mhz`; Hz-to-Hz identity | No ppm-to-Hz conversion, ppm rereferencing or sample resampling |
| `BaselineCorrection` with `BaselineProfile::PositivePeaksV1` | Correct each trace along the selected scalar ascending Hz axis, retaining other Cartesian components | At least three selected-axis points; descending and ppm axes are rejected; quality remains experimental |
| `ReverseAxis` | Reverse samples and known coordinates together | Reject reversal that would invalidate pending encoded modulation; multipoint reversal also invalidates FFT-bin mapping needed by frequency-domain delay correction |
| `ComponentTransform` | Decode retained lanes using the declared absolute grid axis or acquisition observation ordinal | A referenced axis must exist; its own absolute origin determines modulation; local index, absolute grid coordinate and observation ordinal are distinct |
| `SpectrumOperation::Slice` | Fix and remove one axis of rank-2 data, retaining the surviving axis's evidence and history | Shared complex data require component zero and produce the full Cartesian pair in the surviving axis's imaginary orientation; see the [JEOL trace path](/jeol-conventions/#representative-traces-and-automatic-phase) |
| `SpectrumOperation::Reference` | Shift ppm coordinates and the effective carrier reference, preserving all samples and component bases | Finite ppm shifts only; valid on either shared-complex axis; prior evidence stays in history |

`Projection` has dataset scope because it reduces every signal component axis;
it is not a per-axis tensor selector. Global projection requires every signal
axis to be in the frequency domain.
For per-axis magnitude or explicit slice/reduction component selection, use
[SpectrumOperation](/spectrum-operations/#spectrum-operations) within its component
and coordinate restrictions. Mixed time/frequency data can be an intermediate
result between axis transforms; it is not ready for global projection.
Imported ppm spectra are usable
as processed data, but their import does not establish the library FFT state
or make every raw-oriented processing step applicable.

`ProcessingOperation` contains executable requests. `ProcessingRequest` also
describes automatic phase requests recorded by `NormalizedAcmeV1::apply`.
Do not submit a history request to a plan. `OperationTarget` reports actual
axis, axes or dataset scope; it does not invent an axis for global operations.

`ProcessingPlan` and `DensePipeline` reject sparse inputs. The separate
`PhaseCovariantGroupRetainedIstV1::reconstruct` kernel accepts `IstInput` with
exactly 128 distinct measured indices on a 512-point indirect grid, one or two
Cartesian fields and direct-frequency samples in `[M,F2,C,2]` order. It returns
`IstOutput` in `[512,F2,C,2]` order. V1 has no automatic sparse dataset adapter,
duplicate-observation reduction, indirect FFT, `ProcessedDataset` construction
or processing-history integration in this call. The caller must establish the
input component interpretation and retain any external processing provenance.
This fixed profile is not a general NUS reconstruction pipeline. The separate
`NusSettings` and `AutoNusSettings` dataset APIs provide general-grid acquired-row
F2 processing, reconstruction and provenance; see [automatic NUS](/nus-automatic/).

## Format-rule sources

A rule or derivation identifier records which interpretation the reader selected;
it is not a vendor certificate or an evidence-strength
rating. Synthetic bytes test the implementation of the selected interpretation.
Fixture labels such as `PrimarySource` or `ExpertConfirmed` do not independently
establish its scientific correctness.

| Rule / derivation / decoding record | Traceable external source and location | Evidence scope and remaining boundary |
| --- | --- | --- |
| `varian.direct.v1`, `varian.direct-array.v1` | OpenVnmrJ commit `5e20f6f77dd5331473e47402153cb9dd310eb3d4`, [`data.h` headers and status bits](https://github.com/OpenVnmrJ/OpenVnmrJ/blob/5e20f6f77dd5331473e47402153cb9dd310eb3d4/src/vnmr/data.h#L63), [`ft2d.c` direct Fourier call](https://github.com/OpenVnmrJ/OpenVnmrJ/blob/5e20f6f77dd5331473e47402153cb9dd310eb3d4/src/vnmr/ft2d.c#L545), [`init2d.c` frequency positioning](https://github.com/OpenVnmrJ/OpenVnmrJ/blob/5e20f6f77dd5331473e47402153cb9dd310eb3d4/src/vnmr/init2d.c#L1252) | Fixed reference implementation for storage and coordinate/sign derivation. Source facts do not authorize grouped arrays or arbitrary pulse-sequence inference. |
| `varian.2d-phase-explicit-f1coef.v1`, `varian.2d-phase-default-ptype.v1`; corresponding `openvnmrj.5e20f6f.*` derivations | Same commit, [`ft2d.c` combination](https://github.com/OpenVnmrJ/OpenVnmrJ/blob/5e20f6f77dd5331473e47402153cb9dd310eb3d4/src/vnmr/ft2d.c#L577), [`sky.c::combine`](https://github.com/OpenVnmrJ/OpenVnmrJ/blob/5e20f6f77dd5331473e47402153cb9dd310eb3d4/src/vnmr/sky.c#L682) | `combine` uses `(a-i*b)` on stored-sign lanes; conjugating the direct lanes gives canonical `(a+i*b)` in both rows. Explicit coefficients do not establish the separate OpenVnmrJ ntype processing switch. Both coefficient rows preserve the source signs; the default matrix is identity. Analytic reader-to-FFT regressions cover default, TOCSY and echo/antiecho acquisitions with both exponent signs. |
| `openvnmrj.5e20f6f.chemical-shift-reference.v1` | Same commit, [`init_display.c::get_reference/get_rflrfp`](https://github.com/OpenVnmrJ/OpenVnmrJ/blob/5e20f6f77dd5331473e47402153cb9dd310eb3d4/src/vnmr/init_display.c#L906) | `rfl-rfp`, independent zero defaults when absent, combined with the centered-Hz definition. Invalid present values remain errors. |
| `bruker.direct.v1`, `bruker.layout.v1` | [nmrglue v0.11 original reader](https://github.com/jjhelmus/nmrglue/blob/v0.11/nmrglue/fileio/bruker.py#L908), `guess_shape`, `read_binary`, `complexify_data`; the NBL comment identifies the acquisition-manual storage rule | Pinned independent implementation for binary scalar encoding, complex pairing and 1024-byte boundaries. It does not independently verify every NUS/preallocation case or certify physical phase. |
| `bruker.fnmode-qf.v1`, `bruker.fnmode-states.v1`, `bruker.fnmode-states-tppi.v1`, `bruker.fnmode-echo-anti-echo.v1`; `bruker.component-transform.v1` | Bruker [Processing Commands and Parameters, revision 007](https://nmr.chem.ucsb.edu/docs/Bruker_NMR_Manuals/processing-reference_v007.pdf), “2D Processing Commands”, `xfb`, “FnMODE” storage/processing diagrams | Vendor documentation supports named modes and hypercomplex plane interpretation. Exact library sign/parity combinations also have analytic regressions; that does not establish equality to all TopSpin processing defaults or all 3D/NUS acquisition variants. |
| `bruker.processed-1d.v1`, `bruker.processed-2d.v1` | Same Bruker revision 007, `xfb` F1 output diagrams; [nmrglue v0.11 `scale_pdata`](https://github.com/jjhelmus/nmrglue/blob/v0.11/nmrglue/fileio/bruker.py#L1213) | Plane names map to `[F1,F2]` components; intensities multiply by `2^NC_proc`. The support matrix includes contiguous and XDIM tiled 2D layouts with cropped edge padding and rectangular component sets. |
| `jeol.direct.v1`, `jeol.cartesian-2d.v1`, `jeol.cartesian-sections.v1`, `jeol.shared-complex-2d.v1`, `jeol.shared-complex.v1` | [nmrglue 0.12 section mapping](https://github.com/jjhelmus/nmrglue/blob/v0.12/nmrglue/fileio/jeol.py#L122), signed analytic signals and the [limited acquisition audits](/jeol-conventions/#independent-validation) | Section signs, valid-time calibration, shared pairing and real proton/COSY public processing agree with independent references. This does not establish every list/filter interpretation or Delta processing default. |
| `jeol.processed-1d.v1`, `jeol.processed-2d.v1` | Same source section mapping and synthetic processed fixtures | Imported processed calibration and component mapping retain their stated boundaries; no blanket vendor certification is claimed. |
| `jcamp.xydata-asdf.v1` | Davies and Lampen, [JCAMP-DX for NMR, 1993](https://iupac.org/wp-content/uploads/2021/08/JCAMP-DX_NMR_1993.pdf), NMR label/data-table definitions; [IUPAC protocol index](https://old.iupac.org/jcamp/protocols.html) | Independent standard for the NMR records. Synthetic ASDF/checkpoint tests cover the implemented subset; this is not complete protocol conformance, and NTUPLES/LINK/FID are rejected. |

`canonical.*` derivations describe this library's normalization rules, not
external authorities. In particular, `canonical.group-delay.v1` records the
evidence selected for a delay; it does not prove an exact vendor filter inverse.
The Bruker fallback delay whitelist and JEOL stage-derived delay retain limited
independent evidence. Their correction profiles are explicit library algorithms.
Missing parameter evidence remains unknown rather than being silently set to zero.

`bruker.dspfvs-decim-table.v1` defines the DSPFVS/DECIM lookup for versions
10/11/12/13 when GRPDLY is absent or -1. Explicit non-negative GRPDLY takes
priority under `bruker.direct.v1`. Regression tests require DSPFVS=12 and DECIM=16
to resolve to 71.625 complex points within 1e-9. This validates parameter
resolution and numerical correction behavior; it does not establish an exact
vendor filter inverse from real acquisition evidence. Source parameters and the
selected rule are preserved through snapshot v1 and processing evidence.

## Independent acceptance and reproducibility

The release gates distinguish successful execution from recovery of a physical
signal. Automatic methods remain experimental and have deliberately narrow
acceptance domains:

- **ACME:** `NormalizedAcmeV1` retains the bounded entropy search, but an entropy
  minimum alone is insufficient for success. A phase-invariant single-Lorentzian
  magnitude model is used only as a quality check; it does not fit a replacement
  phase correction. The accepted complex shape is proportional to
  `(1 - i*u)/(1 + u*u)`, with established positive polarity. Its half-width must
  span at least two samples, be at most 5% of the trace length, and fit at least
  four half-widths from either edge. The relative complex line-shape residual
  must be at most `1e-6`. Already phased lines return an exact identity correction.
  The audited `p0=-37°, p1=60°` and `p1=240°` examples are explicitly rejected:
  the existing entropy search still does **not** recover their physical phase.
  This guard substantially narrows successful automatic operation. Noise,
  baseline, multiple peaks, broad lines and unknown polarity have no general
  recovery guarantee. `QualityUnverified` is a hard quality failure even when
  `ContinueUnphasedReal` was selected; use an explicit phase/projection operation
  when deliberately processing data outside this domain.
- **AsLS:** `PositivePeaksV1` uses a continuous smoothing scale on the complete
  normalized spectral window. With `u=(x-x_min)/(x_max-x_min)`, the target is the
  trapezoidal approximation to
  `integral w(u)*(y-b)^2 du + (1e6/2048^4)*integral (b''(u))^2 du`.
  On a uniform grid this yields an effective index-difference penalty
  `1e6*((n-1)/2048)^4`. Nonuniform second derivatives retain the corresponding
  quadrature weights. Resampling a fixed window preserves its smoothing scale;
  cropping changes it. The fixed asymmetry remains `0.001`, with at most 50 solves
  and weighted relative convergence tolerance `1e-6`.
  Quantitative acceptance covers 129, 513, 2049 and 8193 samples over 0–1200 Hz,
  a Gaussian `20*exp(-((x-530)/22)^2)` (width parameter 1.83% of the window), and
  baseline `2+0.003*x+2*sin(pi*x/1200)`. Sampled peak-height error is below 2%,
  area error within four widths of the peak is below 5%, and relative L2 error
  is below 5%. A 44 Hz width (3.67% of the window) retains about 91–92% of its
  area and is **outside the quantitative guarantee**. Narrow and broad examples
  agree across sampling densities to within 0.03 amplitude units on shared
  coordinates. General baselines, negative peaks and broad-peak quantitation
  require separate validation; convergence does not establish peak preservation.
- **IST:** `IstOptions::new()` supplies resource limits without a noise assumption.
  Reconstruction requires `noise_standard_deviation(sigma)` from independent
  noise-only evidence in the same amplitude units, or an explicit `noiseless()`
  assertion. The [automatic NUS adapter](/nus-automatic/) can instead supply
  explicitly labelled, validated estimates from acquired observations.
  Signal amplitudes and appended zero columns never estimate noise.
  The noise floor uses a per-column 512-frequency threshold, so F2 cropping does
  not change the supplied noise threshold. Initial signal amplitude, numerical
  threshold floor and convergence are also computed independently per direct
  column, while Cartesian components remain grouped for phase covariance.
  Each column is frozen when it reaches its own stopping condition: energy in
  a stronger neighboring column cannot conceal a weak column's missing data.
  Output includes the supplied sigma, per-column final thresholds, relative
  iterate changes and iteration counts, and measured-data residual. Scalar
  `final_threshold()` and `relative_change()` summaries report column maxima.
  Measured residual is zero because samples are retained bit for bit; this is
  not an error bound on missing data. Single-tone tests span one/eight columns,
  sparse/full column occupancy and two fixed schedules, requiring absolute peak
  amplitude and time-domain L2 errors below `1e-5`. An independently specified
  noisy-tone case bounds L2 error below `0.002`. The fixed 512/128 reconstruction
  also retains an identical weak-column result when independent peaks 1000 or
  1,000,000 times stronger are appended. This is checked with sigma 0 and
  `1e-4`; absolute weak-peak/line-shape errors are below `1e-5` and `2e-4`
  respectively. V1 rejects nonzero columns without recoverable signal. The
  general-grid dataset path instead applies the same shrink/replace map from
  the noise floor, allowing signal, noise and zero columns to coexist without
  discarding measured samples. All-zero columns remain zero without iterations. The method
  remains experimental for other noise, decay, dynamic-range and schedule regimes.
  Preflight excludes borrowed input and includes newly allocated output, FFT
  planning/retained buffers and scratch; it calculates bounds before making plans.

| Contract | Executable evidence | What it establishes |
| --- | --- | --- |
| 1D physical Fourier chain | `tests/processing/fourier_accuracy.rs`, `one_dimensional_physical_dft_survives_window_fill_phase_ppm_and_reverse` | Direct sum over physical times; window, nonzero origin, zero-filled length, phase, reversed ppm coordinates and every complex output sample |
| 2D physical signal | Same file, `two_dimensional_tones_retain_physical_frequencies_phase_and_component_energy` | Positive and negative Hz positions, both nonzero time origins, four Cartesian amplitudes/phases and energy outside the target peak |
| FFT layout and component decoding | `tests/processing/fft_window.rs`, `tests/processing/components.rs` | Both exponent signs, odd/even centering and Nyquist, ptype/ntype and echo/antiecho component relations, parity/crop regressions |
| Unsupported combinations | `tests/processing/contracts.rs` and processing tests | Structural errors fail preflight; component, origin and coordinate combinations return correct results or explicit rejection |
| Identity and replay | Processing and history tests | Request history, source binding, continuation, and execution of recorded parameters |
| Automated scientific quality | `tests/processing/phase/accuracy.rs`, `tests/processing/baseline/accuracy.rs`, `tests/processing/nus/accuracy.rs` | Physical absorption preservation or explicit phase refusal; AsLS height/area/line shape and sampling density; absolute IST amplitude and zero-column/noise independence |

The analytic tests do not call RustFFT or this library's numerical kernels to
produce expected results. They bound floating-point error and leakage, rather
than approving a plot or using repeated execution as an independent oracle.
The tensor baseline regression can compare independent traces with the same
one-dimensional kernel: it verifies component indexing, not AsLS recovery quality.
No real acquisition or external application is required by these tests.

Reproducibility has four distinct levels: source-byte identity, canonical
sample/descriptor identity, recorded resolved operations, and numerical output
comparison. Strict replay checks the input identities and runs supported
recorded operations. Ordinary phase replay applies recorded corrections;
[automatic NUS replay](/nus-automatic/#evidence-snapshot-and-replay) reruns the
recorded automatic request. Replay success alone
does not compare outputs. Compare samples or physical metrics with suitable
tolerances when testing another environment or numerical backend.

Histories record actual output digests and available execution-environment facts;
unavailable facts remain unknown. This unpublished project updates current rules
in place and does not provide compatibility for development snapshots. `linear-component-transform.v1` binds the
origin of the modulation's referenced axis, `asls.v1` covers every independent
tensor component trace, and `projection.v1` records dataset scope and all affected
signal axes. `asls.v1` additionally fixes the sampling-density dependence of
the `PositivePeaksV1` discretization. `normalized-acme.lorentzian-guard.v1`
records the quality-guarded automatic phase policy; explicit phase corrections
retain `phase-correction.v1`.
Runtime Rust histories are not a portable
serialized archive, and NPZ is plot exchange. JSON execution reports are not
replay inputs; [snapshots](/host-integration/) have a separate storage contract. ACME, AsLS and the fixed IST profile retain
experimental scientific-quality limits; the tests above do not claim general recovery quality or bitwise results
across platforms.

## Generic infrastructure methods

[Spectrum operations and estimation](/spectrum-operations/) specifies Gaussian,
five named phase methods, real baseline methods, spectrum operators, binary
interpolation and general-grid NUS contracts. Each uses explicit method versions
and the common context/history/snapshot machinery. The original fixed profiles
above retain their meanings. `tests/processing/phase/quality.rs` and
`tests/processing/baseline/quality.rs` retain the black-box analytic signals and
thresholds; the phase, baseline and NUS `accuracy.rs` modules in `tests/processing/`
add independent general-grid IST amplitude, area and missing-point gates.
Real JEOL proton/COSY processing and `[3,3]` decoding have the bounded
[independent validation](/jeol-conventions/#independent-validation) described above. Other JEOL and
Varian acquisition variants retain incomplete independent evidence.
