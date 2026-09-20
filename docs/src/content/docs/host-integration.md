---
title: Host integration and snapshots
description: Synchronous tasks, checked external results and portable embedded storage.
---

Keep `Dataset` throughout an application workflow. `ProcessingPlan::apply(&input)`
retains selection metadata, warnings and provenance. Use a one-operation plan's
`preflight` to query availability: it runs the same state transition used by execution.
A successful preview is not an executed history record.

See [spectrum operations and estimation](/spectrum-operations/) for spectrum operations, staged
phase/baseline analysis, binary input boundaries and the sparse-to-F2-to-IST-to-F1
path. The executable example includes snapshot restoration after deleting its
temporary source, with all advanced host analysis remaining external.

## Synchronous execution control

`ExecutionContext::new(&mut ledger)` borrows a `WorkLedger`. Attach a shareable
`CancellationToken` with `with_cancellation` and a borrowed `FnMut(ProgressEvent)`
with `with_progress`. Callbacks run on the caller's thread; nmr creates no runtime
or worker pool. Ordinary calls use a default context. Reuse the same context (or
ledger) across plan application, ACME, IST, replay and export.

Use `apply_with_context`, `execute_with_context`, `read_with_context`, reader
`read_trace_with_context` / `read_region_with_context` / `into_dataset_with_context`,
`PlotData::from_processed_with_context`, report `write_json_with_context`, snapshot
read/write and NPZ export/write context variants. Reader methods take context
before their coordinates; other aggregate methods take it last.

Preflight does not spend numerical work. Execution checks the conservative bound
before its numerical allocations, then charges blocks entered; cancellation keeps
those charges. `WorkLedger` is not `Copy`; explicit `Clone` makes an independent
counter. Progress contains a stage, optional zero-based step, completed units, and
an exact total or upper bound when known. It is not an elapsed-time percentage.
Numerical work, transferred I/O bytes and peak library payload bytes are separate.
Peak payload is the largest reported simultaneous allocation estimate, not a sum
of calls, host-retained data or process RSS.

Point loops check at bounded blocks, FFTs between traces, iterative algorithms at
iterations and internal loops, and I/O between buffered transfers. A single FFT,
allocation or OS call can determine response latency. Cancellation returns a
separate typed error and no partial `Dataset` or successful unfinished history.
NPZ file export uses a temporary file and checks before publishing. Generic `Write`
may contain a prefix on failure: the host owns transaction commit.

`ProcessingError::Step` adds `step_index`, `target`, `phase`, and its typed `source`.
Indices are zero-based; whole-plan errors have no index. Use `code()` and
`root_cause()` for branching, and retain the outer error for location. Error text
is explanatory. Host recipe IDs can map directly to step indices.

## Host-produced scientific results

`Dataset::derive_external_processed(descriptor, samples, axes, declaration)`
requires a processed parent. Each output axis is `ExternalAxisSource::Parent`
with input slot zero and a unique valid parent position, or `New`. Cropping,
resampling, permutation and dimension reduction are allowed. The map records
source relationships; the host remains responsible for the coordinate transform.

`ExternalAlgorithmDeclaration` records algorithm ID, version, statement, and
`with_parameters(format, bytes)` for exact host-owned parameters. Record random
seeds there. The boundary retains parent descriptor, digests, sources, metadata
and history without ancestor samples. The output starts fresh checked state from
its descriptor. Inherited FFT bin mappings, phase success/failure, corrected-delay
facts and acquisition coordinate maps do not survive. Calibration and Cartesian
meaning must be declared through checked model APIs. Reusing library-derived
provenance with replacement samples is rejected.

A boundary is never `ProcessingRecord::Applied`. Its output has a recomputed
content/state digest; equal scientific content may have equal identity despite
different declarations. Library history after the boundary can replay when the
host supplies the matching boundary dataset. nmr does not execute host algorithms
or obtain ancestor samples automatically.

## Effective axis evidence and missing sampling tables

`processed.axis_evidence(axis)` returns current reference frequency, reference
authority, optional full carrier calibration, actual observe frequency, and
digital-filter state. The index refers to the current descriptor, including after
column slicing. This borrows the checked processing state without replaying
history. Binning retains the reference and its actual coordinate means; do not
reconstruct bin spacing from acquisition SW/N. Binary arithmetic uses the first
input's coordinate reference, retaining filter state only when both inputs agree.
An imported SF supplies a ppm-to-Hz interval scale; it does not supply carrier ppm.
Imported ppm coordinates remain usable when a reference is unavailable.

For a missing Bruker nuslist or JEOL embedded coordinate list, pass a
`SamplingDeclaration` to `ReadOptions::sampling_declaration`. It names an
`AssertionId`, a nonempty source description, full indirect grid, original index
rows, explicit `SamplingIndexBase`, and indirect component lane counts. The
reader checks those declarations against vendor original grid and physical
observation/lane counts. Existing vendor lists must agree exactly, including
order and repeats. It never rewrites vendor files or silently deduplicates rows.
The accepted declaration is available at `raw.sampling_schedule().declaration()`
and survives snapshots and sample-free execution evidence.

This currently supports 2D Bruker FnTYPE=2 and 2D JEOL reduced-grid acquisitions
with `y_orig_points`, typed `y_sweep`, and an uncropped indirect observation list.
The experimental vendor opt-in remains required. Other formats, missing original
grid/calibration, conflicting evidence and unsupported ranks remain explicit
errors. Declaration payload and normalization storage are reserved against read
metadata/working limits; row normalization checks cooperative cancellation.
IST still rejects repeated observations explicitly.

Run `cargo test --test reading --test processing --locked` for the JCAMP 5.01,
missing-table completion, SF semantics and snapshot v1 regression assertions.
Run `cargo test --test snapshot --locked` for snapshot roundtrips and validation.

## Embedded snapshot contract

`snapshot::write_snapshot(&dataset, &mut writer, limits)` writes a version-one
frame; `read_snapshot` returns a `CheckedSnapshot`. Inspect its descriptor,
metadata and scientific digests before explicitly calling
`restore(AcceptRecordedHistory)`. Restoration accepts archived facts without
claiming re-execution. Existing segments are marked `accepted_archive`; new steps
capture the current environment. The archived environment remains its recorded
value even when another build restores it.

Snapshots include materialized dense/sparse raw data (including NUS order and
repeated observations), processed data, exact binary floating values/coordinates,
source parameter metadata, aggregate context, scientific state, history and
external evidence. No vendor path is accessed during restore. UTF-8 paths travel
between platforms; native non-UTF-8 paths require a matching platform encoding.
The current samples suffice for continuation; hosts must store earlier inputs
needed for full recomputation separately.

The wire frame is `NMRSS001`, little-endian u32 version and u64 payload length,
a typed versioned metadata tree with binary64/complex arrays in blocks of at most
4096 elements, SHA-256 of header plus payload, and `NMRDONE1`. Integer and floating
bits use little-endian encoding. Limits bound frame size, sample bytes, metadata,
working payload and nesting before allocation. The reader validates integrity,
model invariants, supported algorithm/history versions, current scientific digest,
state transitions and evidence links. It checks ancestor descriptor/digest
relationships but cannot recompute ancestor sample digests without those samples.
A checksum detects corruption; it does not authenticate the host archive.

Snapshot v1 uses explicit field and tag ordering, independent of Rust runtime
model layout. During pre-release development the current schema evolves in place;
there are no migration branches for development snapshots. Reports use
`nmr.execution-report.v1` to express external boundaries and accepted history.
JSON reports are not replay inputs.

A checked restore, host history acceptance, successful replay and numerical
agreement are four separate outcomes. Hosts own compression, databases, project
formats, recipes and file transactions.

For a complete standalone byte buffer use `snapshot::decode_snapshot` (or its
context variant), which rejects trailing bytes. `read_snapshot` reads one frame
from a stream and deliberately leaves following container bytes untouched.
