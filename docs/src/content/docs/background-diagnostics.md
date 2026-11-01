---
title: Two-dimensional background diagnostics
description: Experimental read-only Gaussian background measurements for scalar frequency planes.
---

`nmr::processing::background` measures background location, Gaussian amplitude
scale and sampled tails in a two-dimensional scalar frequency plane without
changing it. It requires approximately Gaussian background in sufficiently many
distributed blocks. Results are conditional diagnostics, not contour thresholds
or false discovery guarantees.

## API and input requirements

```rust
use nmr::{processing::background::{diagnose_dataset, Representation}, ExecutionContext};

fn example(spectrum: &nmr::processed::ProcessedDataset) -> Result<(), Box<dyn std::error::Error>> {
    let result = diagnose_dataset(
        spectrum, Representation::PhaseSensitive, &mut ExecutionContext::default()
    )?;
    println!("{:?}: sigma {:?}, location {:?}", result.status, result.sigma, result.location);
    Ok(())
}
```

`diagnose_dataset` requires rank two, scalar components and frequency domains.
Select a real scalar product before diagnosing component-bearing data. Imported
spectra without processing history are accepted.

`diagnose` accepts a borrowed contiguous `f64` slice, shape `[rows, columns]`
with the second axis fastest, and an optional boolean mask. `diagnose_f32`
provides the same interface for `f32`. The mask must match the storage length;
`true` includes a point. Nonfinite and masked points are excluded. Excluded
sampling positions are not replaced. Both extents must be at least 32, with at
least 1024 finite unmasked points; these minima do not guarantee a successful
estimate. No interface copies the full plane or implicitly rescales amplitudes.

The caller must establish `Representation::PhaseSensitive`; use `Unknown` when
that fact is missing. Representation is never inferred from sample signs.
`Magnitude` and `Unknown` are unsupported. `ProcessedDataset::has_magnitude_history()`
checks current and retained ancestor histories for applied modulus operations.
When found, `diagnose_dataset` changes a phase-sensitive declaration to
`Magnitude` and returns `Unsupported`. Absence of that history does not establish
phase-sensitive data.

## Reports and status

Use `status` to determine measurement availability. `reasons` provides diagnostic
explanations; callers should not use its text as a status code.

| Status | Meaning |
| --- | --- |
| `Estimated` | Distributed blocks passed the heuristic model checks. |
| `Constant` | Every point is finite, included and exactly equal, and input minima are met. Location is exact; sigma, IQR and tail are zero; span is `[location, location]`. This does not establish absence of instrumental noise. |
| `InsufficientSamples` | Either extent is below 32 or fewer than 1024 finite unmasked points remain. |
| `Unidentifiable` | Background coverage or block consistency checks failed; location, sigma, tail and span are withheld. |
| `Unsupported` | The representation does not support Gaussian amplitude calibration. |

Malformed shapes, storage lengths, masks and dataset domains/components return
errors, as do cancellation and shared work-budget failures. Unavailable
measurements use `None`. A location IQR may remain available when the final
consistency check fails.

| Field | Meaning |
| --- | --- |
| `location`, `sigma` | Median retained block location and MAD-calibrated scale. |
| `location_iqr`, `location_span` | IQR and 5th–95th percentiles of retained block locations; neither bounds the full baseline excursion. |
| `background_tail` | Median of retained blocks' 99th absolute residual percentiles. |
| `coverage` | Retained blocks divided by all 64 spatial blocks. |
| `background_samples` | Sampled points in retained blocks; for `Constant`, all plane points. |
| `valid_points` | All finite unmasked points scanned; zero for `Unsupported`, which returns before scanning. |

Locations, scales and tails share the input amplitude unit. Counts are not
independent effective sample sizes. Coverage and counts may remain available
for `Unidentifiable` reports.

`background_tail` can reveal sparse tails missed by the central MAD, but weak
peaks can also increase it. Aggregating by median requires widespread tail
evidence and can miss artifacts confined to a minority of blocks. Tail and
position summaries exclude rejected blocks. They are not confidence bounds or
calibrated extreme-tail probabilities; display clients must validate their own
noise-versus-weak-signal tradeoff before using them as a floor.

## Method and limits

The method measures amplitudes directly: adjacent differences depend on axis
correlation, while a global MAD alone supplies no spatial diagnostics.

The fixed policy is:

1. Divide the plane into 8 × 8 blocks. Sample at most 32 × 32 rectangular strata
   per block with deterministic, amplitude-independent hashed offsets.
2. Require at least 16 finite included samples per block. Compute the median,
   MAD / 0.6744897501960817 and 90th absolute residual percentile. Reject
   degenerate scales, repeated values exceeding 10% with at least three equal
   samples, and 90th-percentile/sigma ratios at least 2.5.
3. Take median block scale and location as preliminary references. Retain scales
   strictly between half and twice the reference scale, and locations less than
   two reference sigmas from the reference location.
4. Require at least 40 blocks, including six in each spatial quadrant. Report
   estimates only if scale IQR/sigma < 0.35 and location IQR/sigma < 0.75.

These checks are heuristic. Rejecting contaminated blocks can bias estimates
near acceptance boundaries. Mixed zero/noise regions can pass if enough
consistent background remains. Sparse positive or negative peaks are tolerated
only while sufficient distributed background survives; selection is symmetric
under sign reversal.

Blocks should span many correlation lengths. Long correlations, broad signals,
heavy tails, periodic sampling alias and structured background can escape the
checks or cause rejection. A single plane cannot reliably distinguish all such
signals from noise. These diagnostics do not reuse the NUS input-noise estimator,
which operates on complex reconstruction input under different assumptions.

Scanning takes O(rows × columns) work; sampled sorting is bounded. Scratch
payload has a conservative 32 KiB bound independent of plane size, observed by
the shared execution context; this excludes allocator overhead and process RSS.
Cancellation is checked every 4096 scanned points and at block boundaries.

## Reproducibility and validation

No processing history entry is added. Cache dataset reports using the source's
`canonical_digests()`, declared representation and `METHOD` (also returned as
`report.method`). For array calls, bind the sample contents through a digest or
reliable caller-managed data version, plus shape, mask identity, representation
and method. The report itself does not identify its source. Changes to the fixed
policy require a new method identity.

```console
cargo test --test background --locked -- --nocapture
cargo test --test background --release --locked -- --nocapture
```

Calibration tests use 256 × 256 unit-variance Gaussian planes, independent or
with separable AR(1) coefficients 0.65 and 0.3. Each of twelve deterministic seeds
must estimate sigma within 8%; absolute mean bias and seed standard deviation
must be below 4%. These are validation gates, not confidence intervals or
accuracy guarantees for arbitrary spectra.

Additional tests cover affine transformations, peak-width sweeps, structured
and heavy-tail contamination, masks, nonfinite values, failure states,
cancellation and f32/f64 agreement. Periodic-stripe tests measure tail response,
not successful artifact classification.

A dense processing test passes noise alone and signal plus noise through
exponential windows, zero filling, both-axis FFTs and explicit phase rotations.
The final real-component estimate is compared with the noise-only final RMS
about its mean, at original and doubled lengths. This validates that linear
chain, excluding raw vendor quadrature decoding, NUS and nonlinear processing.
