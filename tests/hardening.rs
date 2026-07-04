//! Hardening tests: the reader and codecs must be total over arbitrary input —
//! no panics, no hangs — returning errors for anything malformed.

use quiver::codec;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::util::Limits;
use quiver::writer::Builder;

fn valid_archive() -> Vec<u8> {
    let mut b = Builder::new();
    b.add_file("a", vec![9u8; 500]).unwrap();
    b.add_file("b/c", b"hello".to_vec()).unwrap();
    b.finish()
}

#[test]
fn open_rejects_garbage_without_panic() {
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        b"QVR1".to_vec(),
        b"QVR9".to_vec(),
        vec![0xff; 20],
        vec![0xff; 64],
        b"QVR1\x01\x00\x00\x00".to_vec(),
    ];
    for case in cases {
        assert!(
            Archive::open_with(&case, Limits::STRICT).is_err(),
            "expected error for {case:?}"
        );
    }
}

#[test]
fn every_single_byte_truncation_is_handled() {
    // Truncating a valid archive at every length must never panic.
    let valid = valid_archive();
    for cut in 0..valid.len() {
        let _ = Archive::open_with(&valid[..cut], Limits::STRICT);
    }
}

#[test]
fn every_single_byte_flip_is_handled() {
    // Flipping any one byte must either open (and read) cleanly or error — never
    // panic, and never read out of bounds.
    let valid = valid_archive();
    for i in 0..valid.len() {
        let mut corrupt = valid.clone();
        corrupt[i] ^= 0xff;
        if let Ok(archive) = Archive::open_with(&corrupt, Limits::STRICT) {
            for idx in 0..archive.len() {
                let _ = archive.read_index(idx);
            }
        }
    }
}

#[test]
fn codec_decoders_survive_byte_sweeps() {
    for method in [Compression::Store, Compression::Rle, Compression::Lz] {
        for a in 0u8..=255 {
            let _ = codec::decode(method, &[a], 1 << 16);
            let _ = codec::decode(method, &[a, a, a, a], 1 << 16);
            let _ = codec::decode(method, &[a, 0x00, 0xff, 0x80], 1 << 16);
        }
    }
}

#[test]
fn decompression_bombs_are_bounded() {
    // An RLE stream that would expand to ~16 MiB must be refused under a small cap.
    let mut bomb = Vec::new();
    for _ in 0..(16 * 1024) {
        bomb.push(0xFF); // repeat control: 128 copies
        bomb.push(0x00);
    }
    assert!(codec::decode(Compression::Rle, &bomb, 1 << 20).is_err());
}

#[test]
fn strict_limits_reject_huge_declared_counts() {
    // Craft a header claiming a huge entry count; opening must fail on the limit
    // rather than trying to allocate.
    let mut bytes = valid_archive();
    bytes[8..12].copy_from_slice(&u32::MAX.to_be_bytes()); // entry_count field
    assert!(Archive::open_with(&bytes, Limits::STRICT).is_err());
}
