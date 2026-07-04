# The Quiver archive format

This document specifies the on-disk byte layout precisely enough to write an
interoperable reader or writer. The reference implementation is [`src/format`],
with the fixed constants defined there.

## Conventions

* All integers are **fixed-width, big-endian**. There are no varints.
* Offsets and lengths are byte counts from the start of the file.
* Checksums are **CRC-32** (IEEE 802.3 polynomial, reflected), matching
  `gzip`/`zlib`.
* Strings are UTF-8; a reader rejects invalid UTF-8.

## Overall structure

```text
  +-----------------+  offset 0
  |   FileHeader    |  20 bytes
  +-----------------+  offset 20
  |   data blocks   |  each: BlockHeader (16 B) + stored bytes
  |       ...       |
  +-----------------+  offset = header.index_offset
  |  index region   |  entry_count EntryRecords
  +-----------------+  offset = file_len - 8
  |     footer      |  8 bytes
  +-----------------+  offset = file_len
```

Data blocks come first so that a writer can stream them out while accumulating
the index, then append the index and back-patch the header's `index_offset`.

## FileHeader (20 bytes)

| Field         | Type      | Notes                                  |
|---------------|-----------|----------------------------------------|
| magic         | `[u8; 4]` | must be `QVR1`                         |
| version       | `u8`      | must be `1`                            |
| flags         | `u8`      | reserved; must be `0`                  |
| reserved      | `u16`     | must be `0`                            |
| entry_count   | `u32`     | number of records in the index         |
| index_offset  | `u64`     | offset of the index region             |

A reader rejects a wrong magic (`BadMagic`), an unknown version
(`UnsupportedVersion`), a non-zero reserved field (`ReservedField`), and an
`index_offset` outside `[20, file_len - 8]` (`IndexOutOfRange`).

## Data block

Each entry that carries data is stored as a block header followed by
`stored_len` bytes of (possibly compressed) data.

### BlockHeader (16 bytes)

| Field         | Type      | Notes                                        |
|---------------|-----------|----------------------------------------------|
| magic         | `[u8; 2]` | must be `QB`                                 |
| method        | `u8`      | compression: 0=Store, 1=Rle, 2=Lz            |
| reserved      | `u8`      | must be `0`                                  |
| stored_len    | `u32`     | length of the stored bytes that follow       |
| original_len  | `u32`     | length after decompression                   |
| crc32         | `u32`     | CRC-32 of the **stored** bytes               |

The `crc32` covers the on-disk bytes, so a reader verifies block integrity
before decompressing. Directories and zero-length files have no block.

## EntryRecord (variable length)

The index is `entry_count` records written back-to-back.

| Field         | Type            | Notes                                    |
|---------------|-----------------|------------------------------------------|
| name_len      | `u16`           | length of the name in bytes              |
| name          | `[u8; name_len]`| UTF-8, normalized slash-separated path   |
| kind          | `u8`            | 0=File, 1=Directory, 2=Symlink           |
| compression   | `u8`            | matches the block's method               |
| data_offset   | `u64`           | offset of the block header (0 if no data)|
| stored_len    | `u64`           | stored data length                       |
| original_len  | `u64`           | original data length                     |
| crc32         | `u32`           | CRC-32 of the **original** content       |
| mode          | `u32`           | Unix permission bits                     |
| mtime         | `i64`           | seconds since the Unix epoch             |

A reader validates each record's data range: `data_offset >= 20` and
`data_offset + 16 + stored_len <= index_offset` (`EntryRangeOutOfRange`). Names
are bounded by `Limits::max_name_len`.

## Footer (8 bytes)

| Field       | Type      | Notes                                  |
|-------------|-----------|----------------------------------------|
| index_crc   | `u32`     | CRC-32 of the entire index region      |
| magic       | `[u8; 4]` | must be `QVRX`                         |

The `index_crc` lets a reader detect corruption of the directory before it
trusts any offsets within it.

## Compression codecs

### Store (0)

The stored bytes are the original bytes verbatim.

### Rle (1)

A control-byte run-length scheme:

* control `< 0x80` — a **literal run**: `control + 1` raw bytes follow (1..=128).
* control `>= 0x80` — a **repeat run**: `(control & 0x7f) + 1` copies (1..=128) of
  the single byte that follows.

### Lz (2)

An LZSS-style scheme. The stream is groups of up to 8 tokens behind a **flag
byte** (bits MSB→LSB):

* flag bit `1` — a **literal**: the next byte is copied.
* flag bit `0` — a **match**: the next two bytes are a big-endian `u16`
  `(distance-1) << 4 | (length - 3)`, giving distances 1..=4096 and lengths
  3..=18. The match copies `length` bytes from `distance` bytes back in the
  output (overlapping copies are valid and encode runs).

## Reader safety requirements

A conforming reader MUST, on any input:

1. Never read outside the file (bounds-check every access).
2. Validate `index_offset`, the footer magic, and the index CRC before parsing
   records.
3. Validate every record's data range before reading its block.
4. Verify the block CRC, the decoded length, and the content CRC when reading an
   entry.
5. Bound entry count, name length, decompressed block size, and path depth via
   `Limits`, so a small archive cannot expand into unbounded work or memory.
6. Terminate and return an error rather than panicking.

The reference reader enforces all of these; the fuzz targets under `fuzz/` keep
them honest.

[`src/format`]: https://github.com/quiver-archive/quiver/tree/main/src/format
