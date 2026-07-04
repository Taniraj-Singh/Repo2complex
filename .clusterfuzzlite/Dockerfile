# ClusterFuzzLite builder image for Quiver.
#
# Based on the OSS-Fuzz Rust base builder, which ships a pinned Rust toolchain,
# cargo-fuzz, and the sanitizer runtimes.
FROM gcr.io/oss-fuzz-base/base-builder-rust

COPY . $SRC/quiver
WORKDIR $SRC/quiver
COPY .clusterfuzzlite/build.sh $SRC/build.sh
