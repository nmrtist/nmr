---
title: Processing contracts
description: Preparation, state transitions, resource accounting and replay boundaries.
---

Use [Processing and export](/processing/) for the runnable workflow. This page
explains the contracts needed when constructing or inspecting a plan.

## Supported inputs

`ProcessingPlan` applies an explicit sequence to dense rank-1 or rank-2 data.
The processed descriptor and component tensor themselves support N-D data;
rank-3 and higher processing requests return `UnsupportedRank` before numerical
execution. Declared raw lineage can select or permute source axes. A direct
acquisition role may appear anywhere in a processed descriptor, but current
processing requires that role, when present, to occupy the fastest axis.
`DensePipeline` provides the fixed workflow for delay correction, windowing,
zero filling, Fourier transforms, supported quadrature recombination, phase,
projection, reference conversion, and optional baseline correction. Both
`ProcessingPlan` and `DensePipeline` reject sparse input. The fixed IST profile
is a separate `IstInput` to `IstOutput` kernel; it is not a pipeline selector
and does not construct a processed dataset or history. The separate
`NusSettings::prepare` adapter executes acquired-row F2 processing followed by
general-grid reconstruction, producing a checked Dataset and library history;
see [spectrum operations and estimation](/spectrum-operations/). Fixed-grid limits are in
[Scientific contracts](/scientific-contract/#supported-operation-combinations).
The supported scientific foundation is the checked data model and explicit
deterministic processing. Automatic pipeline policy, ACME, AsLS and the fixed
512/128 IST kernel have a separate experimental scientific scope. They do not
promise general inference quality. See [pre-release changes](/contributing/#pre-release-changes) before preserving snapshots.
Replay success does not establish bitwise numerical reproducibility.

The [scientific contract](/scientific-contract/) defines Fourier normalization,
nonzero time origins, Nyquist layout, window/first-point weighting, phase
formula, identity requests and the accepted combinations for imported spectra.
It also links format rules to independent sources and bounded analytic tests.

## Preparation and execution

The unified path borrows a `Dataset` during preparation:

```rust
let prepared = plan.preflight(&dataset, nmr::processing::ProcessingOptions::default())?;
for step in prepared.steps() {
    println!("{:?}: {:?} ({})", step.target(), step.resolved(), step.algorithm_version());
}
let output = prepared.execute()?;
```

`PreparedPlan<'a>` owns resolved steps and borrows the immutable input. Execution
consumes it, takes no replacement input, and retains aggregate reading context.
The `ProcessingPlan` can be reused or released after preparation. Preflight uses
cached aggregate identities and validated current state without scanning samples;
FFT construction and retained plan/scratch have a checked bound before planning,
and retained prepared vectors are checked before reserve and added to the numerical
working budget. Immutable processed axes share storage across descriptions.
Axis backing, state arrays and sampling maps are also charged before construction,
including new axes from execution and output-history validation. Copied raw
snapshots, source records, history and aggregate reading context are charged as
well, together with conservatively bounded temporary descriptor/index arrays.
`prepared.resources()` reports output, metadata and total working bounds.
`prepared.steps()` borrows each original request, resolved parameters, rule
identifier and before/after descriptor. These are pending execution facts, not
history records. For example, `StandardZeroFill` exposes its resolved point count,
and digital-filter correction exposes the actual delay/skip/fold/residual values
before execution. The same frozen parameters are recorded after execution.
`ProcessingOptions::memory(MemoryLimits::new()...)` sets their independent limits;
output and metadata are included in working bytes. Allocation limits return
`ProcessingError::LimitExceeded` with resource, required and limit fields.

`DensePipeline::plan(&raw)` exposes its policy as an ordinary `ProcessingPlan`.
Use that plan with the unified path above, or use
`DensePipeline::apply(&dataset, options)` to retain aggregate context.
Low-level `process_dataset_with_options` applies the same recipe to `RawDataset`.

`NormalizedAcmeV1::preflight(&dataset, request, options)`
returns `PreparedAutoPhase`, with `resources()` and `estimated_work()`. Preparing
does not select a trace or copy processing history. `execute_with_context(&mut context)` checks
the entire conservative work reservation before any history copy, representative
selection or optimizer allocation. The memory bound covers those phases, final
correction or allowed scalar fallback, history and aggregate context. The output
bound covers the larger successful Cartesian result even if fallback produces
scalar samples. `apply` is the convenience form of the same path.

The guarded ACME rule accepts only its documented single-Lorentzian validation
domain. `QualityUnverified` is always an error, including with
`ContinueUnphasedReal`; that policy only handles the existing explicit no-signal,
undefined-objective or nonconvergence failures. Those failures record an attempted
automatic operation followed by `UnphasedReal`, never a successful phase claim.

## Resource accounting

Resource limits are per call unless a mutable `WorkLedger` is reused explicitly.
Cloning a ledger creates an independent counter. No dataset carries an implicit
cross-read/process/export budget session.

| Entry | Output limit | Working meaning and input boundary |
|---|---|---|
| Explicit plans, automatic-phase application, Dataset region copies | Returned numeric sample capacity | New metadata plus peak live numeric buffers, including output; excludes the borrowed resident input |
| `PlotData::preflight(input, MemoryLimits)` | Copied scalar samples | Samples + materialized coordinates/indices + labels/shape + copied provenance/history; excludes borrowed input and existing shared backing |
| Full stream-oriented imports with `ReadLimits` | `max_materialized_bytes` limits final decoded samples | Temporary source/decoder buffers use `max_working_bytes`; final output can be charged separately, so this is not the same total as processing working bytes |
| Lazy Reader trace/region materialization | Trace/region/materialization limits according to the access operation | Access reserves retained result/crop buffers and applicable retained metadata as documented by the reader; it is distinct from full stream-import temporary buffering |
| Standalone IST | `IstOptions::max_output_bytes` | Its fixed-kernel bound includes a new compact copy of measured samples, reconstruction buffers, output and audited FFT planning/scratch; borrowed input is excluded, and it does not use `MemoryLimits` |

Limits describe conservative library payload capacities, not allocator overhead
or process RSS. Output and metadata are subsets of processing/plot working bytes;
do not add them again. Standalone `optimize` and baseline slice kernels remain
expert mathematical interfaces with their separately documented controls; use
dataset application when history, context and shared memory limits are required.

For a `RawDataset`, `preflight_raw(&ProcessingInput, options)` resolves operations
and output descriptors without borrowing the sample storage. Its `PreparedRawPlan`
can be inspected and applied to a raw dataset with the matching canonical identity;
execution rechecks display metadata and provenance allocations before copying.
`apply_raw` performs preparation and execution in one call, returning a
`ProcessedDataset`. These raw entry points also serve unified preparation and
automatic pipeline execution internally. Use processed `apply` to continue a
`ProcessedDataset`, or the unified path above to retain aggregate reading context.

## History and input bindings

`ProcessingHistory::inputs()` exposes ordered external input bindings, and
`replay(&[&dataset], options)` replays from the original aggregate while retaining
its reading context. Current histories and execution support one external start;
missing or extra supplied inputs, unsupported multi-input histories and wrong
representation kinds are rejected. For two-input arithmetic use the separate `LinearCombination` derivation
boundary described in [Combine two spectra](/spectrum-operations/#combine-two-spectra);
it has its own ordered-input replay contract, not a multi-input plan history.

Executable plan requests use `ProcessingOperation`. A history record exposes a
`ProcessingRequest`, which can also describe an automatic phase request from
`NormalizedAcmeV1::apply`; automatic phase is not a plan variant.
`operation.target()` and `record.target()` report an `OperationTarget` with
actual axis or dataset scope. Global `Projection` has dataset scope and no
placeholder axis. Use record accessors and `..` when matching non-exhaustive
record variants so consumers allow additional record fields.
Canonical digest types have their public home in `nmr::provenance`.

`axis_lineage()` uses `InputAxisRef { input slot, source axis }` through checked
accessors. Slots belong to the owning history's input collection; a declared raw
origin separately uses slot zero for its snapshot. Ancestry identifies axes and
does not establish coordinate direction or an FFT bin mapping.

## Fourier and projection

The default FFT exponent sign is negative. Transforms are unnormalized and
centered, with the even-length Nyquist bin at the left. A positive sign is
available explicitly; it does not imply general compatibility with another
processing package.

Explicit dense processing uses pinned RustFFT 6.4.1 Radix4 for power-of-two
lengths and Bluestein with a Radix4 inner transform for other lengths. This
provides an audited planning allocation bound, but can be slower than automatic
SIMD/mixed-radix selection and can change roundoff. Preflight creates no FFT plan;
the working limit includes planning, retained plan and scratch before execution
constructs it. Experimental IST uses the same audited bounded backend and performs
its memory estimate before creating FFT plans.

`Projection::Real` extracts component zero on every signal axis with
`PolarityState::Ambiguous180`, without requiring phase history or declaring an
absorption spectrum. It accepts Cartesian frequency signal axes, including
imported complex spectra. `RealAbsorptive` requires established positive
polarity, while `RealSigned` permits a global 180-degree inversion; both retain
their explicit phase-policy requirements. A phase-rotation record alone does
not establish scientific phase correctness.
Automatic phase correction uses the deterministic `NormalizedAcmeV1` profile;
recoverable quality failure is explicit and never silently reported as a
successful phase solution.

## Replay boundaries

Successful raw processing stores the canonical raw descriptor, sample, and
dataset digests plus the typed reader sample-normalization record in
`ProcessingHistory`. `ProcessingHistory::replay_raw` checks that complete
identity and executes the recorded resolved operations without consulting
opaque source metadata or rerunning automatic inference.

Supported Bruker processed imports also support strict
`ProcessingHistory::replay_processed`. Additional `ProcessingPlan::apply` calls
retain the original import binding and append execution segments. Replay accepts
that original input, checks canonical samples/state, actual source-byte digests
and applied decoding, then executes the resolved segments. Moving the input
directory is allowed; changing parameter comments is a strict source mismatch,
even when canonical samples and scientific semantics are unchanged.

Reader records expose NC_PROC scaling, scalar encoding, byte order and per-plane
tensor component coordinates. Complete procs/proc2s text is available through
`BrukerSourceMetadata::documents`, including unknown/global records, multiline
values, comments and original line endings. The metadata input-byte limit applies
to the combined document lengths; it is not a total parsed-object memory limit.
JCAMP XYDATA path and Parts imports also support strict replay, with XFACTOR and
YFACTOR available through `ProcessedReadRecord::transform`. Identical path/Parts
bytes have the same reading identity; Parts has no filesystem locator. Exact
parameter ranges and byte offsets are exposed by `JcampSourceMetadata::records`.
Supported rank-1/rank-2 JEOL scalar/Cartesian imports and processed Parts also carry
reading records for numeric encoding, section sign, reordering, valid
window and coordinate scaling. `JeolSourceMetadata::parameters` returns the
original typed JEOL records and non-sample byte areas, preserving raw units and
classes. Processed JEOL rank 3 and higher remains unsupported.
Caller-created memory inputs without external sources
can replay by canonical identity. Reader execution provenance cannot be attached
to caller-supplied replacement samples.

Replay success means input checks and execution succeeded. It does not assert
that output samples were compared against a previous result. Execution segments
record their actual output digests; available environment facts are queryable,
with the configured explicit FFT backend and pinned RustFFT version recorded.
The build identifier binds packaged source and the lockfile when present;
unavailable other-dependency/CPU configuration remains unknown. Full
numerical comparison remains the caller's responsibility.

`export_npz` writes deterministic schema-versioned plot data and refuses to
overwrite an existing target.

## Axis reversal and delay correction

`ReverseAxis` reverses samples and known coordinates in either direction. A
single-point axis is recorded without changing its samples or coordinates;
unknown coordinates are rejected. Multi-point reversal clears the library FFT
bin-mapping capability, even after a second reversal restores sample order.
Frequency-domain delay correction therefore remains rejected after reversal.
Reversal also rejects changes to coordinates still required by pending encoded
component modulation; decode those components first.
Reversal records use `reverse-axis.v1`.

`DigitalFilterCorrection::AcknowledgeZeroDelayV1` records that pending zero-delay
evidence has been handled without changing samples or coordinates. Automatic
dense processing emits this record for known zero delay; unknown delay remains an
error, and explicit phase-ramp profiles still reject a zero numeric correction.
Processed datasets retain validated current delay, reference and FFT state.
Completed corrections retain their evidence and reject repeated correction,
including an explicit replacement delay. A nonzero retained time-domain residual
remains pending for its later correction.

## Recorded algorithm identifiers

Processed descriptor, dataset and sample digests use their v1 protocols.
Delay correction histories use
`frequency-domain-phase-ramp.v1` and `time-domain-shift-fold.v1` for the
state contract and named V1 numeric profiles.

Current component, tensor-baseline and projection records use
`linear-component-transform.v1`, `asls.v1` and `projection.v1` respectively.
AsLS scales the fixed smoothing penalty by the fourth power of point
spacing relative to its reference grid; `BaselineProfile` supplies an extensible
selection slot. Guarded automatic-phase records use
`normalized-acme.lorentzian-guard.v1`.

