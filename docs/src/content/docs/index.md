---
title: nmr
description: Read and process supported NMR data in Rust.
---

`nmr` is a Rust library for reading NMR acquisitions and processed spectra,
processing data with explicit operations, and exporting scalar spectra as NPZ.
Use it when building a Rust analysis tool or application that needs a common
model for supported Bruker, JEOL Delta, Varian, and JCAMP-DX data. It is not a
graphical viewer; `PlotData` supplies samples and coordinates to your own display.

## Start here

[Follow the quickstart](/getting-started/) to build from source, read a bundled
four-point spectrum, check its values and export it. No instrument data or Python
installation is needed. Then [read your own data](/reading/) or
[process a raw acquisition](/processing/).

Before importing an experiment, check the [format support matrix](/formats/).
Support depends on the acquisition layout, not just the vendor or extension.

## Scope and current limits

| Task | Current capability |
| --- | --- |
| Read data | Supported raw Bruker, JEOL and Varian layouts; processed Bruker, JEOL and scalar 1D JCAMP-DX. Lazy trace/region access is available for raw data. |
| Process dense data | Explicit rank-1/rank-2 plans: windows, zero filling, FFT, component decoding, phase, reference and spectrum operations. Reading does not run processing. |
| Reconstruct NUS | Separate explicit-noise and automatic-noise adapters; dense plans reject sparse input. Repeated observations can be read but cannot be reconstructed by these adapters. |
| Save results | NPZ for plotting; execution reports for evidence; snapshots for offline continuation with the matching implementation. |

All JEOL reads and Bruker NUS require an **experimental vendor-semantics opt-in**.
Automatic phase, baseline and reconstruction methods have bounded validation
domains, not general recovery guarantees. See [scientific evidence](/scientific-contract/)
and the task-specific guides before using inferred results quantitatively.
Reading supported 3D Bruker data does not imply 3D processing support.

The source currently declares version `0.1.1` and is pre-release. APIs and
serialized development snapshots may change in place; preserve the matching
source revision when saving work. The quickstart uses a local dependency and
does not assume that this revision is published on a registry.

## Build an application or contribute

Use [host integration and snapshots](/host-integration/) for cancellation,
progress, external algorithms and offline storage. Use the
[data model](/data-model/) when working with axes and component tensors.

The [contribution guide](/contributing/) covers setup, architecture, focused
tests and documentation maintenance. Generate the API reference for your checkout
with `cargo doc --no-deps --open` from the repository root.
