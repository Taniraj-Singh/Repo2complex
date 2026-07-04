#![no_main]
//! Fuzz target: the LZ77 block decoder.
//!
//! The decoder is the security-critical half of the codec. This target feeds it
//! arbitrary bytes with a bounded output ceiling. Invariants:
//!
//! 1. No panic; back-reference underflows and truncation yield errors.
//! 2. **Re-encode stability**: any output the decoder produces must, when
//!    re-encoded and decoded again, reproduce the same bytes — proving the
//!    encoder and decoder agree on the format.

use libfuzzer_sys::fuzz_target;
use quiver::codec::lz;

const CAP: usize = 1 << 20;

fuzz_target!(|data: &[u8]| {
    if let Ok(output) = lz::decode(data, CAP) {
        let reencoded = lz::encode(&output);
        let again = lz::decode(&reencoded, CAP).expect("re-encoded stream must decode");
        assert_eq!(output, again, "LZ codec is not round-trip stable");
    }
});
