# Tests

Integration tests are organized by the behavior they assert. Each functional
directory has one `main.rs` entrypoint discovered by Cargo; nested directories
use ordinary `mod.rs` modules. Unit tests for private implementation details
remain beside the implementation in `src/`.

| Target | Ownership |
| --- | --- |
| `data_model` | Raw/processed structure, coordinates, chemical-shift references, identity, ownership and data access |
| `reading` | Format selection, Bruker/JEOL/Varian/JCAMP-DX readers, source provenance, layout fixtures and malformed inputs |
| `processing` | Processing methods, parameter/state contracts, scientific accuracy, preflight, history and replay |
| `snapshot` | Snapshot validation, current v1 wire fixtures, recorded environments and offline restoration |
| `execution` | Cancellation, progress, shared resource accounting and execution-report JSON |
| `output` | Plot-data construction, memory preflight and NPZ export |
| `workflows` | Complete application flows spanning several of the capabilities above |

Run all root-package tests, or focus on a target and optionally a module:

```console
cargo test --all-targets --all-features --locked
cargo test --test reading --locked
cargo test --test reading --locked bruker::
cargo test --test processing --locked phase::quality_gates::
cargo test --test snapshot --locked validation::
cargo test --test workflows --locked
```

Release mode runs the same scientific assertions:

```console
cargo test --release --all-targets --all-features --locked
```

[The contribution guide](../CONTRIBUTING.md) explains validation expectations for formatting, lint, documentation
and MSRV checks. Execution-report assertions run in the `execution` target.

## Placing a test

Choose the directory from the main assertion. A phase-accuracy test stays beside
phase methods even if it uses snapshot roundtrips; a snapshot-validation test
belongs in `snapshot/`. Use `workflows/` when the complete sequence across
components is the behavior under test.

Within readers, group by format and name modules after the behavior:
`processed.rs`, `parameters.rs`, `sampling.rs`, `provenance.rs`, or `fixtures.rs`.
Within processing, keep phase, baseline, NUS and spectrum methods together;
`accuracy.rs` and `quality.rs` contain their independent numerical expectations.
Property tests belong with the model or operation whose invariant they check.

Name files after a capability or scenario, without project-stage labels such as
`unified`, `infrastructure`, `consumer`, or `release_science`. Add the module
declaration when adding a file: Cargo does not recursively discover arbitrary
Rust files. Keep test cases in test modules; shared workflow runners may also
carry assertions exercised by integration tests. Preserve numerical tolerances
when moving scientific checks.

## Auxiliary code, fixed data and downstream acceptance

- `support/` contains small builders or workflows shared across test targets.
  Helpers used by one group stay in that group.
  Shared builder modules may have unused functions in an individual target.
- [`fixtures/`](fixtures/README.md) contains fixed bytes grouped by format,
  expected values, license information and evidence limitations. Use
  `support/fixture_paths.rs` for runtime paths and `include_bytes!` or
  `include_str!` for embedded inputs. Generated inputs belong in per-test
  temporary directories.
- [`downstream/`](downstream/README.md) is an independent application package
  with synchronous allocation and performance acceptance. Run both commands
  below explicitly; root-package `cargo test` does not execute them.

```console
cargo run --manifest-path tests/downstream/Cargo.toml --release --locked
cargo run --manifest-path tests/downstream/Cargo.toml --release --locked -- performance
```

Snapshot tests construct synthetic inputs at runtime and check roundtrips,
validation and offline restoration.
