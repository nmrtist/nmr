---
title: Data model and conventions
description: Axis, component, complex, state, and sparse-data semantics.
---

Raw axes are ordered slowest to fastest. The unique direct acquisition axis is last
and is the fastest-varying logical dimension. A shape counts logical points
only; raw component lanes are derived from `RawAxisKind`.

Every complex sample uses `R + iI`. Component-major trace storage keeps direct
points fastest. Readers may decode byte order, numeric scaling, vendor storage
order, supported sign conventions, and padding, but never apply FFT, filtering,
phasing, interpolation, scan normalization, or NUS reconstruction.

Source provenance exposes a typed, versioned `SampleNormalization` record with
source-block numeric scale factors and the stored-imaginary sign used to produce
public `R+iI`. Raw snapshots retain that record without retaining opaque vendor
metadata.

A reader applies the vendor-to-public sign transform of its selected format
rule. The [source register](/scientific-contract/#format-rule-sources) distinguishes
independent documentation, reference implementations and remaining evidence gaps.
Matching another application's historical array does not by itself establish
physical frequency direction or phase.

For Varian raw data, the binary words remain interleaved real/quadrature values,
but the reader negates the stored quadrature lane when producing public complex
samples. OpenVnmrJ commit `5e20f6f77dd5331473e47402153cb9dd310eb3d4`
decodes the scalar stream without a sign change and applies a negative-exponent
Fourier transform while its displayed frequency coordinate decreases with data
point index. The equivalent nmr representation uses ascending physical-frequency
coordinates, canonical `R + iI`, and the opposite stored quadrature sign. The
fixed source derivation applies to the supported direct quadrature; it does
not authorize new multidimensional layouts.

## Axis quantities

`spectral_width_hz` is evidence for the NMR spectral window or bandwidth declared
by source parameters, or retained through verified processing lineage.
`coordinate_span()` is the absolute distance between the first and last sampled
coordinates in the axis coordinate unit. They are deliberately independent: for
example, a Bruker processed axis with `SI` points has an endpoint span of
`SW_p * (SI - 1) / SI` after conversion to Hz. A single-point axis has no
coordinate span.

JCAMP `FIRSTX`, `LASTX`, `NPOINTS`, and consistent `DELTAX` establish coordinates
and point spacing, but do not establish acquisition bandwidth. Such axes return
`None` from `spectral_width_hz()` and expose their endpoint distance through
`coordinate_span()`.

Known nucleus labels are normalized across raw and processed readers to
mass-number-first ASCII notation, such as `1H`, `13C`, `15N`, `19F`, and `31P`.
The shared normalizer accepts every periodic-table element symbol and English
element name with a leading or trailing mass number. This covers Delta forms
such as `Carbon13` and `Fluorine19` without maintaining an isotope-by-isotope
alias list. The verified Delta special names `Proton`, `Deuterium`, and
`Tritium` map to `1H`, `2H`, and `3H`. Whitespace, angle brackets, a leading
caret, and conventional spellings such as `H1` are handled by the same rule.
Empty values and `off` mean no nucleus; unknown non-empty labels remain opaque
and are never guessed from frequency. Original text remains only in source
provenance as `VendorMetadata` or processed `SourceMetadata`.

| Source contract | Source form examples | Public `nucleus()` | Original retained in |
|---|---|---|---|
| Bruker acquisition / processing | `NUC1=<1H>`, `AXNUC=<1H>` | `1H` | Bruker vendor/source parameters |
| Varian `procpar` | `tn=H1`, `dn=C13` | `1H`, `13C` | Varian parameter records and raw text |
| JEOL Delta domain parameter | `Proton`, `Carbon13`, `Fluorine19` | `1H`, `13C`, `19F` | JEOL parameter records |
| JCAMP-DX `.OBSERVE NUCLEUS` | `<1H>` or isotope notation such as `<^13C>` | `1H`, `13C` | JCAMP source labels |

This mapping changes only the vendor-neutral axis value. Readers never rewrite
the retained source record, and tests use each source's own form rather than
substituting another vendor's spelling.

## Dataset ownership and context

`Dataset` is an opaque aggregate over checked raw or processed data. Borrowed
`DescriptorRef` and `ProvenanceRef` keep their scientific distinctions explicit.
`metadata()` retains import identity, warnings and the original reader selection
through unified processing. `kind()` describes current samples; the optional
`source_format()` describes the original selection, so a raw-derived processed output
can correctly retain a raw source format. Memory-wrapped datasets have no invented
selection. Cached canonical digests exclude these display/selection fields.

Use `as_dense_processed()` for a contiguous processed tensor; the aggregate does
not promise a universal sample slice or deep `Clone`.
`into_raw()`/`into_processed()` return private-field context wrappers in
`Result<RawDatasetWithContext, Dataset>` / `Result<ProcessedDatasetWithContext, Dataset>`.
A mismatch returns the entire unchanged input. Successful narrowing preserves
samples, cached digest, source selection, warnings and history without copying or
rescanning samples. Shared access is available through `raw()`/`processed()` and
`Deref`; `into_dataset()` restores aggregate ownership. Explicitly dropping the
aggregate context uses `into_raw_data()`/`into_processed_data()`.
`ProcessingPlan::apply`, `DensePipeline::apply` and `NormalizedAcmeV1::apply`
retain context. Domain transitions remain recorded operations.


## Requests, axes and state

`Window` and `ProcessingOperation` are request descriptions, not proof of valid
parameters. Convenience window constructors reject intrinsic invalid values;
public variants can still describe an invalid request. `ProcessingPlan::new`
checks non-emptiness, while preflight is the authoritative parameter/state check.
`PhaseCorrection` and `ZeroFill` have private fields and checked constructors.
Named phase setters (`zero_order_degrees`, `with_first_order_degrees`,
`with_pivot_fraction`) make degrees and array-width fractions explicit.

`AxisIndex` and processing-operation axis numbers denote positions in the current
descriptor. They do not bind a dataset or follow reordered axes; source ancestry
uses `InputAxisRef`. Requests use descriptor positions and explicitly named physical units.

Public enum evolution distinguishes closed mathematical sets (Raw/Processed and
Fourier +/-), extensible classifications (formats, source/error/resource kinds),
and extensible read-only results. Open enum variants are matched with a fallback;
read-only struct variants also use `..`. Constructible request variants such as
`Window::Exponential { lb_hz }` deliberately retain their listed fields. New
configuration contracts use a new variant or a checked parameter object.

## Owned regions and lineage

`Dataset::read_region(&Region, MemoryLimits)` returns an independent owned
`data::DataBlock` for either representation. `region_resources()` computes its
output, metadata and working bounds before sample copying. The block keeps an
absolute region; `as_raw()` retains sparse observation ordinals and coordinates,
while `as_processed()` exposes local tensor coordinates and every component.
It remains usable after dropping the input. Region copying creates no processing
history or new processing origin.

Declared raw lineage uses `InputAxisRef::new(InputSlot::new(0), axis)`. The declaration currently owns one raw snapshot in slot
zero. Selected/permuted axes remain supported, while missing slots, out-of-range
axes and duplicate references fail aggregate validation. Library-derived lineage
and history references are constructed internally.

`RawDataset::capabilities()` is derived on demand from canonical descriptor,
sampling, and correction state. Reading does not promise ppm conversion,
group-delay correction, NUS reconstruction, or dense processing. A
`ProcessingPlan::preflight(&Dataset, options)` is the final authority for those
optional capabilities and borrows its immutable input until execution. The
lower-level raw API retains `ProcessingInput` and `preflight_raw`.

Raw descriptor/dataset identities use v1, excluding axis display labels and
the acquisition title. Raw sample encoding remains v1; scientific fields and
source-byte identities remain bound. Low-level prepared raw execution and raw
replay allow display-only differences while retaining prepared/historical output
labels and the matched input's source snapshot. Changing actual parameter-file bytes still fails strict replay. Preserve the
matching implementation for development snapshots; see [pre-release changes](/contributing/#pre-release-changes).

## Sparse observations

A NUS schedule lists measured zero-based indirect coordinates in acquisition
order. Repeated coordinates are retained as separate observations; the reader
does not choose, sum, or average them. Single-trace access to a repeated
coordinate reports a structured ambiguous error. Missing coordinates remain
absent and report structured unsampled errors instead of being filled with
zeroes. `ProcessingPlan` and `DensePipeline` reject sparse inputs. The separate
fixed IST kernel accepts caller-prepared `IstInput`; it does not automatically
adapt a sparse dataset, combine duplicates or attach processing history. See
[the exact IST boundary](/scientific-contract/#supported-operation-combinations).
For a Dataset-based reconstruction path, use [NusSettings or AutoNusSettings](/nus-automatic/).

## Related API

- `raw::RawDataset`,
  `raw::RawDescriptor`,
  and `raw::RawData` separate
  acquisition metadata, layout, and samples.
- `axis` defines axis roles,
  domains, units, coordinates, directions, and frequency evidence.
- `raw::SamplingSchedule`
  represents measured NUS coordinates without inventing missing samples.
- `processed::ProcessedDataset`
  and `processing::ProcessingHistory`
  represent processed output and its recorded transformations.
