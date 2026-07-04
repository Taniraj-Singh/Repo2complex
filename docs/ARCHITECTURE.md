# Architecture

How Quiver is put together: the layers, why they are separated, and how bytes
move through them. Aimed at contributors and at anyone auditing the
untrusted-input handling.

## Principles

1. **Total functions on untrusted input.** Every path that reads an archive
   terminates, never panics, and never reads out of bounds. Failure is a
   returned `Error`. Enforced by construction (a single bounds-checked
   `ByteReader`, `#![forbid(unsafe_code)]`, centralized `Limits`) and checked
   continuously by the fuzz targets.

2. **Bounded work.** A single [`util::Limits`] caps entry count, name length,
   decompressed block size, total decompressed size, and path depth. The safety
   envelope is auditable in one place.

3. **Three-level integrity.** Stored bytes, decoded length, and original content
   are each protected: a block CRC over the on-disk bytes, a length check, and a
   content CRC. The index has its own CRC in the footer.

4. **Layers usable in isolation.** The codecs, the format structs, the reader,
   and the writer are independent; `pack::Pack` composes them but nothing forces
   you through it.

5. **Zero dependencies.** Only `std`. Trivial supply chain, offline-friendly
   build, fast fuzzing.

## Layer map

```
                     ┌───────────────────────────┐
                     │        pack::Pack          │  façade
                     └───────────────────────────┘
        ┌──────────────┬───────────┬──────────────┬─────────────┐
        ▼              ▼           ▼              ▼             ▼
   ┌─────────┐   ┌──────────┐ ┌─────────┐  ┌──────────┐  ┌───────────┐
   │ writer  │   │  reader  │ │ catalog │  │   vfs    │  │ integrity │
   └─────────┘   └──────────┘ └─────────┘  └──────────┘  └───────────┘
        │              │
        ▼              ▼
   ┌─────────┐   ┌──────────┐
   │  codec  │   │  format  │
   └─────────┘   └──────────┘
        │              │
        └──────┬───────┘
               ▼
   ┌─────────────────────────────┐
   │  types · errors · util       │  foundation (no deps up)
   └─────────────────────────────┘
```

## Write path

```
entries ──▶ writer::Builder ──▶ codec::encode ──▶ format structs ──▶ bytes
```

`Builder` stages entries, validates each path (rejecting `..`, absolute, and
drive paths — the "zip slip" defence), and on `finish` performs a two-pass
layout: write each entry's compressed block (choosing the method per the
`CompressionPolicy`), remember its record, then append the index and footer and
back-patch the header's `index_offset`.

## Read path

```
bytes ──▶ reader::Archive::open ──▶ validated records
              │
              ▼ (per entry, on demand)
       format::BlockHeader ──▶ codec::decode ──▶ verified bytes
```

`Archive::open` validates the header, footer, index CRC, and every entry's data
range up front, so later reads operate on already-checked offsets. Reading an
entry parses its block header, verifies the stored CRC, decodes under a per-block
output ceiling, then checks the decoded length and content CRC. `Extractor`
walks entries sequentially for streaming extraction; `Catalog`, `vfs::Tree`, and
`integrity::verify` are read-only views layered on the parsed records.

## Where the limits bite

| Limit                 | Enforced in                                  |
|-----------------------|----------------------------------------------|
| `max_entries`         | `format::decode_index`                       |
| `max_name_len`        | `format::EntryRecord::read`                  |
| `max_decoded_block`   | `codec::decode` (bomb guard), reader         |
| `max_total_decoded`   | extraction callers                           |
| `max_path_depth`      | `vfs::Tree::build`                            |

The reader also validates every declared length against the actual file size
*before* allocating, so a header claiming a four-billion-byte block fails fast
instead of attempting the allocation.

## Testing strategy

* **Unit tests** beside each module cover its own invariants (codec round-trips,
  header parsing, range validation).
* **Integration tests** (`tests/`) drive the public API end to end, including
  every compression policy and a build→open→read round trip.
* **Hardening tests** (`tests/hardening.rs`) sweep truncations and single-byte
  flips of a valid archive and byte-sweep the decoders — the property is "never
  panics".
* **Regression tests** pin specific past bugs.
* **Fuzz targets** (`fuzz/`) assert the total-function and round-trip properties
  on arbitrary input, continuously.

See [FUZZING.md](FUZZING.md) and [FORMAT.md](FORMAT.md) for more.

[`util::Limits`]: https://docs.rs/quiver/latest/quiver/util/struct.Limits.html
