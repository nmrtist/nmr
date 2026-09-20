---
title: Errors and resource limits
description: Structured failure and bounded allocation behavior.
---

## Common failures

| Symptom | What to do |
| --- | --- |
| Input cannot be found | Check the process working directory and quote paths containing spaces. Keep companion parameter files with the acquisition. |
| `Ambiguous` | Select a specific result directory or set `ReadPreference::PreferRaw` / `PreferProcessed`. |
| `Incomplete` | Restore the required primary or companion files; parameter files alone are not a spectrum. |
| `EXPERIMENTAL_VENDOR_SEMANTICS` | Decide whether the [documented experimental scope](/formats/) is acceptable, then opt in explicitly; do not silently retry. |
| Detection succeeds but reading is unsupported | Check the exact layout in the format matrix. Detection recognizes formats; it does not validate all decoding capabilities. |
| Missing ppm reference or unknown digital-filter delay | Supply scientifically justified evidence through the applicable API, or use a recipe that does not require it. Do not replace unknown values with zero. |
| `LimitExceeded` or insufficient work | Inspect required/limit fields and the prepared resource/work estimate. Grant a suitable budget or reduce the task; see the scopes below. |
| Automatic phase/noise/reconstruction fails | Inspect quality diagnostics and the method's domain. Do not report a rejected estimate as successful or substitute zero noise. |
| NPZ target already exists | Choose a new filename. Export deliberately refuses overwrite. Its parent directory must exist. |

For bug reports include the source revision, command or minimal code, error kind,
code and evidence, and non-private format metadata. See [Contributing](/contributing/).

## Error handling

`ReadError::kind` is the stable control-flow category; `reason` retains typed
details and `format` retains source context. Unsupported features, invalid
metadata, assertion conflicts, source changes, truncation, corruption, invalid
access, absent NUS coordinates, I/O, arithmetic overflow, allocation, and limit
failures remain distinct.

## Configure read limits

```rust
use nmr::raw::OpenOptions;
use nmr::ReadLimits;

let limits = ReadLimits::new()
    .max_source_bytes(2 * 1024 * 1024 * 1024)
    .max_metadata_bytes(32 * 1024 * 1024)
    .max_working_bytes(256 * 1024 * 1024)
    .max_region_bytes(64 * 1024 * 1024)
    .max_materialized_bytes(512 * 1024 * 1024)
    .max_component_lanes(16)
    .max_transform_coefficients(32)
    .max_modulation_period(16)
    .max_transform_work(100_000_000);
let reader = OpenOptions::new().limits(limits).open("data/experiment")?;
```

Source, metadata, temporary working storage, region output, and complete
materialized output have independent limits. Covered buffer sizes use checked
arithmetic and are checked before allocation; this is not a claim that every
Reader allocation is accounted for. Parsing and layout validation fail closed: the
library does not infer missing axes, units, trace order, references, or signs.
Materialized processed readers also preflight their numeric source buffers and
temporary decoding storage against the source and working limits separately.

Lazy `Reader` accesses charge simultaneous numeric buffers: region or complete
output plus the adapter trace-decoding peak, or the full trace and its cropped
copy, whichever is larger. Complete materialization checks this before copying
the source snapshot. A per-region limit cannot relax the limit fixed at open.
Reader-owned metadata and parser object expansion are not fully accounted for;
the current reader working limit is not a complete library-memory bound.
Bruker raw path and Parts do preflight their combined parameter-table expansion
before parsing; the path also charges owned input text. The parsed-table bound
remains charged during Bruker decoding and Reader accesses, including snapshot
chunking. Lazy Bruker NUS also charges its schedule and logical row map, checking
their construction peak before parsing coordinates. Other retained metadata
is not fully accounted for. The complete cross-format Reader resource model is
not yet available; no RSS or OOM guarantee is made.

`UnsupportedFeature` means that the input was recognized but its scientific
layout is outside the supported capability boundary. It includes a stable code,
optional canonical axis, and source evidence. It is not evidence that another
reader should reinterpret the same input. `InvalidMetadata`, `Corrupt`,
`Truncated`, `Invalid`, `LimitExceeded`, and `Allocation` are fatal for the
selected candidate and are never fallback signals.

## Resource scopes

Reader and processing working limits have different scopes.
They must not be treated as interchangeable total-memory caps:

| Entry | Numeric output | Working limit | Metadata limit |
|---|---|---|---|
| Lazy `raw::Reader` trace/region/materialize | Region/materialized cap also applies | Includes simultaneous output and adapter buffers; only migrated retained metadata is charged | Metadata input bytes at open |
| Complete processed reads / borrowed Parts | Materialized cap | Adapter-specific source and intermediate buffers; final output may be charged separately | Metadata input bytes |
| Explicit processing / Dataset regions | Output cap is a subset of working | Conservative total newly owned payload bound | Newly owned metadata payload |

JCAMP expansion can produce output larger than the working limit; its
`max_materialized_bytes` is the independent output guard. Borrowed input and
fixed stack buffers are excluded. JEOL lazy decoding uses an 8 KiB stack buffer
and reads contiguous runs within the selected trace without fetching unrelated
tile rows. Materialization still needs a temporary source copy equal to source
length; temporary disk has no separate configurable quota. Concurrent operations
must be budgeted separately. Full reader preflight/metadata accounting remains
outside the current resource contract; the limits above do not imply a single RSS bound.

`InputSource`, `ParameterErrorKind`, `ReadResource`, reader warnings, and detailed
read-error reasons are open to future variants. Structural diagnostic variants
also permit added fields; downstream matching should use `..` and a fallback arm.
Use `InputSource::memory(role)` rather than constructing its memory variant.
`EXPERIMENTAL_VENDOR_SEMANTICS` reports an explicit support boundary, not corrupt
bytes or permission to retry with a different interpretation automatically.

## Related API

Explicit processing accepts `nmr::resource::MemoryLimits` through
`ProcessingOptions::memory`, with finite 512 MiB output, metadata and total working
defaults. A borrowed `PreparedPlan::resources()` reports the checked bounds.
`ProcessingError::LimitExceeded` carries `ResourceKind`, `required` and `limit`;
size overflow and allocation failure remain different errors. Returned samples
and metadata are subsets of the total working bound, not extra amounts to add.
These are conservative library-controlled payload capacities, not process RSS,
allocator bookkeeping or a guarantee of recovering from operating-system OOM.
ReadLimits and standalone experimental interfaces have not yet migrated to this
complete shared contract.

`Dataset::read_region` also accepts `MemoryLimits`; `region_resources` returns
the corresponding bounds without copying samples. It checks all three limits
before allocating a `DataBlock`. `data::AccessError::LimitExceeded` has the same
resource/required/limit payload. Input Dataset storage is caller-owned and
excluded; returned layouts, sparse trace metadata, relative indexes and any
ordinal-validation index are included. This does not relax Reader open limits.

- `raw::ReadError` carries
  the stable `ReadErrorKind`
  category and typed `ReadErrorReason`.
- `ReadLimits`
  configures independent read and allocation limits.
- `processing::ProcessingOptions`
  bounds plan memory; `processing::WorkLedger`
  accounts for numerical work.
- Processing, plotting, and export expose their own typed errors from the
  `processing`,
  `plot`, and
  `export` modules.
