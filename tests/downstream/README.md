# Downstream acceptance

This unpublished package exercises the public nmr API as an independent
application. It has its own manifest and lockfile and depends on the repository
through `nmr = { path = "../.." }`.

Run from the repository root:

```console
cargo run --manifest-path tests/downstream/Cargo.toml --release --locked
cargo run --manifest-path tests/downstream/Cargo.toml --release --locked -- performance
```

The default command first checks cancellation before NUS allocation, then runs
interactive processing, batch processing and snapshot-store workflows.
The `performance` command measures coordinate, component-plane and slice access,
and checks that scalar and multidimensional component iteration allocates nothing.
Elapsed timings are measurements, not fixed pass/fail thresholds.

`src/allocations.rs` owns the process-wide counting allocator,
`src/workflows.rs` contains application workflows, `src/performance.rs` contains
the retained performance checks, and `src/support.rs` constructs synthetic data.

Keep these checks synchronous in this separate executable: concurrent tests
would contaminate the global allocation counters. Root-package `cargo test`
does not run this package; CI explicitly runs both commands above.
