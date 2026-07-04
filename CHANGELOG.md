# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project aims to follow [Semantic Versioning](https://semver.org/).

## [0.3.1]

### Added
- Random-access `Archive` reader with up-front validation of the header, footer,
  index CRC, and every entry's data range.
- `Builder` writer with `Store`/`Rle`/`Lz` block codecs and an `Auto`
  compression policy that never expands incompressible data.
- Virtual directory `Tree` (`vfs`), catalog queries (`catalog`), and
  whole-archive `integrity::verify`.
- Five `cargo-fuzz` targets and a ClusterFuzzLite configuration with vendored,
  fully-offline builds, seed corpora, and dictionaries.
- `Pack` façade and runnable PoC examples (`pack`, `list`, `hexdump`, `hostile`,
  `fuzz_smoke`).

### Security
- Entry paths are validated against traversal (`..`), absolute, and drive-letter
  forms at add time ("zip slip" defence).
- All decoders enforce a bounded output ceiling (decompression-bomb guard) and
  validate LZ back-references.

## [0.3.0]

### Added
- Initial format definition and the `format`/`codec`/`errors`/`util`
  foundation.
