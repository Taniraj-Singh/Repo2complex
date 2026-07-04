//! Security proof-of-concept: feed the archive reader and the block codecs a
//! battery of hostile inputs and show that every one is rejected *gracefully* —
//! a located error, never a panic, out-of-bounds read, or unbounded allocation.
//!
//! Usage: `cargo run --example hostile`

use std::panic;

use quiver::codec;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::util::Limits;

fn probe<T>(f: impl FnOnce() -> Result<T, String>) -> (&'static str, String) {
    match panic::catch_unwind(panic::AssertUnwindSafe(f)) {
        Ok(Ok(_)) => ("ACCEPTED", String::new()),
        Ok(Err(msg)) => ("rejected", msg),
        Err(_) => ("PANIC!!", "library panicked — this is a bug".to_string()),
    }
}

fn archive_cases() -> Vec<(&'static str, Vec<u8>)> {
    // A valid archive we can then corrupt in targeted ways.
    let valid = {
        let mut b = quiver::writer::Builder::new();
        b.add_file("a.bin", vec![7u8; 400]).unwrap();
        b.finish()
    };

    let mut bad_magic = valid.clone();
    bad_magic[0] = b'X';

    let mut bad_index_crc = valid.clone();
    // Flip a byte inside the index region (near the end, before the footer).
    let n = bad_index_crc.len();
    bad_index_crc[n - 10] ^= 0xff;

    let mut giant_index_offset = valid.clone();
    // Header index_offset is the last 8 bytes of the 20-byte header.
    giant_index_offset[12..20].copy_from_slice(&u64::MAX.to_be_bytes());

    let mut corrupt_block = valid.clone();
    corrupt_block[40] ^= 0xff; // inside the data block

    vec![
        ("empty", vec![]),
        (
            "random 8 bytes",
            vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04],
        ),
        ("just the magic", b"QVR1".to_vec()),
        ("bad magic", bad_magic),
        ("giant index offset", giant_index_offset),
        ("corrupt index (crc)", bad_index_crc),
        ("corrupt data block", corrupt_block),
        ("truncated valid archive", valid[..valid.len() / 2].to_vec()),
    ]
}

fn codec_cases() -> Vec<(&'static str, Compression, Vec<u8>)> {
    vec![
        (
            "rle: literal claims 5, none follow",
            Compression::Rle,
            vec![0x04],
        ),
        ("rle: repeat with no value", Compression::Rle, vec![0x80]),
        (
            "lz: match backref underflow",
            Compression::Lz,
            vec![0x00, 0x00, 0x00],
        ),
        (
            "lz: truncated match token",
            Compression::Lz,
            vec![0x00, 0x12],
        ),
        ("lz: random bytes", Compression::Lz, vec![0xff; 32]),
        ("store: within cap", Compression::Store, vec![1, 2, 3]),
    ]
}

fn main() {
    let mut panics = 0usize;
    let mut total = 0usize;

    println!("== ARCHIVE READER (Archive::open, strict limits) ==");
    for (name, input) in archive_cases() {
        let (verdict, detail) = probe(|| {
            Archive::open_with(&input, Limits::STRICT)
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
        report(name, verdict, &detail, &mut panics);
        total += 1;
    }

    println!("\n== BLOCK CODECS (decode, 1 MiB output cap) ==");
    for (name, method, input) in codec_cases() {
        let (verdict, detail) = probe(|| {
            codec::decode(method, &input, 1 << 20)
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
        report(name, verdict, &detail, &mut panics);
        total += 1;
    }

    println!("\n{total} hostile inputs exercised, {panics} panics.");
    if panics == 0 {
        println!("PASS: every hostile input was handled gracefully.");
    } else {
        eprintln!("FAIL: {panics} panic(s).");
        std::process::exit(1);
    }
}

fn report(name: &str, verdict: &str, detail: &str, panics: &mut usize) {
    if verdict == "PANIC!!" {
        *panics += 1;
    }
    let detail = if detail.is_empty() {
        String::new()
    } else {
        format!("  ({})", truncate(detail, 58))
    };
    println!("  [{verdict:>8}] {name}{detail}");
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
