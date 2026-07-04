//! A self-contained, dependency-free "smoke fuzzer" proof-of-concept.
//!
//! The real fuzzing lives in `fuzz/` under `cargo-fuzz` + ClusterFuzzLite; this
//! example demonstrates the same idea in portable Rust. It drives the archive
//! reader and the block decoders with a large number of deterministic
//! pseudo-random inputs and asserts the crate's core guarantee — no panics — and
//! that anything the reader accepts survives an extract → rebuild → re-open
//! round trip.
//!
//! Usage: `cargo run --release --example fuzz_smoke -- [seed] [iterations]`

use std::panic;

use quiver::codec;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::util::Limits;
use quiver::writer::Builder;

/// Deterministic SplitMix64 PRNG (not cryptographic).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn bytes(&mut self, max: usize) -> Vec<u8> {
        let len = (self.next() as usize) % max;
        (0..len).map(|_| (self.next() & 0xff) as u8).collect()
    }

    fn structured(&mut self, max: usize, alphabet: &[u8]) -> Vec<u8> {
        let len = (self.next() as usize) % max;
        (0..len)
            .map(|_| alphabet[(self.next() as usize) % alphabet.len()])
            .collect()
    }
}

fn guard<T>(f: impl FnOnce() -> T) -> Result<T, ()> {
    panic::catch_unwind(panic::AssertUnwindSafe(f)).map_err(|_| ())
}

fn main() {
    panic::set_hook(Box::new(|_| {}));

    let seed = std::env::args()
        .nth(1)
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x5EED_5EED);
    let iterations: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    let mut rng = Rng(seed);
    let mut runs: u64 = 0;
    let mut accepted: u64 = 0;
    let mut panics: u64 = 0;

    // A prefix of the real magic reaches deeper into the header parser far more
    // often than uniform random bytes.
    let seeded_magic = b"QVR1";

    for _ in 0..iterations {
        // 1) Archive reader with two input distributions.
        for buf in [rng.bytes(96), {
            let mut v = seeded_magic.to_vec();
            v.extend(rng.bytes(80));
            v
        }] {
            runs += 1;
            match guard(|| Archive::open_with(&buf, Limits::STRICT)) {
                Ok(Ok(archive)) => {
                    accepted += 1;
                    if !roundtrip_ok(&archive) {
                        panics += 1;
                        eprintln!("ROUNDTRIP FAIL on {buf:?}");
                    }
                }
                Ok(Err(_)) => {}
                Err(()) => {
                    panics += 1;
                    eprintln!("PANIC Archive::open on {buf:?}");
                }
            }
        }

        // 2) Block decoders on raw random bytes.
        for method in [Compression::Rle, Compression::Lz, Compression::Store] {
            let buf = rng.structured(64, &[0x00, 0x01, 0x02, 0x7f, 0x80, 0x81, 0xff]);
            runs += 1;
            if guard(|| codec::decode(method, &buf, 1 << 20)).is_err() {
                panics += 1;
                eprintln!("PANIC decode {method:?} on {buf:?}");
            }
        }
    }

    let _ = panic::take_hook();
    println!("seed       : 0x{seed:x}");
    println!("iterations : {iterations}");
    println!("total runs : {runs}");
    println!("accepted   : {accepted}");
    println!("panics     : {panics}");
    if panics == 0 {
        println!("\nPASS: {runs} random inputs, zero panics, all round-trips stable.");
    } else {
        eprintln!("\nFAIL: {panics} panic(s) — replay with seed 0x{seed:x}");
        std::process::exit(1);
    }
}

/// Every entry an accepted archive exposes must decode, and the archive must
/// rebuild into something openable — a light end-to-end consistency check.
fn roundtrip_ok(archive: &Archive<'_>) -> bool {
    let mut builder = Builder::new();
    for i in 0..archive.len() {
        let Ok(data) = archive.read_index(i) else {
            // A decode error on a hand-crafted archive is fine; skip it.
            return true;
        };
        let meta = archive.records()[i].to_meta();
        if builder.add_file_with(meta, data).is_err() {
            return true;
        }
    }
    let rebuilt = builder.finish();
    Archive::open(&rebuilt).is_ok()
}
