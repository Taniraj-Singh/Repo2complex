# Quiver

A small, dependency-free Rust library for a **seekable binary archive container
format** — a minimal, well-specified `tar`/`zip` with its own block compression.
Build archives from named entries and read them back with random access,
streaming extraction, a virtual directory view, catalog queries, and
whole-archive integrity verification.

It has **zero runtime dependencies**, is `#![forbid(unsafe_code)]`, and every
path that reads an archive is bounded and total — malformed or hostile input
yields an error, never a panic, out-of-bounds read, hang, or unbounded
allocation. That makes it a natural fit for reading untrusted archives and for
continuous fuzzing.

```rust
use quiver::pack::Pack;

// Build an archive from named blobs (compression chosen automatically).
let bytes = Pack::create([
    ("readme.txt", b"hello".to_vec()),
    ("data/nums.bin", (0u8..200).collect::<Vec<_>>()),
]).unwrap();

// Open it and read an entry back.
let archive = Pack::open(&bytes).unwrap();
assert_eq!(archive.read("readme.txt").unwrap(), b"hello");

// Verify every entry's checksums.
let (_, report) = Pack::open_verified(&bytes).unwrap();
assert!(report.is_clean());
```

## The format

An archive is a fixed-layout header, a run of compressed data blocks, an index
of entry records, and a checksummed footer:

```text
  [FileHeader]        magic "QVR1", version, entry_count, index_offset
  [data blocks...]    each: BlockHeader ("QB") + stored bytes
  [index region]      entry_count EntryRecords (name, offsets, CRCs, mode, mtime)
  [footer]            index CRC + magic "QVRX"
```

All integers are fixed-width big-endian (no varints). Each data block and the
index carry a CRC-32, and each entry additionally records a CRC of its original
(decompressed) content, so corruption is caught at three levels. See
[docs/FORMAT.md](docs/FORMAT.md) for the byte-level specification.

## Compression

Three block codecs are built in, selected per entry:

| Method  | What it is                                             |
|---------|--------------------------------------------------------|
| `Store` | raw, uncompressed bytes                                |
| `Rle`   | run-length encoding (great for sparse/repetitive data) |
| `Lz`    | a bounded-window LZ77/LZSS compressor                  |

The `Auto` policy picks, per entry, whichever yields the smallest block and
never expands incompressible data. The decoders are the crate's main attack
surface and are written defensively — bounded output (no decompression bombs),
validated back-references, and no unchecked reads.

## Modules at a glance

| Module        | Responsibility                                            |
|---------------|-----------------------------------------------------------|
| `errors`      | the error taxonomy (`Format`, `Truncated`, `Codec`, ...)  |
| `util`        | bounds-checked byte I/O, checksums, limits, path safety   |
| `types`       | entry kinds, compression methods, entry metadata          |
| `format`      | the on-disk header, block header, record, index, footer   |
| `codec`       | the `Store`/`Rle`/`Lz` block codecs                       |
| `reader`      | `Archive` (random access) and `Extractor` (streaming)     |
| `writer`      | `Builder` and the compression policy                      |
| `vfs`         | a virtual directory `Tree` over the flat entry list       |
| `catalog`     | high-level queries: sizes, ratios, filters, largest       |
| `integrity`   | whole-archive verification (`quiver`'s `gzip -t`)         |
| `pack`        | the `Pack` façade tying it together                       |

## Safety on untrusted input

Reading is the trust boundary. Opening an archive validates the header, footer,
index CRC, and every entry's data range before any data is touched; decoding an
entry additionally checks the block CRC, the decoded length, and the content
CRC. Every quantity a hostile archive could inflate is bounded by a
[`util::Limits`] budget (entry count, name length, decompressed size, path
depth). No input can make the reader panic — it returns an [`errors::Error`],
and [`errors::Error::is_data_error`] distinguishes "reject this file" from "fix
your code".

## Fuzzing

Quiver ships five [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz)
targets and a [ClusterFuzzLite](https://google.github.io/clusterfuzzlite/)
configuration for continuous fuzzing:

| Target                 | Exercises                                        |
|------------------------|--------------------------------------------------|
| `open_archive_fuzzer`  | the archive parser + reading every entry         |
| `read_verify_fuzzer`   | open + whole-archive integrity verification      |
| `decode_lz_fuzzer`     | the LZ decoder + encode/decode round-trip        |
| `decode_rle_fuzzer`    | the RLE decoder + encode/decode round-trip       |
| `build_open_fuzzer`    | writer → reader pipeline consistency             |

```sh
cargo install cargo-fuzz
cargo fuzz run open_archive_fuzzer
```

The build is fully offline: the fuzz crate's dependencies are vendored under
[`vendor/`](vendor). See [docs/FUZZING.md](docs/FUZZING.md).

## Proof-of-concept demos

```sh
cargo run --example pack          # build a sample archive, show compression
cargo run --example list          # render the archive as a directory tree
cargo run --example hexdump       # annotated hex dump of the on-disk layout
cargo run --example hostile       # feed hostile inputs; show graceful rejection
cargo run --release --example fuzz_smoke -- 0xC0FFEE 200000   # in-process fuzzer
```

## Building

```sh
cargo build
cargo test
```

Requires Rust 1.70+ and has no external dependencies.

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how the layers fit together
- [docs/FORMAT.md](docs/FORMAT.md) — the binary format specification
- [docs/FUZZING.md](docs/FUZZING.md) — the fuzzing setup
- [CONTRIBUTING.md](CONTRIBUTING.md) — developer guide

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.

[`util::Limits`]: https://docs.rs/quiver/latest/quiver/util/struct.Limits.html
