---
title: Getting started
description: Build from source, read a bundled spectrum and verify an NPZ export.
---

## Prerequisites

Install Rust and Cargo 1.85 or newer, with the native linker required by your Rust
toolchain, and Git. Node.js is only needed to work on this documentation site.
The first build needs access to Cargo's dependency registry or a populated cache.
No instrument software, external dataset or Python package is required below.

## Read and export a known spectrum

```console
git clone https://github.com/nmrtist/nmr.git
cd nmr
cargo run --locked --example read_spectrum -- tests/fixtures/jcamp_dx/ppm.dx target/first-spectrum.npz
```

Run the command from the repository root. `read_spectrum` is a Cargo example,
not an installed `nmr` command. It reads the bundled synthetic JCAMP-DX fixture,
prepares scalar plot data and writes the NPZ file. It does not run an FFT or
change the already processed spectrum.

Check the output:

- Shape is `[4]`.
- Samples are `[2.0, 4.0, 6.0, 8.0]`.
- Axis 0 has physical coordinates `[4.0, 3.0, 2.0, 1.0]` in ppm.
- The last line reports `exported 4 samples` and the output path.

These exact values are also asserted in the reading tests. The fixture is
original synthetic data under the repository's MIT OR Apache-2.0 license.
The complete example is
[`examples/read_spectrum.rs`](https://github.com/nmrtist/nmr/blob/main/examples/read_spectrum.rs).

Export refuses to overwrite `target/first-spectrum.npz`. To run again, choose
another filename, or remove your previous generated file first. The parent
output directory must already exist; Cargo creates `target` during the build.

NPZ contains `data.npy`, `axis_0.npy` and a UTF-8 JSON manifest stored as the byte
array `manifest.npy`. If you already use Python with NumPy, you can independently
inspect it (optional):

```python
import numpy as np
with np.load("target/first-spectrum.npz", allow_pickle=False) as result:
    assert result["data"].tolist() == [2.0, 4.0, 6.0, 8.0]
    assert result["axis_0"].tolist() == [4.0, 3.0, 2.0, 1.0]
```

NPZ is a plot exchange format, not a restorable processing project. Use
[snapshots](/host-integration/#embedded-snapshot-contract) for offline continuation.

## Use the library in your application

This checkout is pre-release. To use exactly the source you just built, create a
sibling application from the repository root:

```console
cd ..
cargo new --bin nmr-start
cd nmr-start
```

Add this entry under the existing `[dependencies]` in `nmr-start/Cargo.toml`:

```toml
nmr = { path = "../nmr" }
```

Replace `src/main.rs` with:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = nmr::read("../nmr/tests/fixtures/jcamp_dx/ppm.dx")?;
    let spectrum = dataset.as_processed().ok_or("processed input required")?;
    assert_eq!(spectrum.data().samples(), &[2.0, 4.0, 6.0, 8.0]);
    println!("Read {} samples", spectrum.data().samples().len());
    Ok(())
}
```

Run `cargo run` from `nmr-start`; expect `Read 4 samples`. This relative input
path assumes that working directory. In your application, accept an explicit
input path. Keep the application's own `Cargo.lock` to record its dependency
resolution, and record the nmr source revision separately.

## Next steps

- [Reading data](/reading/): choose raw or processed input, inspect warnings and read raw regions lazily.
- [Processing and export](/processing/): transform your own raw 1D or dense 2D acquisition, or try synthetic NUS.
- [Format support](/formats/): check required companions and accepted layouts before importing.
- [Errors and resource limits](/errors-limits/): resolve selection, metadata, output and budget failures.
- [Contributing](/contributing/): build the API reference, run focused tests or edit this site.
