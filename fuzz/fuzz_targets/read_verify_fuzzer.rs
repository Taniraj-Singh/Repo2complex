#![no_main]
//! Fuzz target: open + whole-archive integrity verification.
//!
//! Drives `Archive::open_with` and then `integrity::verify`, which decodes every
//! entry and checks each block CRC, decoded length, and content CRC. This
//! reaches the codecs through the archive layer with all the format's integrity
//! checks engaged. The only invariant is "no panic" — verification legitimately
//! reports failures for corrupt data, but must always terminate gracefully.

use libfuzzer_sys::fuzz_target;
use quiver::integrity;
use quiver::reader::Archive;
use quiver::util::Limits;

fuzz_target!(|data: &[u8]| {
    if let Ok(archive) = Archive::open_with(data, Limits::STRICT) {
        let report = integrity::verify(&archive);
        // The report must account for exactly the archive's entries.
        assert_eq!(report.checked(), archive.len());
    }
});
