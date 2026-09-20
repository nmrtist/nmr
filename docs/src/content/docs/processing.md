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

Dense plans support rank 1 and 2 and reject sparse data and higher ranks before
numerical execution. `DensePipeline` is a fixed recipe with experimental automatic
policy. Explicit operations do not establish scientific correctness of your chosen
parameters; automatic estimators have [limited validation domains](/scientific-contract/).

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
