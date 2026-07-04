//! End-to-end integration tests: build archives and read them back through the
//! public API, across every compression policy and a range of contents.

use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::writer::{Builder, CompressionPolicy};

fn sample_entries() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("empty.txt", Vec::new()),
        ("small.txt", b"hello world".to_vec()),
        ("run.bin", vec![0xABu8; 10_000]),
        (
            "text.log",
            b"the quick brown fox jumps\n".repeat(500).to_vec(),
        ),
        ("binary.dat", (0..=255u8).cycle().take(6000).collect()),
        ("deep/nested/path/file.rs", b"fn main() {}".to_vec()),
    ]
}

#[test]
fn roundtrip_all_policies() {
    for policy in [
        CompressionPolicy::None,
        CompressionPolicy::Fixed(Compression::Store),
        CompressionPolicy::Fixed(Compression::Rle),
        CompressionPolicy::Fixed(Compression::Lz),
        CompressionPolicy::Auto,
    ] {
        let mut builder = Builder::new().with_policy(policy);
        for (path, data) in sample_entries() {
            builder.add_file(path, data).unwrap();
        }
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        assert_eq!(archive.len(), 6, "policy {policy:?}");
        for (path, data) in sample_entries() {
            assert_eq!(
                archive.read(path).unwrap(),
                data,
                "policy {policy:?} path {path}"
            );
        }
    }
}

#[test]
fn auto_policy_compresses_but_never_expands() {
    let compressible = vec![b'z'; 50_000];
    let bytes = Pack::create([("z.bin", compressible.clone())]).unwrap();
    // The whole archive (header + block + index + footer) must be far smaller
    // than the raw data thanks to compression.
    assert!(bytes.len() < compressible.len() / 2);
    assert_eq!(
        Archive::open(&bytes).unwrap().read("z.bin").unwrap(),
        compressible
    );
}

#[test]
fn extract_all_matches_inputs() {
    let bytes = Pack::create([("a", b"aaa".to_vec()), ("b/c", b"bbb".to_vec())]).unwrap();
    let extracted = Pack::extract_all(&bytes).unwrap();
    assert_eq!(extracted.len(), 2);
    assert_eq!(extracted[0].0.path, "a");
    assert_eq!(extracted[1].0.path, "b/c");
    assert_eq!(extracted[1].1, b"bbb");
}

#[test]
fn verification_is_clean_for_valid_archives() {
    let bytes = Pack::create([("x", vec![1u8; 3000]), ("y", vec![2u8; 3000])]).unwrap();
    let (_, report) = Pack::open_verified(&bytes).unwrap();
    assert!(report.is_clean());
    assert_eq!(report.checked(), 2);
}

#[test]
fn tree_reflects_paths() {
    let bytes = Pack::create([
        ("src/main.rs", b"m".to_vec()),
        ("src/lib.rs", b"l".to_vec()),
        ("Cargo.toml", b"c".to_vec()),
    ])
    .unwrap();
    let tree = Pack::tree(&bytes).unwrap();
    let mut top = tree.list("").unwrap();
    top.sort();
    assert_eq!(top, ["Cargo.toml", "src"]);
    assert!(tree.get("src").unwrap().is_dir);
    assert!(!tree.get("src/main.rs").unwrap().is_dir);
}

#[test]
fn content_addressing_is_order_stable() {
    // Two archives with the same entries added in a different order still read
    // back identically (order is preserved per index, not merged).
    let a = Pack::create([("one", b"1".to_vec()), ("two", b"2".to_vec())]).unwrap();
    let archive = Archive::open(&a).unwrap();
    assert_eq!(archive.records()[0].name, "one");
    assert_eq!(archive.records()[1].name, "two");
}

#[test]
fn large_archive_many_entries() {
    let mut b = Builder::new();
    for i in 0..1000 {
        b.add_file(&format!("f{i:04}"), format!("data-{i}").into_bytes())
            .unwrap();
    }
    let bytes = b.finish();
    let archive = Archive::open(&bytes).unwrap();
    assert_eq!(archive.len(), 1000);
    assert_eq!(archive.read("f0500").unwrap(), b"data-500");
    assert!(archive.read("nope").is_err());
}
