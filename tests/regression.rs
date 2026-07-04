//! Regression tests pinning specific behaviours that were once wrong or are
//! easy to break. Each references the symptom it guards against.

use quiver::codec;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::writer::{Builder, CompressionPolicy};

#[test]
fn r0001_empty_file_entry_roundtrips() {
    // Symptom: a zero-length file used to produce no record / read as an error.
    let mut b = Builder::new();
    b.add_file("empty", Vec::new()).unwrap();
    let bytes = b.finish();
    let archive = Archive::open(&bytes).unwrap();
    assert_eq!(archive.read("empty").unwrap(), Vec::<u8>::new());
}

#[test]
fn r0002_lz_overlapping_match_expands_run() {
    // Symptom: LZ back-references with distance < length (used to encode runs)
    // must copy byte-by-byte so freshly written bytes feed the copy.
    let data = vec![b'Q'; 1000];
    let encoded = codec::encode(Compression::Lz, &data);
    assert_eq!(
        codec::decode(Compression::Lz, &encoded, 1 << 20).unwrap(),
        data
    );
}

#[test]
fn r0003_rle_run_spans_control_boundary() {
    // Symptom: runs longer than 128 must split across multiple control bytes.
    let data = vec![0x5Au8; 1000];
    let encoded = codec::encode(Compression::Rle, &data);
    assert_eq!(
        codec::decode(Compression::Rle, &encoded, 1 << 20).unwrap(),
        data
    );
}

#[test]
fn r0004_index_crc_detects_directory_corruption() {
    // Symptom: corrupting a name in the index must be caught by the index CRC at
    // open time, not silently accepted.
    let mut b = Builder::new();
    b.add_file("secret.txt", b"data".to_vec()).unwrap();
    let mut bytes = b.finish();
    // The name bytes live in the index region near the end (before the footer).
    let n = bytes.len();
    bytes[n - 12] ^= 0x01;
    assert!(Archive::open(&bytes).is_err());
}

#[test]
fn r0005_store_policy_never_transforms() {
    // Symptom: the None policy must store bytes verbatim (compression == Store).
    let data = vec![b'x'; 500];
    let mut b = Builder::new().with_policy(CompressionPolicy::None);
    b.add_file("f", data.clone()).unwrap();
    let bytes = b.finish();
    let archive = Archive::open(&bytes).unwrap();
    assert_eq!(archive.records()[0].compression, Compression::Store);
    assert_eq!(archive.read("f").unwrap(), data);
}

#[test]
fn r0006_zip_slip_paths_rejected() {
    // Symptom: entry paths escaping the root must be rejected at add time.
    let mut b = Builder::new();
    assert!(b.add_file("../../etc/passwd", b"x".to_vec()).is_err());
    assert!(b.add_file("/absolute", b"x".to_vec()).is_err());
    assert!(b.add_file("a/../../b", b"x".to_vec()).is_err());
}

#[test]
fn r0007_length_mismatch_is_detected() {
    // Symptom: if a block decodes to a different length than its record claims,
    // that must be an error, not a truncated read.
    let mut b = Builder::new();
    b.add_file("f", vec![1u8; 100]).unwrap();
    let bytes = b.finish();
    // Corrupt the block header's original_len (inside the first data block).
    // Header is 20 bytes; block header starts at 20; original_len is at +8..+12.
    let mut corrupt = bytes.clone();
    corrupt[20 + 8] ^= 0xff;
    // Opening still works (index intact); reading the entry must fail.
    let archive = Archive::open(&corrupt).unwrap();
    assert!(archive.read("f").is_err());
}

#[test]
fn r0008_duplicate_paths_rejected() {
    // Symptom: two entries with the same path must be rejected by the builder.
    let mut b = Builder::new();
    b.add_file("dup", b"1".to_vec()).unwrap();
    assert!(b.add_file("dup", b"2".to_vec()).is_err());
}
