# nmr

Rust library for reading supported NMR acquisitions and processed spectra,
processing them with explicit operations, and exporting plot data as NPZ.
It provides a common data model for supported Bruker, JEOL Delta, Varian and
JCAMP-DX layouts, including lazy raw trace and region access.

## Try it

Requires Rust 1.85 or newer and a working native Rust build toolchain. From a
source checkout:

```console
cargo run --locked --example read_spectrum -- tests/fixtures/jcamp_dx/ppm.dx target/first-spectrum.npz
```

This reads the bundled synthetic four-point spectrum, prints samples
`[2.0, 4.0, 6.0, 8.0]`, and exports it without processing. Use a new output filename
on each run: export refuses to overwrite files. The
[quickstart](docs/src/content/docs/getting-started.md) explains installation in
your own Rust application, expected results and next steps.

## Scope

Dense processing supports rank 1 and 2; it does not run during reading.
All JEOL imports and Bruker NUS require an explicit experimental-semantics opt-in.
Automatic phase, baseline and NUS methods have limited scientific validation
domains. Read the [format matrix](docs/src/content/docs/formats.md) and
[scientific contract](docs/src/content/docs/scientific-contract.md) before
relying on a particular layout or inference method.

The project is pre-release (`0.1.0` in this checkout). APIs and development
snapshots may change in place. The quickstart uses source installation rather
than assuming registry availability.

- [User guide](https://nmr.nmrtist.space/): reading, processing, limits and application integration.
- [Processing examples](docs/src/content/docs/processing.md): 1D, dense 2D and NUS workflows.
- API for this checkout: `cargo doc --no-deps --open`.
- [Contributing](CONTRIBUTING.md): development setup, architecture, tests and documentation.

## License

Licensed under either Apache-2.0 or MIT, at your option.
