---
title: Format support
description: Supported raw and processed layouts, evidence and rejection boundaries.
---

Header recognition is not a support claim. Anything outside this matrix is
rejected with a structured `UnsupportedFeature`, `InvalidMetadata`, or
corrupt-input error.
Consequently, a successful `detect` result does not guarantee that `read` will
accept the selected layout.

The matrix describes implemented support with synthetic regression coverage.
Independent evidence varies by rule, particularly for JEOL section/sign and
digital-filter interpretation; see [format-rule sources](/scientific-contract/#format-rule-sources).
Successful decoding and a recorded rule identifier do not constitute vendor certification.

Default reads reject all JEOL layouts and Bruker NUS with the code
`EXPERIMENTAL_VENDOR_SEMANTICS`. Explicitly enable
`allow_experimental_vendor_semantics(true)` on `ReadOptions`, `raw::OpenOptions`,
or the relevant vendor `Parts` to use those interpretations. Successful Bruker NUS reads
omit the blanket experimental warning. All JEOL unified reads, including NUS, retain
`ReadWarning::ExperimentalVendorSemantics` with layout-specific `details`. Raw/Parts callers record their explicit
choice themselves. Experimental rules may change independently of the stable
format contract. The opt-in does not admit unsupported JEOL layouts or Varian NUS.
The [JEOL conventions and limitations](/jeol-conventions/) reference explains
component signs, time coordinates and the scope of independent validation.

| Vendor | Accepted input | Supported boundary |
|---|---|---|
| Bruker | `fid`/`ser`, acquisition parameter files, optional `nuslist` | 1D; 2D AQSEQ 0 with QF, States, States-TPPI, or Echo/AntiEcho; dense; FnTYPE 2 NUS (experimental opt-in) including repeated observations and zero-padded preallocation; dense 3D AQSEQ 0 with QF, States, States-TPPI, or Echo/AntiEcho on each indirect axis; int32/float64, both byte orders, contiguous or K-block rows |
| Bruker | processed `1r`/optional `1i`, or XDIM-tiled 2D processing data | 1D scalar/Cartesian; scalar `2rr`, rectangular `2rr/2ri` or `2rr/2ir`, or full Cartesian quartet `2rr/2ri/2ir/2ii`; int32/float64 and both byte orders; calibrated ppm axes when SW_p, SF, and OFFSET are complete |
| JEOL (experimental opt-in) | raw Delta JDF | 1D axis types `[1]`/`[3]`; 2D `[3,1]`, `[3,3]`, and `[4,4]`; dense and list-defined NUS on the original grid; float32/float64; declared byte order; implemented tile reorder, component sign, and valid-window crop |
| JEOL (experimental opt-in) | frequency-domain Delta JDF | 1D `[1]` scalar and `[3]` Cartesian; 2D `[3,1]` parameter series and `[3,3]` Cartesian; declared precision, byte order, and valid-window crop |
| Varian | raw `fid` plus `procpar` | One direct axis; one ordinary non-grouped array plus direct; or the strict 2D `phase=[1,2]` v1 layout with default ptype or eight explicit f1coef coefficients; big-endian int16/int32/float32 with block scale |
| JCAMP-DX | NMR SPECTRUM `XYDATA=(X++(Y..Y))` | terminated single-block scalar 1D, JCAMP-DX 5.00 or 5.01, Hz or ppm X coordinates (observe frequency/nucleus optional), scaled X checkpoints, inline comments, AFFN/PACKED/SQZ/DIF/DUP/DIFDUP |

## JCAMP-DX details

Single-point JCAMP requires `FIRSTX == LASTX`; contradictory endpoints are rejected.
The [IUPAC 5.01 recommendation, example 2](https://old.iupac.org/jcamp/protocols/dx5-01-corrected.pdf)
retains ordinary NMR XYDATA. Accepting its version label does not authorize LINK,
NTUPLES, XYPOINTS, peak tables, other data classes, or unsupported encodings.
Version labels, duplicate fields, checkpoints, declared counts and END remain checked.

## Bruker details

Bruker processed `SF` is a spectral reference frequency, distinct from acquisition
`SFO1`; see the vendor's [Acquisition Reference](https://nmrcore.chem.kuleuven.be/media/files/guides/bruker_manuals_4.2.0/acquisition-reference.pdf).
The processed reader retains SF as `ProcessedAxis::spectrum_reference()` with
the rule `bruker.processed-sf.v1`. It does not read acquisition companions, so
it leaves observe frequency unknown. SF alone does not establish carrier ppm
or a digital-filter correction. Missing SF warns with `ReferenceFrequency`.
JCAMP endpoint coordinates do not supply acquisition spectral-width evidence;
use the common coordinate span for endpoint distance. Bruker `SW_p` and typed
JEOL sweep parameters remain source-declared spectral-width evidence even when
their numeric value differs from sampled endpoint span.

Older Bruker status files may omit `SW_h` while retaining `SW` in ppm. In that
case the reader derives the same Hz evidence as `SW * SFO1`; it does not infer a
width when either typed value is absent or invalid.

Bruker digital-filter delay uses an explicit non-negative `GRPDLY` first,
including known zero. When `GRPDLY` is missing or `-1`, the reader checks integer
`DSPFVS`/`DECIM` fields against the original version 10/11/12/13 table. For example,
12/16 resolves to 71.625 logical complex sample points. Table values carry
`bruker.dspfvs-decim-table.v1` authority and `canonical.group-delay.v1` derivation;
the original parameters remain in source metadata. This evidence follows the
effective axis state, snapshot v1, execution reports and replay. No experimental
opt-in is required. Unsupported or incomplete combinations remain `Unknown`;
malformed present fields and other negative `GRPDLY` values remain errors.

Under explicit experimental opt-in, Bruker NUS schedules retain acquisition order and may contain repeated logical
coordinates. Materialization preserves every repeated observation. Asking for
one trace by a coordinate that occurs more than once returns an ambiguous-access
error instead of choosing, summing, or averaging observations. A `ser` file
preallocated to `NusTD` is accepted only when bytes beyond the acquired `TD`
rows are zero.

For a complete Bruker processed quartet, public component coordinates are
`[F1, F2]`: `2rr -> [0,0]`, `2ri -> [1,0]`, `2ir -> [0,1]`, and
`2ii -> [1,1]`. Stored plane values are scaled by `NC_proc` without a sign
change. A nonrectangular component set (including three planes or a diagonal pair) or
a malformed extra plane is an error and never falls back to scalar `2rr`.
XDIM decoding respects tiles crossing the valid SI boundary; padded values do
not enter the output. States FnMODE=4 uses identity lane decoding, without the
alternating sign used by FnMODE=5.

## JEOL details

For JEOL `[3,1]` arrayed acquisitions, a header ruler abbreviated to Tesla is
projected as `TeslaPerMeter` only when the typed `y_acq` parameter references a
compound `T * m^-1` record. The header SI prefix scales coordinates into T/m.
Unrecognized compound parameter units are not reduced to their first base unit.
The axis also carries `AxisQuantity::MagneticFieldGradientStrength`, so consumers
do not need to infer experiment meaning from the unit or parameter spelling.
An array parameter referenced in the same way with a typed seconds unit carries
`AxisQuantity::TimeDelay`; this identifies the pseudo-dimension without claiming
a more specific relaxation model. The typed JEOL `experiment` value is retained
as the portable pulse-program name.

When that gradient axis is accompanied by typed JEOL time parameters, portable
diffusion acquisition metadata retains the gradient parameter, pulse duration,
diffusion interval, optional recovery delay, and their exact source parameter
names. Values are converted to seconds. The vendor gradient-shape label is
retained without assigning a shape coefficient; no b-value, diffusion fit, or
inverse Laplace result is derived by the reader.

For raw JEOL time-domain data, a declared enabled digital filter with complete,
consistent `orders` and `factors` stage metadata experimentally establishes
`GroupDelayState::Pending` with typed delay evidence on the direct axis.
Only a string `digital_filter=FALSE` (case-insensitive) establishes `NotApplicable`.
A missing switch, wrong type, invalid string, or malformed/partial enabled-stage
metadata leaves the state `Unknown`. The reader
does not apply the correction.

JEOL frequency axes commonly declare `x_offset` and `y_offset` in ppm rather
than Hz. When a typed same-axis observe frequency is present, the reader converts
that center to `transmitter_offset_hz` in the common frequency evidence; a native
Hz offset remains preferred. For supported `[3,3]` raw data, Delta's paired
indirect real/imaginary sections are exposed as two canonical `Complex` lanes.

## Varian details

Varian resolution requires the supported `phase=[1,2]` layout. Pulse-sequence and
`apptype` names never select component semantics. The explicit `f1coef` rule
maps the eight values to two rows of complex coefficients `(a + i*b)`, acting on
already conjugated direct samples. Both rows preserve the supplied signs; merely
providing coefficients does not imply an extra F1 conjugation. A missing or empty
`f1coef` retains the default ptype v1 identity matrix, equivalent to explicitly
supplying `1 0 0 0 0 0 1 0`. Inactive but nonempty coefficients remain explicit
source facts. When `sw` and `reffrq` are positive, the canonical
carrier is `(sw / 2 - rfl + rfp) / reffrq`. Missing `rfl` and `rfp` independently
default to zero, following the fixed OpenVnmrJ reference rule; invalid present
values are rejected. The indirect rule uses `sw1`, `rfl1`, `rfp1`, and
`reffrq1`. `tof`/`dof` remain transmitter
frequency evidence and are not substituted for the spectral reference offset.
Grouped arrays, alternate phase order, NUS phase pairs,
multiple arrays, and additional indirect dimensions fail with the stable
`VARIAN_UNSUPPORTED_COMPONENT_LAYOUT` code.

`ReadAssertions` is a narrow fallback for genuinely absent Varian facts. It
requires a direct encoding, a complete physical-to-canonical trace bijection,
and a checked component transform. The fallback is limited to one indirect
`ni` dimension with no array, NUS, extra indirect dimension, or source
`f1coef`. Explicit header, `ni`, `arraydim`, trace-count, or resolved transform
facts cannot be overridden; disagreement is `AssertionConflict`.
Nontrivial `ObservationOrdinal` modulation cannot accompany a reordered asserted
trace permutation because that combination does not establish one ordinal per
logical lane group; use absolute-grid modulation instead.

## Unsupported layouts

Nonrectangular Bruker processed component sets, JCAMP
LINK/NTUPLES/FID/peak tables, Bruker 3D NUS, 3D AQSEQ other than 0, PARMODE 3+,
other indirect modes, JEOL processed 3D+, JEOL raw 3D+, and ambiguous Varian
arrays are not supported. Partial header fields never upgrade a layout to
supported decoding.

## Related API

- `detect` identifies and selects
  raw or processed inputs without decoding all samples.
- `formats::bruker`,
  `formats::jeol`, and
  `formats::varian` expose vendor
  metadata and in-memory `Parts` decoders. `formats::jcamp_dx`
  provides the strict JCAMP-DX in-memory decoder.
- `raw::RawFormat` is a
  source/provenance identifier returned by detection; canonical descriptors do
  not depend on it.

Explicit JEOL record text can supply ordered parameter coordinates. Supported
list/ramp declarations such as `tau_interval => y_acq {1[ms], 5[s]}` retain
nonuniform SI values. Compatible time and gradient prefixes are converted per
entry; unknown units, conflicting declarations and count mismatches fail.
`Parameters::embedded_axes()` reports name, disk axis, list/ramp kind and
before/after-data record area. Ramp count must agree with the header within
1e-4 intervals; interpolation preserves stated endpoints when a printed step
is rounded. Raw record bytes remain available. This is experimental syntax
coverage, not certification of a JEOL acquisition.
