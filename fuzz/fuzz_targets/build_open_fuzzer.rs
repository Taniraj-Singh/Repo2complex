#![no_main]
//! Fuzz target: writer → reader pipeline consistency.
//!
//! Interprets the fuzz input as a series of entry payloads (each prefixed by a
//! one-byte length), builds an archive from them with the automatic compression
//! policy, opens the result, and asserts every entry reads back byte-for-byte.
//! This stitches the builder, all three codecs, and the reader together so that
//! a disagreement *between* stages — not just within one — surfaces as a crash.

use libfuzzer_sys::fuzz_target;
use quiver::reader::Archive;
use quiver::writer::Builder;

fuzz_target!(|data: &[u8]| {
    // Carve the input into (name, payload) entries.
    let mut builder = Builder::new();
    let mut expected: Vec<(String, Vec<u8>)> = Vec::new();
    let mut i = 0usize;
    let mut idx = 0usize;
    while i < data.len() && idx < 256 {
        let len = data[i] as usize;
        i += 1;
        let end = (i + len).min(data.len());
        let payload = data[i..end].to_vec();
        i = end;
        let name = format!("e{idx}");
        if builder.add_file(&name, payload.clone()).is_ok() {
            expected.push((name, payload));
        }
        idx += 1;
    }

    let bytes = builder.finish();

    // The freshly built archive must always open and read back exactly.
    let archive = Archive::open(&bytes).expect("self-built archive must open");
    assert_eq!(archive.len(), expected.len());
    for (name, payload) in &expected {
        let got = archive.read(name).expect("built entry must read back");
        assert_eq!(&got, payload, "entry {name} changed through the pipeline");
    }
});
