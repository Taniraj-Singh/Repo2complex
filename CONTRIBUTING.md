# Contributing to Quiver

Thanks for your interest in improving Quiver. This guide covers the developer
workflow and conventions.

## Setup

Quiver needs a recent stable Rust (1.70+) and has no external dependencies:

```sh
git clone https://github.com/quiver-archive/quiver
cd quiver
cargo build
cargo test
```

For fuzzing you additionally need nightly and `cargo-fuzz`
(see [docs/FUZZING.md](docs/FUZZING.md)).

## The checks CI runs

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps          # with RUSTDOCFLAGS=-D warnings
```

All four must pass before a PR is merged. `cargo test` includes the doctests in
the module documentation, so keep those examples working.

## Conventions

* **No `unsafe`.** The crate is `#![forbid(unsafe_code)]`.
* **No new dependencies** without discussion — the zero-dependency property is a
  feature.
* **Untrusted input is bounded.** Any new read path must respect
  [`util::Limits`] and must never allocate based on an unvalidated length.
* **Errors, not panics.** Public APIs return `Result`; `unwrap`/`expect` are only
  for invariants that hold by construction.
* **Document public items.** `missing_docs` is a warning; new public items need a
  doc comment that says *why*, not just what.
* **Keep the format spec in sync.** If you change the byte layout, update
  [docs/FORMAT.md](docs/FORMAT.md) in the same PR.

## Where things go

| You're changing…                | Look in…                              |
|---------------------------------|---------------------------------------|
| the on-disk layout              | `src/format` (and update `docs/FORMAT.md`) |
| a compression method            | `src/codec`                           |
| how archives are read           | `src/reader`                          |
| how archives are written        | `src/writer`                          |
| the directory/query views       | `src/vfs`, `src/catalog`              |
| verification                    | `src/integrity`                       |
| the façade                      | `src/pack`                            |

## Tests

* Add unit tests next to the code you change.
* Add an integration test in `tests/` if the change is observable through the
  public API.
* Fixing a bug? Add a regression test to `tests/regression.rs` describing the
  original symptom (follow the `rNNNN_...` naming).
* Touching a fuzzed surface? Consider a new seed in `fuzz/corpus/` or a new
  invariant in the target.

## Reporting security issues

If you can make a read path panic, hang, read out of bounds, or allocate
unboundedly on a crafted archive, that is a security issue — please report it
privately with the input that triggers it.
