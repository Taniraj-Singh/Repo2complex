#!/bin/bash -eu
# ClusterFuzzLite build script for the Quiver fuzz targets.
#
# Runs inside the OSS-Fuzz / ClusterFuzzLite base builder image, which provides
# a Rust toolchain, cargo-fuzz, and the libFuzzer sanitizer runtime. It builds
# every fuzz target and copies the binaries and their seed corpora into $OUT.
#
# The build is fully offline: the fuzz crate's dependencies are vendored under
# `vendor/` and redirected via `.cargo/config.toml`, and CARGO_NET_OFFLINE is
# set below, so no network access is required or permitted.

set -euo pipefail

export CARGO_NET_OFFLINE=true

SANITIZER="${SANITIZER:-address}"
cargo fuzz build -O --sanitizer "$SANITIZER"

FUZZ_TARGET_DIR="$(pwd)/fuzz/target/x86_64-unknown-linux-gnu/release"

TARGETS=(
  open_archive_fuzzer
  read_verify_fuzzer
  decode_lz_fuzzer
  decode_rle_fuzzer
  build_open_fuzzer
)

for target in "${TARGETS[@]}"; do
  cp "${FUZZ_TARGET_DIR}/${target}" "${OUT}/${target}"

  corpus_dir="$(pwd)/fuzz/corpus/${target}"
  if [ -d "${corpus_dir}" ]; then
    (cd "${corpus_dir}" && zip -q -r "${OUT}/${target}_seed_corpus.zip" .)
  fi

  if [ -f "$(pwd)/.clusterfuzzlite/${target}.dict" ]; then
    cp "$(pwd)/.clusterfuzzlite/${target}.dict" "${OUT}/${target}.dict"
  fi
done

echo "Built ${#TARGETS[@]} fuzz targets into ${OUT}"
