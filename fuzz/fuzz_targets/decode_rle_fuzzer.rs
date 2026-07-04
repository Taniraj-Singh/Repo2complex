#![no_main]
//! Fuzz target: the RLE block decoder.
//!
//! Feeds arbitrary bytes to the run-length decoder with a bounded output ceiling
//! and checks the same properties as the LZ target: no panic, and that any
//! decoded output re-encodes and decodes back to itself.

use libfuzzer_sys::fuzz_target;
use quiver::codec::rle;

const CAP: usize = 1 << 20;

fuzz_target!(|data: &[u8]| {
    if let Ok(output) = rle::decode(data, CAP) {
        let reencoded = rle::encode(&output);
        let again = rle::decode(&reencoded, CAP).expect("re-encoded stream must decode");
        assert_eq!(output, again, "RLE codec is not round-trip stable");
    }
});
