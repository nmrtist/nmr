---
title: JEOL conventions and limitations
description: JEOL time coordinates, component and phase conventions, experimental scope and validation limits.
---

This reference explains the JEOL-specific coordinate and component conventions
that affect processing results, and the limits of their validation. Check the
[format matrix](/formats/) for accepted layouts; follow
[Processing and export](/processing/) for runnable workflows.

All JEOL reads require `.allow_experimental_vendor_semantics(true)` and unified
reads retain an experimental warning. Opt-in admits the implemented
interpretation; it does not establish scientific correctness for every JEOL file.

JEOL raw readers distinguish physical storage, the valid acquisition interval,
and later processing crops. `ProcessingPlan` and `DensePipeline` consume the
reader's component semantics directly.

## Time coordinates

For a ranged **time acquisition axis**, inclusive source offsets `first..=last`
select samples from the stored sections. Header `axis_start` and `axis_stop`
describe the endpoints of that valid interval:

```text
valid_points = last - first + 1
t0 = axis_start
dt = (axis_stop - axis_start) / (valid_points - 1)
```

Unit prefixes apply to both coordinates. Storage padding is discarded and does
not contribute to `dt`. A later `RetainRange { start, end }` advances `t0` by
`start * dt` and retains `dt`. End-only zero filling retains both. Single-point
intervals retain their explicit source coordinate without inventing a dwell.
Binary/textual lists and NUS original-grid coordinates retain their separate
meanings. Parameter and imported frequency axes retain their coordinate rules.

The FFT consistency check is `abs(SW * dt - 1) <= 1e-9`;
conflicting calibration fails preflight.

## Components and signs

Disk axis codes are in `[x,y]` order; public axes are `[F1,F2]`.

| Layout | Canonical raw representation | Processing interpretation |
| --- | --- | --- |
| `[3]` or `[3,1]` | `z = S0 - i*S1` | Direct complex samples; `[3,1]` has a scalar parameter axis |
| `[3,3]`, no P/N declaration | F1 lanes: `S0 - i*S1`, `-S2 + i*S3` | Independent Cartesian components on both signal axes |
| `[3,3]`, time-domain `pn_type=y` | `C=(A-B)/2`, `S=(A+B)/(2i)`, where `A=S0-i*S1`, `B=-S2+i*S3` | Declared Y P/N acquisition decoded to Cartesian before processing |
| `[4,4]` | One shared pair: `z = S0 - i*S1` | F1 shares F2's components with the opposite imaginary orientation |

The initial `[3,3]` second section pair is negated as a whole. This agrees with
[nmrglue 0.12 `reorganize_2d`](https://github.com/jjhelmus/nmrglue/blob/v0.12/nmrglue/fileio/jeol.py#L149).
The source also establishes the two stored sections for `[4,4]`; its array
alone does not establish F1's physical direction. That direction is additionally
checked against signed analytic tones. Limited private-file audits are summarized
under [independent validation](#independent-validation).

A time-domain `[3,3]` file with typed `pn_type=y` declares Y P/N acquisition.
Four sections alone do not establish Cartesian acquisition: the coherence paths
are `P=C+i*S` and `N=C-i*S`, while initial section mapping yields `A=P`, `B=-N`.
Interpreting A/B as C/S creates F1 images. The reader applies the complex-linear
inverse in the table and records `jeol.pn-y-2d.v1`; no extra component operation
is needed before NUS. Unresolved P/N declarations are rejected; experiment names
are not used to guess quadrature. Re-read and reprocess the raw source to apply
current normalization rather than reusing old processed values.

For `[4,4]`, raw `IndirectComponents::SharedComplex { conjugated: true, .. }`
has one lane. After processing initialization, F1 has
`ComponentBasis::SharedComplex { axis: AxisIndex::new(1), conjugated: true }`
and F2 owns the Cartesian pair. Component counts remain `[1,2]` through both
FFTs. A component count of one therefore does not by itself mean scalar data.
Real and imaginary output remain accessible at component coordinates `[0,0]`
and `[0,1]`. Magnitude counts the shared pair once.

With negative Fourier requests on both axes, the stored complex result is:

```text
S(f1,f2) = sum z(t1,t2) * exp(+i*2*pi*f1*t1) * exp(-i*2*pi*f2*t2)
```

F1 phase rotation also uses F1's imaginary orientation. Windowing, filling,
cropping, phase correction, Hz/ppm conversion, reversal and final projection
retain the pairing. `SpectrumOperation::Reference` shifts a ppm axis and its
effective reference without changing any samples. `Slice` preserves the full
complex pair as described below. Other reductions and filters, including
per-axis magnitude, still require projection first when shared components are
present.

## Representative traces and automatic phase

After both FFTs, use `SpectrumOperation::Slice { index, component: 0 }` to fix
the **removed** axis. Both directions produce an independent Cartesian 1D
spectrum with two components; no real projection is needed:

| Removed/fixed axis | Remaining trace | Complex samples |
| --- | --- | --- |
| Axis 0 (F1) | F2, now axis 0 | Stored real and imaginary fields |
| Axis 1 (F2) | F1, now axis 0 | Stored real field and negated imaginary field |

The F1 conjugation expresses the pair in F1's own imaginary orientation. The
physical coordinates, direction, axis role, label, nucleus, sweep and frequency
evidence remain those of the surviving axis. Its effective reference and filter
state survive too. The recorded slice retains the source descriptor and its
shared orientation; snapshots and raw replay reproduce the same trace.
`component: 1` is rejected for this layout: there is only one shared complex
channel, and choosing a single field would discard the pair.

```rust
use nmr::processing::{
    PhaseMethod, ProcessingOperation as Op, ProcessingOptions, ProcessingPlan,
    SpectrumOperation,
};

// `spectrum_2d` has completed both FFTs, without scalar projection.
// `phase_axis` is 1 for F2 or 0 for F1; `fixed_index` is on the other axis.
let trace = ProcessingPlan::new(vec![Op::Spectrum {
    axis: 1 - phase_axis,
    operation: SpectrumOperation::Slice { index: fixed_index, component: 0 },
}])?.apply(&spectrum_2d)?;
let estimate = PhaseMethod::RobustConsensus
    .prepare(&trace, 0, ProcessingOptions::new())?
    .estimate()?;
let corrected_2d = ProcessingPlan::new(vec![Op::PhaseCorrection {
    axis: phase_axis,
    correction: estimate.correction(),
}])?.apply(&spectrum_2d)?;
```

Any selected `PhaseMethod` can use this path. Apply its correction to the
original F1 or F2 axis with **no additional sign change**. `estimate.apply` is
bound to the analyzed 1D trace and records its method diagnostics there; the
explicit 2D correction records the transferred phase parameters. Automatic
phase quality still depends on the chosen method and representative signal.

For calibration, `SpectrumOperation::Reference { delta_ppm: 0.1 }` is accepted
on either ppm axis before or after slicing. It shifts that axis's coordinates
and carrier reference by 0.1 ppm, preserving bandwidth, samples and the other
axis. The prior reference evidence remains available in history. Hz axes and
non-finite shifts remain rejected.

## Processing conventions

The [dense 2D example](/processing/#process-dense-2d-data-in-ppm) demonstrates
`DensePipeline` with source-evidenced delay correction and ppm calibration.
Explicit phase settings can be supplied through `DenseAxisConfig`.
`Projection::Real` with `ExpectedPolarity::Signed` selects the neutral real
channel when that is the desired display.

For an unweighted complex result, construct a `ProcessingPlan` with
`FourierTransform { axis: 1, transform: FourierTransform::default() }` followed
by the same request on axis 0, then call `preflight(...).execute()`.
Use negative requests for both JEOL axes; the reader supplies F1's orientation.
Project after both transforms when requesting scalar plot data.

Shared component rules are `jeol.shared-complex-2d.v1` and
`jeol.shared-complex.v1`. See [pre-release changes](/contributing/#pre-release-changes)
when retaining snapshots or updating an application.

## Experimental scope

All JEOL unified imports retain `ExperimentalVendorSemantics`, including raw NUS
with a validated vendor or externally declared schedule. The warning's `details`
identify the disk-axis layout, the limited audit coverage, and applicable NUS,
processed-domain, embedded parameter-axis or digital-filter concerns. The opt-in
admits experimental reads; it does not suppress warnings. Bruker NUS warning
policy is unchanged. Concrete missing-metadata warnings remain independent.

| Layout / semantics | Available independent evidence | Remaining warning boundary |
| --- | --- | --- |
| Raw `[3]` | Selected proton components and NumPy FFT/pipeline audit | Other precision, storage, coordinate and calibration combinations |
| Raw `[4,4]` | Selected COSY components, shared orientation, FFT and traces | Other encodings and calibration combinations |
| Raw `[3,3]` | Selected HSQC component decode and declared P/N inversion | General acquisition semantics and NUS reconstruction |
| Raw `[1]`, `[3,1]` | Synthetic regressions | Independent layout and semantic qualification |
| Processed JDF | Synthetic regressions | Raw FFT audits do not qualify vendor frequency-domain files |
| NUS, embedded axes, digital filter | Checked declarations / selected-file numerical checks | Not general vendor semantic qualification |

The evidence status in `src/formats/jeol/semantics.rs` remains `Experimental` for
all admitted layouts. None of these audits establishes an entire layout family.
Any proposal to broaden support needs independently reproduced evidence with its
exact encoding/domain/acquisition scope and boundary regressions. Warning details
survive snapshots and execution reports that retain input metadata.

## Independent validation

`tests/reading/jeol/time.rs` covers valid point counts, poisoned storage
padding, lazy/materialized equality, valid endpoints, nonzero time origin,
later cropping/filling and genuine calibration conflicts.
`tests/processing/jeol.rs` compares every complex sample against physical direct
sums with odd/even lengths, both signs, windows, cropping, filling, phase,
projection, replay and snapshots. It separately bounds mirror leakage for
signed tones and verifies all four `[3,3]` Cartesian fields. These ordinary
regressions use redistributable synthetic signals.

`tests/processing/jeol/slices.rs` uses independent finite inverse sums to check
both slice directions, phase transfer, reference shifts, component validation,
resource limits, cancellation, replay and snapshots. P/N regressions in
`tests/processing/jeol/pn.rs` cover signed coherence paths and reconstruction.

Earlier local audits compared selected proton/COSY files with nmrglue and NumPy,
and selected HSQC components with an independent section decode and P/N inverse.
The raw files and audit tools are not distributed, so these are limited historical
evidence, not reproducible acceptance commands or qualification of other layouts.
Use the synthetic tests for current regression coverage and the
[format-rule source register](/scientific-contract/#format-rule-sources) for
independent sources and remaining evidence gaps. No equality to Delta's automatic
phase or NUS reconstruction defaults is claimed.

```console
cargo test --test reading --locked jeol::
cargo test --test processing --locked jeol
```
