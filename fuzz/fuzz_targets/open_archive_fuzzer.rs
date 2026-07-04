#![no_main]
//! Fuzz target: the archive reader — the crate's principal attack surface.
//!
//! Feeds arbitrary bytes to `Archive::open_with` under strict limits and, for
//! any archive that opens, reads every entry. Invariants:
//!
//! 1. No panic, no out-of-bounds read, no unbounded allocation (enforced by the
//!    bounds-checked reader and the strict `Limits`).
//! 2. Reading an accepted entry either succeeds or returns an error — never
//!    crashes.

use libfuzzer_sys::fuzz_target;
use quiver::reader::Archive;
use quiver::util::Limits;

fuzz_target!(|data: &[u8]| {
    if let Ok(archive) = Archive::open_with(data, Limits::STRICT) {
        // Touch every entry; decode errors are fine, panics are not.
        for i in 0..archive.len() {
            let _ = archive.read_index(i);
        }
        // Metadata iteration must also be total.
        for meta in archive.entries() {
            let _ = meta.name();
        }
    }
});
