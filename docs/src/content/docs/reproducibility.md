---
title: Execution reports and comparison
description: Persistent execution evidence, exact source identities, and independent scientific tolerance checks.
---

`nmr::execution_report::write_json` exports the versioned
`nmr.execution-report.v1` JSON sidecar for a processed dataset. It records source
roles and SHA-256 values, original canonical input identities, decoded scaling
and sign rules, normalization evidence, requested and resolved operations,
algorithm identifiers, axis descriptions and lineage, experimental vendor interpretation status, execution segments, and
actual output digests. Coordinates include units and physical quantities.

```rust
use nmr::execution_report::write_json;

let dataset = nmr::read("data/processed-spectrum")?;
let processed = dataset.as_processed().ok_or("processed input required")?;
let mut report = std::fs::File::create("execution-report.json")?;
write_json(processed, &[], &mut report, 64 * 1024 * 1024)?;
```

The writer first validates finite report numbers and counts all UTF-8 output
against the supplied limit. It then streams a second pass to the destination,
without copying samples or building a JSON tree. Invalid metadata or an
insufficient report limit leaves the writer untouched. An I/O failure can leave a
prefix; publish from a temporary file when a complete report is required.

JSON is a persistent report, **not an automatic import/replay format**. Keep the
original numeric data, every parameter/sampling file, the report, the matching
source artifact and the actual application's `Cargo.lock`. To redo a computation,
verify source hashes, read using the recorded decoding rule and explicit
experimental choice where needed, construct an explicit plan from the recorded
resolved parameters, and compare its result. Existing in-memory history replay
continues to require strict input identity and rejects unsupported algorithm IDs.
The report does not embed source bytes or promise a future migration service.

Caller statements made with `ExternalAlgorithmDeclaration` appear only under
`external_algorithm_declarations`, labelled `caller-declaration-unverified`.
They cannot create applied library records or establish numerical quality. An
import without library processing has no invented processing history.
`known_experimental_vendor_semantics` flags recognized JEOL/Bruker-NUS origins;
false is not certification of externally declared or unclassified data.

## Schema and numerical representation

V1 uses stable, explicit snake-case field names and kebab-case enum tags. It does
not serialize Rust `Debug` output. Arrays preserve canonical/source/record order;
nullable identity or evidence means unavailable, not zero. Source SHA-256 is
lowercase hexadecimal; an unconsumed source has a null hash. Non-UTF-8 source
locators are omitted (null); paths are hints and never define identity. Finite
binary64 values use round-trippable JSON numbers. Non-finite values are errors.
The project is unpublished. Report schemas and algorithm contracts evolve in
place under their initial identifiers; development reports and snapshots have no
backward-compatibility guarantee.

The canonical protocols are `nmr.raw-descriptor.v1`, `nmr.raw-samples.v1`,
`nmr.raw-dataset.v1`, and `nmr.processed-descriptor.v1`,
`nmr.processed-samples.v1`, `nmr.processed-dataset.v1` respectively. These are
source identity protocols, not numerical tolerance assertions. Preserve matching
crate source when independently reproducing their complete encoding.

## Source and build identity

`build.rs` computes identities without Git. `nmr.source-files.v1` hashes its
NUL-terminated protocol tag followed, in sorted path order, by each relative
UTF-8 slash path's little-endian u64 byte length, path bytes, little-endian u64
content length and original bytes. Inputs are all `.rs` files below `src`,
`Cargo.toml`, `build.rs`, and `Cargo.lock` when present. The script is included in
the published package. Packaging may normalize Cargo.toml, so the package can
legitimately have a different source identity from the development checkout.

`nmr.build-inputs.v1` hashes its NUL-terminated protocol tag, the lowercase
ASCII source hash, and the queried `rustc -vV` identity with CR/LF replaced by
spaces and outer whitespace trimmed. It then hashes the environment names
`TARGET`, `PROFILE`, `OPT_LEVEL`, `DEBUG`, `CARGO_CFG_TARGET_FEATURE`, and
`CARGO_ENCODED_RUSTFLAGS` in that order. Each name and value has a preceding
little-endian u64 byte length. Encoded Rust flags are retained as UTF-8 hex in the
report to preserve Cargo unit-separator boundaries. This identifies captured compilation inputs; it is not a hash of the
linked binary or a promise that unrecorded linker/system inputs are identical.
Execution segments retain the build identity and pinned RustFFT version/backend.

`packaged_dependency_lock_sha256` identifies the lock file next to this crate,
which may differ from a downstream application's actual resolution. A dependency
library cannot reliably recover the consumer's complete Cargo resolution during
its own build: `resolved_transitive_dependency_identity` is explicitly null.
Preserve the application's actual lock file to close that boundary. Runtime CPU
features, rounding mode, denormal handling and complete floating-point environment
remain unknown. Binary64 storage and compile target features are recorded
separately; no cross-platform bitwise guarantee is made.

## Independent numerical comparison

`compare_processed` requires identical descriptors (including annotations),
shapes and component meanings. It never aligns coordinates, interpolates,
rotates phases, or fits an amplitude scale. `ComparisonOptions::new(atol, rtol)`
uses explicit caller-selected scientific tolerances; the result reports every
sample's absolute and relative error, L2 error, relative L2 error, and whether
all samples satisfy `abs(actual-reference) <= atol + rtol*abs(reference)`.
`max_difference_bytes` bounds the returned difference payload, excluding borrowed
inputs. `compare_samples` provides the same numeric metrics for equal finite
sequences without claiming anything about axes.

Nonzero error against a zero reference has undefined relative error (`None`),
while zero versus zero reports zero. Non-finite samples, non-representable
reported errors/norms, incompatible datasets and invalid limits/tolerances are
errors. These cases never produce a fabricated zero-error result. Norms use a
scaled reduction. Tolerance agreement and strict digest/replay identity remain
separate decisions.

For an isolated nonnegative peak, `compare_positive_scalar_spectra` additionally
accepts a half-open index ROI and reports maximum-coordinate displacement,
trapezoidal ROI area ratio and error leakage outside the ROI (outside-error L2
relative to complete reference L2). It requires calibrated monotone 1D frequency
coordinates, scalar components, at least two ROI points and positive reference
area. It uses absolute coordinate spacing, works with either axis direction and
chooses the first array index on tied maxima. It does not define peak assignment
for overlapping/signed/complex spectra, remove baselines, or decide a suitable
ROI. A global 25% amplitude deficit remains a 25% relative error and a 0.75 area
ratio; it is not normalized away.

NPZ V1 remains a plotting exchange format. Its manifest contains a history count
but no complete recipe, and omits the axis `quantity` field even when PlotAxis
retains it. Use this report alongside the NPZ and retained source data for
scientific provenance; the NPZ alone is not a complete scientific archive.
