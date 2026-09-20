---
title: Reading data
description: Detect and materialize datasets, or lazily access raw acquisitions.
---

For a runnable fixture-based example, start with [Getting started](/getting-started/).
Use the [format matrix](/formats/) to check the required companion files.
Snippets below assume a function returning `Result` so that `?` can propagate errors.

## Read a complete dataset

```rust
let loaded = nmr::read("data/experiment")?;
println!("{:?}", loaded.source_format());
println!("{:?}", loaded.identity());
```

`nmr::read` is the unified materializing entry point. `nmr::detect` uses the
same resolver and selection policy without decoding all samples. Configure an
exact format assertion, raw-versus-processed preference, limits, or Varian
interpretation with `ReadOptions`.

## Opt into experimental vendor semantics

All JEOL imports and Bruker NUS require an explicit evidence-scope decision:

```rust
let loaded = nmr::ReadOptions::new()
    .allow_experimental_vendor_semantics(true)
    .read("data/experimental.jdf")?;
println!("{:?}", loaded.warnings());
```

The same setter exists on `raw::OpenOptions`, `formats::jeol::Parts`, and
`formats::bruker::Parts`. Defaults reject these interpretations, while `detect`
continues to recognize the format. Opt-in retains experimental code paths for
validation; it is not independent confirmation of their scientific meaning.
See the [support and evidence boundary](/formats/).
Bruker NUS schedules do not cause a blanket experimental warning. Every JEOL
unified read, including NUS with a vendor or externally declared schedule, retains
`ExperimentalVendorSemantics`. Its `details` describe the disk-axis layout and
applicable evidence gaps; successful schedule validation does not qualify component
or acquisition semantics. Concrete metadata warnings remain independent.

## Inspect the result

Reading returns `Dataset` directly. Its private representation exposes
`as_raw()`, `as_processed()`, typed descriptor/provenance views and `metadata()`.
The selected format and path are optional because memory-created aggregates do
not invent a filesystem selection.

## Choose raw or processed data

The default preference is `ReadPreference::RequireUnique`: if complete raw and
processed candidates both remain, `read` and `detect` return `Ambiguous` before
decoding. To select processed data explicitly, use
`ReadOptions::new().preference(ReadPreference::PreferProcessed)`. `PreferRaw` or
an exact format assertion also makes the choice explicit.

Selection first excludes candidates missing required primary or companion files
when at least one structurally complete candidate exists, then applies the
raw-versus-processed preference. Thus an experiment root with complete raw data
and only parameter files for an incomplete processed result selects raw data.
Selecting that incomplete processed directory or its parameter file directly
still returns `Incomplete`; an exact processed format assertion does the same.
Structural completeness does not imply reader support, so a complete selected
layout can still return `UnsupportedFeature` without falling back to raw data.

Detection means recognition and candidate selection, not a pre-read support
probe. A selected format can still return `UnsupportedFeature` when `read`
validates a scientific layout outside the support matrix. Treat that result as
a normal capability boundary. Invalid metadata, assertion conflicts, source
changes, corruption, truncation, invalid requests, allocation, and limit
failures must not trigger fallback to a different reader or interpretation.

## Read a raw trace or region lazily

```rust
use nmr::raw::{self, Region};

let reader = raw::open("data/experiment")?;
println!("{:?}", reader.descriptor().logical_shape());

let coordinate = vec![0; reader.descriptor().axes().len() - 1];
let trace = reader.read_trace(&coordinate)?;

let shape = reader.descriptor().logical_shape();
let region = Region::new(vec![0; shape.len()], shape)?;
let data = reader.read_region(&region, 64 * 1024 * 1024)?;
```

`raw::open` keeps data lazy; `raw::read` and `raw::Reader::into_dataset`
materialize the acquisition. A region read uses the smaller of its per-call
byte limit and the limit configured in `ReadLimits`.

## Temporary storage and source consistency

Complete raw materialization hashes a private temporary copy of the numeric
source and decodes that same copy. It needs temporary disk space equal to that
source's length, adds copy I/O, and can fail if the temporary file cannot be
created or written. The copy buffer uses at most 64 KiB and respects the working
limit; the output limit is checked before copying. Trace and region reads remain
lazy. Keep source files static while reading: this is not an atomic snapshot of
all files in an acquisition. Parameters are identified by their actual parsed
bytes; Varian and JEOL binary metadata are checked against the copied source.

## Related API

- `raw::open` opens a lazy,
  format-detected reader with default limits.
- `raw::OpenOptions`
  configures limits and complete conflict-checked assertions before opening.
- `raw::Reader`
  exposes descriptor, trace, region, and full-materialization access.
- `raw::Region` describes a
  checked logical region; `raw::Trace`
  is the normalized result of a trace read.
- `ReadOptions` configures
  unified detection and materialized reading.
