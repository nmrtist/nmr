---
title: Processing and export
description: Choose a raw-data workflow, inspect its plan and export a scalar spectrum.
---

Start with the [quickstart](/getting-started/) if you have not built the crate.
The commands below run from the repository root. Supply your own acquisition
with its companion files and a new output filename in an existing directory.
Check [format support](/formats/) first; the isolated binary fixtures in
`tests/fixtures` are not all standalone experiment directories.

## Choose a workflow

| Input and task | Entry point |
| --- | --- |
| Already processed scalar spectrum | `read_spectrum` example; no raw processing needed |
| Dense 1D raw data, explicit demonstration recipe | `process_1d` example using `ProcessingPlan` |
| Dense 2D raw data with delay and ppm reference evidence | `process_2d` example using `DensePipeline` |
| Sparse 2D NUS | Separate `NusSettings` / `AutoNusSettings` adapters; [NUS guide](/nus-automatic/) |
| Slice, reference, combine, estimate phase or baseline | [Spectrum operations and estimation](/spectrum-operations/) |

Explicit dense plans support ranks 1 through 3 and reject sparse data and higher ranks before
numerical execution. `DensePipeline` is a fixed recipe with experimental automatic
policy and remains limited to ranks 1 and 2. Explicit operations do not establish scientific correctness of your chosen
parameters; automatic estimators have [limited validation domains](/scientific-contract/).

For dense Bruker 3D, explicit plans operate in F1, F2, F3 order. Resolve
each encoded indirect axis while the direct axis is Cartesian, then Fourier
transform the signal axes. Choose digital-filter correction, phase and
frequency-reference operations using evidence appropriate to the acquisition.
Use the neutral real projection to retain signed values without asserting
absorptive phase or positive polarity. The same region API can read a slice
by requesting one point on the fixed axis.

For a smoke check on the ubiquitin acquisitions (14 and 16 are NOESY-HSQC;
2 is HNCACB), run this from the nmr checkout with the sample directory beside it:

```console
cargo run --locked --example process_3d -- ../ubiquitin/14 ../ubiquitin/16 ../ubiquitin/2
```

This recipe requires two encoded indirect axes and acquisition evidence for
direct digital-filter delay and ppm references. It reports the shape, nuclei
and ppm endpoints after a signed real projection on the acquired grid. It
applies no window, zero filling or absorptive phasing.

To compare all three samples against an independent calculation, run NMRPipe's
direct-axis FFT on each stored row. These settings were exercised with
NMRPipe 13 on little-endian Linux; the largest temporary pair uses about 128 MiB.

```sh
scratch=$(mktemp -d)
for sample in 2 14 16; do
  rows=5120
  if [ "$sample" = 16 ]; then rows=8192; fi
  bin2pipe -in "../ubiquitin/$sample/ser" -out "$scratch/$sample.fid" \
    -aswap -ri -i2r -xN 2048 -xT 1024 -xMODE Complex \
    -ndim 2 -yN "$rows" -yT "$rows" -yMODE Real \
    -xSW 7211.53846153846 -xOBS 600.23 -xCAR 4.660246905353 -xLAB F3 &&
  nmrPipe -in "$scratch/$sample.fid" -fn FT -nodf -out "$scratch/$sample.ft" &&
  cargo run --locked --example process_3d -- --nmrpipe-rows "$scratch/$sample.ft" "../ubiquitin/$sample" ||
    break
  rm "$scratch/$sample.fid" "$scratch/$sample.ft"
done
```

The example aligns Fourier exponent conventions, applies the declared direct
delay as a phase ramp, and uses an explicit DFT with States-TPPI parity or
Echo/AntiEcho lane conversion as specified by the acquisition. It checks signed
amplitudes over eight full F1/F2 planes per sample and the location and sign of
their strongest points. The largest absolute difference in each plane must
be at most 1e-5 times the reference plane maximum. This provisional
diagnostic threshold accounts for NMRPipe's float32 FFT and amplification of
relative error in planes with cancellation. It is not a release accuracy bound.
This checks raw row layout, FFT signs and signed extraction against an
independent calculation. The comparison is on integer
bins. It does not benchmark NMRPipe's native group-delay correction, optimize
phase, independently calibrate chemical shifts, or assign peaks. The example
prints ppm endpoints computed from acquisition evidence; an independently
referenced spectrum or identifiable resonance is needed to validate the physical
ppm calibration. The NMRPipe header is only an input to this diagnostic.

The TopSpin 2D raw-plane exports provide a separate axis metadata check.
Compare the acquisition center `O1 / BF1` to the processed center
`OFFSET - SW_p / (2 * SF)`, and compare `SW_h / BF1` to `SW_p / SF` for
sweep width. Use these status files under each sample's `pdata` directory:

| Sample | F1 | F2 | F3 | Largest center difference |
| --- | --- | --- | --- | --- |
| 2 | `5/proc2s` | `2/proc2s` | `2/procs` | 0.000031 ppm |
| 14 | `2/proc2s` | `3/proc2s` | `2/procs` | 0.004954 ppm |
| 16 | `3/proc2s` | No comparable plane | `3/procs` | 0.000003 ppm on the two H axes |

All eight comparable widths agree within 0.000001 ppm. The 14 F2 plane has a
slightly different center despite the same width. These are TopSpin processing
coordinates from the same acquisition, so this is evidence about the axis
convention and referencing metadata, not a physical chemical-shift standard.

## Export a 1D magnitude spectrum

```console
cargo run --locked --example process_1d -- <raw-path> <output.npz>
```

Replace the placeholders with actual paths, quoting paths containing spaces.
The example selects raw input when both raw and processed candidates exist,
checks rank 1, then applies:

1. A 1 Hz exponential window on axis 0.
2. Standard zero filling (inspect the resolved point count in preflight).
3. A centered, unnormalized negative-exponent Fourier transform.
4. Magnitude projection, retaining ambiguous absolute polarity.

The program prints its resource bound and `exported N magnitude samples`.
The output contains scalar samples, axis coordinates and an NPZ manifest.
These are demonstration choices: this example does **not** correct digital-filter
delay, estimate phase or establish a ppm reference. The result is not an
absorptive or quantitatively validated spectrum.

`process_1d` uses default reader semantics and therefore rejects JEOL. If adapting
it for JEOL validation, explicitly add
`.allow_experimental_vendor_semantics(true)` to its `ReadOptions` chain and
inspect `input.warnings()`. Read the [JEOL conventions and limitations](/jeol-conventions/) before
choosing corrections; opt-in alone does not validate the recipe.

## Process dense 2D data in ppm

```console
cargo run --release --locked --example process_2d -- <raw-path> <output.npz>
```

This example opts into experimental vendor semantics and uses source-evidenced
direct delay correction, 1 Hz windows, standard zero filling, negative Fourier
requests, magnitude projection and descending ppm axes. It requires dense rank-2
input with supported components, known delay state and sufficient axis reference
evidence. Missing calibration or unknown delay is an error; the example does
not invent a reference. Inspect the source before adopting its fixed policy.
The [JEOL conventions and limitations](/jeol-conventions/) explains shared components and axis signs.

## Inspect and change a plan

In `examples/process_1d.rs`, edit the `ProcessingOperation` list to choose your
operations. Preparation borrows the input and resolves the output shape, state
and conservative resource bounds before numerical execution:

```rust
// `plan` and `input` are the plan and Dataset from process_1d.rs.
let prepared = plan.preflight(&input, nmr::processing::ProcessingOptions::new())?;
for step in prepared.steps() {
    println!("{:?}: {:?}", step.target(), step.resolved());
}
println!("{:?}", prepared.resources());
let spectrum = prepared.execute()?;
```

Use the resulting `Dataset` for later operations to retain warnings, source
context and history. See [processing contracts](/processing-reference/) for
state requirements, [scientific conventions](/scientific-contract/) for FFT and
phase definitions, and [host integration](/host-integration/) for a shared work
ledger, cancellation and progress.

## Export or continue working

`PlotData::preflight(processed, MemoryLimits::new())?.execute()?` prepares scalar
samples and coordinates. Complex or hypercomplex results require an appropriate
projection first. `export_npz` refuses existing targets; choose a new filename.
See the [quickstart NPZ check](/getting-started/#read-and-export-a-known-spectrum)
for inspecting arrays. Magnitude removes phase information: keep the complex
Dataset if you need to phase or inspect its components later.

Save a [snapshot](/host-integration/#embedded-snapshot-contract) for offline
continuation, or an [execution report](/reproducibility/) for source and processing
evidence. NPZ alone contains neither a complete recipe nor a replayable project.

If a plan fails, inspect its typed error and step index. Resolve missing evidence
or unsupported operations before retrying; increasing a budget cannot fix a
scientific state error. See [troubleshooting](/errors-limits/#common-failures).

## Related API

Generate the reference for this checkout with `cargo doc --no-deps --open`.
Start with `ProcessingPlan`, `DensePipeline`, `PlotData` and `export_npz`.
Detailed [processing contracts](/processing-reference/) describe preparation,
resource accounting, state transitions and replay.
