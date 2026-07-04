# Fuzzing

Quiver is built to be fuzzed. Reading an archive and decompressing a block are
the untrusted-input surfaces, and the fuzz targets assert not just "does not
crash" but also *semantic* invariants — codec round-trip stability and
writer/reader agreement — so subtle logic bugs surface as crashes too.

## Targets

Declared in [`fuzz/Cargo.toml`](../fuzz/Cargo.toml), sources under
[`fuzz/fuzz_targets/`](../fuzz/fuzz_targets):

| Target                 | Entry point                        | Extra invariant beyond "no crash"                 |
|------------------------|------------------------------------|---------------------------------------------------|
| `open_archive_fuzzer`  | `Archive::open_with` + read all    | reading any accepted entry never crashes          |
| `read_verify_fuzzer`   | open + `integrity::verify`         | the report accounts for exactly `archive.len()`   |
| `decode_lz_fuzzer`     | `codec::lz::decode`                | decoded output re-encodes and decodes back equal  |
| `decode_rle_fuzzer`    | `codec::rle::decode`               | decoded output re-encodes and decodes back equal  |
| `build_open_fuzzer`    | build from input → open → read     | every built entry reads back byte-for-byte        |

The "extra invariant" is what makes these valuable: a decoder that accepted an
output it could not reproduce, or a writer whose bytes the reader mis-parsed,
would be caught even though neither bug crashes on its own.

## Running locally

```sh
cargo install cargo-fuzz
rustup toolchain install nightly

cargo fuzz run open_archive_fuzzer
cargo fuzz run decode_lz_fuzzer -- -runs=5000000
cargo fuzz run decode_lz_fuzzer fuzz/artifacts/decode_lz_fuzzer/crash-<hash>   # reproduce
```

Seed corpora live under [`fuzz/corpus/`](../fuzz/corpus) and dictionaries under
[`.clusterfuzzlite/`](../.clusterfuzzlite); `cargo fuzz` picks them up.

## Seed corpus

Each target ships realistic, structured seeds (not empty files): valid archives
built by the writer (auto/store/empty) for the reader targets, and real encoded
blocks for the codec targets. The binary seeds are valid by construction, so they
exercise the success paths immediately as well as the rejection paths.

## Offline build

The fuzz crate's only external dependency, `libfuzzer-sys` (and its transitive
crates), is **vendored** under [`vendor/`](../vendor) and redirected through
[`.cargo/config.toml`](../.cargo/config.toml). Combined with
`CARGO_NET_OFFLINE=true` in [`build.sh`](../.clusterfuzzlite/build.sh), the
build needs **no network access** — a requirement of the CI harness. Regenerate
the vendored tree after a dependency change with
`cargo vendor --manifest-path fuzz/Cargo.toml vendor`.

## ClusterFuzzLite (CI)

The [`.clusterfuzzlite/`](../.clusterfuzzlite) directory holds the standard
configuration:

* `Dockerfile` — based on the OSS-Fuzz Rust builder image.
* `build.sh` — builds every target with `cargo fuzz build` (offline) and copies
  the binaries, zipped seed corpora, and dictionaries into `$OUT`.
* `project.yaml` — declares the language, engine (libFuzzer), and sanitizers
  (address, undefined).

The PR workflow
([`.github/workflows/cflite-pr.yml`](../.github/workflows/cflite-pr.yml)) builds
and runs the fuzzers for a few minutes against each code change.

## Triage tips

* Use the `hexdump` example to see a valid archive's layout, and `inspect`-style
  decoding via the `hostile` example to understand rejection paths.
* A crash in a round-trip/agreement assertion (not a library panic) points at an
  encoder/decoder or writer/reader *disagreement* — check both sides.
* Reproduce under `Limits::STRICT`, which the reader targets use, so behaviour
  matches.
