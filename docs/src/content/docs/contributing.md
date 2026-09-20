---
title: Contributing
description: Set up development, find the relevant code and validate a contribution.
---

Bug reports, documentation improvements and focused pull requests are welcome.
Use the [issue tracker](https://github.com/nmrtist/nmr/issues) to report a problem
or discuss a large change; submit code through a
[pull request](https://github.com/nmrtist/nmr/pulls). Include a reproducer, expected
behavior, what changed, why, and the checks you ran or could not run.
Contributions use the repository's MIT OR Apache-2.0 license.

## Development setup

Clone the repository and run all Cargo commands below from its root. Rust 1.85
is the declared minimum; CI checks that version and stable Rust. Install the
native linker for your platform and the `rustfmt` and `clippy` components:

```console
rustup component add rustfmt clippy
cargo build --locked
cargo run --locked --example read_spectrum -- tests/fixtures/jcamp_dx/ppm.dx target/contributor-spectrum.npz
cargo doc --locked --no-deps --open
```

Use a new NPZ filename if it already exists. The generated API reference matches
your source checkout; hosted documentation may describe a different revision.
The tests create synthetic inputs locally and do not require private acquisitions
or vendor software. Dependency downloads require network access or a populated cache.

## Architecture

| Area | Responsibility |
| --- | --- |
| `src/reading`, `src/formats`, `src/io` | Candidate selection, vendor metadata and decoding, lazy raw I/O |
| `src/raw`, `src/processed`, `src/dataset.rs`, `src/axis.rs` | Checked representations, axes, components and aggregate context |
| `src/processing/contracts`, `prepare`, `engine`, `kernels`, `methods` | Validate requests/state, bound work, execute numerical kernels, estimate parameters |
| `src/provenance`, `src/derivation.rs`, `src/external.rs` | Source identity, library and external result boundaries |
| `src/execution.rs`, `src/resource.rs` | Synchronous cancellation/progress, work and memory budgets |
| `src/plot.rs`, `src/export`, `src/snapshot`, `src/execution_report` | Plot preparation, NPZ exchange, offline storage and evidence reports |

The ordinary path is read → `Dataset` → prepare a plan → execute → another
`Dataset` → plot/export. Readers normalize supported storage conventions but do
not process signals. Preparation resolves parameters and validates scientific
state; execution creates history. Keep aggregate context when adding a workflow.
The [data model](/data-model/), [processing contracts](/processing-reference/)
and [host integration](/host-integration/) explain these boundaries.

## Build and test

Run checks appropriate to your change:

```console
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --doc --locked
cargo doc --all-features --no-deps --locked
```

For a focused change, use `cargo test --test reading --locked`,
`cargo test --test processing --locked` or `cargo test --test workflows --locked`.
The [test guide](https://github.com/nmrtist/nmr/blob/main/tests/README.md) maps all
seven integration targets and explains fixture placement. Root `cargo test`
does not run the separate downstream application:

```console
cargo run --manifest-path tests/downstream/Cargo.toml --release --locked
cargo run --manifest-path tests/downstream/Cargo.toml --release --locked -- performance
```

The [CI workflow](https://github.com/nmrtist/nmr/blob/main/.github/workflows/ci.yml)
is the complete check list: it additionally covers release tests, downstream
formatting/lint, warning-free rustdoc, packaging, `cargo deny` and MSRV tests.
On PowerShell, make rustdoc warnings fail with `$env:RUSTDOCFLAGS = "-D warnings"`
before running `cargo doc`; in a POSIX shell, prefix that command with
`RUSTDOCFLAGS="-D warnings"`.

Scientific changes need analytic signals or independent reference evidence,
including failure cases and resource boundaries. Replay or a plausible plot is
not an independent numerical oracle. Preserve scientific tolerances when moving
tests; use the [scientific contract](/scientific-contract/) to locate the applicable
source rule and quality gates.

For reader changes, supply format evidence and a minimal redistributable fixture
where possible. Record its origin, license, expected samples and evidence gaps in
[`tests/fixtures/README.md`](https://github.com/nmrtist/nmr/blob/main/tests/fixtures/README.md).
Do not commit private data, credentials, machine-specific paths or generated output.
Change fixture expectations deliberately and validate them independently.

## Work on this site

The site uses Astro with Starlight, English Markdown pages, and a checked-in npm
lockfile. Node.js **22.12 or newer** is required. From the repository root:

```console
cd docs
npm ci
npm run dev
```

Open the local URL printed by Astro. Edit `src/content/docs/*.md`; titles and
descriptions live in frontmatter. `astro.config.mjs` defines navigation. When
renaming a page or heading, update navigation and all references. During
pre-release development, use the current names directly; do not retain obsolete
routes, alias anchors or compatibility redirects.

```console
npm run build
npm run preview
```

Build includes `starlight-links-validator` for internal routes and anchors.
External links and examples still need separate checks. Markdown code blocks
are not automatically compiled by this build; verify changed Rust examples with
Cargo. Inspect the homepage, sidebar, tables and code blocks at desktop and
narrow widths. There is currently one language, so no translation tree to sync.

Keep the README a short entry point and put maintained instructions in the site.
Keep user tasks separate from contracts and test evidence. Avoid recording local
delivery logs or private-data timings as current support claims.

## Pre-release changes

This project has not been released. APIs, algorithm identifiers and snapshot
schemas can evolve in place; development snapshots have no backward-compatibility
or migration guarantee. Document the current behavior and update affected
examples and tests when changing it. Preserve matching source when retaining
snapshots. A version tag or successful restore does not establish scientific
quality or cross-platform bitwise agreement.
