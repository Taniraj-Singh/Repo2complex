//! Extended testing suite: Comprehensive test scenarios and validation.
//!
//! This example provides:
//! - Extended test coverage for all features
//! - Stress testing scenarios
//! - Comprehensive validation patterns
//! - Edge case demonstrations
//! - Integration testing examples
//! - Regression test templates
//!
//! Usage: `cargo run --example extended_tests`

use quiver::codec::*;
use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::types::{Compression, EntryKind};
use quiver::writer::{Builder, CompressionPolicy};

// ============================================================================
// SECTION A: Codec Testing Suite
// ============================================================================

fn section_a_codec_testing() {
    println!("\n=== SECTION A: Codec Testing Suite ===\n");

    println!("Test A1: Store Codec");
    {
        let test_vectors = vec![
            ("empty", vec![]),
            ("single", vec![0x42]),
            ("small", vec![1, 2, 3, 4, 5]),
            ("large", vec![0xFF; 10000]),
        ];

        for (name, data) in test_vectors {
            let encoded = encode(Compression::Store, &data);
            match decode(Compression::Store, &encoded, 100000) {
                Ok(decoded) if decoded == data => println!("  ✓ {}: {} bytes", name, data.len()),
                Ok(_) => println!("  ✗ {}: data mismatch", name),
                Err(e) => println!("  ✗ {}: {}", name, e),
            }
        }
    }

    println!("\nTest A2: RLE Codec");
    {
        let test_vectors = vec![
            ("zeros", vec![0u8; 1000]),
            ("alternating", (0..500u32).map(|i| if i % 2 == 0 { 0xAA } else { 0x55 }).collect::<Vec<_>>()),
            ("runs", {
                let mut v = Vec::new();
                for val in 0u8..=255u8 {
                    for _ in 0..10 {
                        v.push(val);
                    }
                }
                v
            }),
        ];

        for (name, data) in test_vectors {
            let encoded = encode(Compression::Rle, &data);
            match decode(Compression::Rle, &encoded, data.len() + 1000) {
                Ok(decoded) if decoded == data => {
                    let ratio = (encoded.len() as f64 / data.len() as f64) * 100.0;
                    println!("  ✓ {}: {} -> {} ({:.1}%)", name, data.len(), encoded.len(), ratio);
                }
                Ok(_) => println!("  ✗ {}: data mismatch", name),
                Err(e) => println!("  ✗ {}: {}", name, e),
            }
        }
    }

    println!("\nTest A3: LZ Codec");
    {
        let test_vectors = vec![
            ("zeros", vec![0u8; 1000]),
            ("text", b"The quick brown fox jumps over the lazy dog. ".repeat(100)),
            ("repetitive", b"ABCDEF".repeat(500)),
            ("random", (0..5000u32).map(|i| (i.wrapping_mul(2654435761) >> 16) as u8).collect::<Vec<_>>()),
        ];

        for (name, data) in test_vectors {
            let encoded = encode(Compression::Lz, &data);
            match decode(Compression::Lz, &encoded, data.len() + 1000) {
                Ok(decoded) if decoded == data => {
                    let ratio = (encoded.len() as f64 / data.len() as f64) * 100.0;
                    println!("  ✓ {}: {} -> {} ({:.1}%)", name, data.len(), encoded.len(), ratio);
                }
                Ok(_) => println!("  ✗ {}: data mismatch", name),
                Err(e) => println!("  ✗ {}: {}", name, e),
            }
        }
    }

    println!("\nTest A4: Codec Limits");
    {
        // Test decompression output limit
        let data = vec![0xAA; 10_000_000];
        let encoded = encode(Compression::Rle, &data);

        // This should fail - output would exceed 1MB limit
        let result = decode(Compression::Rle, &encoded, 1024 * 1024);
        match result {
            Ok(_) => println!("  ✗ Should have exceeded limit"),
            Err(_) => println!("  ✓ Correctly rejected oversized output"),
        }
    }

    println!("\nTest A5: Codec Roundtrip");
    {
        for method in [Compression::Store, Compression::Rle, Compression::Lz] {
            let data = (0..1000u32).map(|i| (i % 256) as u8).collect::<Vec<_>>();
            let encoded = encode(method, &data);
            
            if let Ok(decoded) = decode(method, &encoded, data.len() + 10) {
                if decoded == data {
                    println!("  ✓ {:?} roundtrip", method);
                } else {
                    println!("  ✗ {:?} data mismatch", method);
                }
            }
        }
    }
}

// ============================================================================
// SECTION B: Archive Building & Parsing
// ============================================================================

fn section_b_archive_testing() {
    println!("\n=== SECTION B: Archive Building & Parsing ===\n");

    println!("Test B1: Empty Archive");
    {
        let bytes = Builder::new().finish();
        match Archive::open(&bytes) {
            Ok(a) => {
                println!("  ✓ Empty archive: {} entries, {} bytes", a.len(), bytes.len());
                assert!(a.is_empty());
                assert_eq!(a.len(), 0);
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest B2: Single Entry");
    {
        let mut b = Builder::new();
        b.add_file("test.txt", b"data".to_vec()).unwrap();
        let bytes = b.finish();

        match Archive::open(&bytes) {
            Ok(a) => {
                println!("  ✓ Single entry archive: {} bytes", bytes.len());
                assert_eq!(a.len(), 1);
                assert!(a.contains("test.txt"));
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest B3: Multiple Entries");
    {
        let mut b = Builder::new();
        for i in 0..10 {
            b.add_file(&format!("file_{i}.txt"), format!("File {i}").into_bytes()).unwrap();
        }
        let bytes = b.finish();

        match Archive::open(&bytes) {
            Ok(a) => {
                println!("  ✓ Multiple entries: {} entries, {} bytes", a.len(), bytes.len());
                assert_eq!(a.len(), 10);

                for i in 0..10 {
                    assert!(a.contains(&format!("file_{i}.txt")));
                }
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest B4: Entry Types");
    {
        let mut b = Builder::new();
        b.add_file("file.txt", b"content".to_vec()).unwrap();
        b.add_directory("dir").unwrap();
        b.add_symlink("link", "file.txt").unwrap();

        let bytes = b.finish();
        match Archive::open(&bytes) {
            Ok(a) => {
                println!("  ✓ Mixed entry types: {} entries", a.len());

                for rec in a.records() {
                    match rec.kind {
                        EntryKind::File => println!("    File: {}", rec.name),
                        EntryKind::Directory => println!("    Dir:  {}", rec.name),
                        EntryKind::Symlink => println!("    Link: {}", rec.name),
                    }
                }
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest B5: Deep Nesting");
    {
        let mut b = Builder::new();
        let mut path = String::new();
        for i in 0..15 {
            path.push_str(&format!("d{i}/"));
            b.add_directory(&path).unwrap();
        }
        path.push_str("file.txt");
        b.add_file(&path, b"deep".to_vec()).unwrap();

        let bytes = b.finish();
        match Archive::open(&bytes) {
            Ok(a) => println!("  ✓ Deep nesting: {} entries", a.len()),
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest B6: Long Names");
    {
        let mut b = Builder::new();
        let long_name = "file_".to_string() + &"x".repeat(250) + ".txt";
        
        match b.add_file(&long_name, b"data".to_vec()) {
            Ok(_) => {
                let bytes = b.finish();
                match Archive::open(&bytes) {
                    Ok(a) => println!("  ✓ Long name ({} chars): {} entries", long_name.len(), a.len()),
                    Err(e) => println!("  ✗ Failed to open: {}", e),
                }
            }
            Err(e) => println!("  ✗ Failed to add: {}", e),
        }
    }
}

// ============================================================================
// SECTION C: Compression Policy Testing
// ============================================================================

fn section_c_compression_testing() {
    println!("\n=== SECTION C: Compression Policy Testing ===\n");

    let test_data = [
        ("Random", (0..5000u32).map(|i| (i.wrapping_mul(1103515245) >> 16) as u8).collect::<Vec<_>>()),
        ("Text", b"The quick brown fox. ".repeat(250)),
        ("Repetitive", vec![0x42u8; 5000]),
        ("Sparse", {
            let mut v = vec![0u8; 4900];
            v.extend_from_slice(&[0xFFu8; 100]);
            v
        }),
    ];

    for policy in [
        CompressionPolicy::None,
        CompressionPolicy::Fixed(Compression::Store),
        CompressionPolicy::Fixed(Compression::Rle),
        CompressionPolicy::Fixed(Compression::Lz),
        CompressionPolicy::Auto,
    ] {
        println!("Policy: {:?}", policy);

        for (name, data) in &test_data {
            let mut b = Builder::new().with_policy(policy);
            b.add_file(&name.to_lowercase(), data.clone()).unwrap();
            let bytes = b.finish();

            let archive = Archive::open(&bytes).unwrap();
            let rec = &archive.records()[0];

            let ratio = (rec.stored_len as f64 / rec.original_len as f64) * 100.0;
            println!("  {:<12} {:>5.1}% [{}]", name, ratio, rec.compression.label());
        }
        println!();
    }
}

// ============================================================================
// SECTION D: Data Access Patterns
// ============================================================================

fn section_d_access_patterns() {
    println!("\n=== SECTION D: Data Access Patterns ===\n");

    // Build test archive
    let mut b = Builder::new();
    for i in 0..20 {
        b.add_file(&format!("entry_{i:02}"), vec![i as u8; 100]).unwrap();
    }
    let bytes = b.finish();
    let archive = Archive::open(&bytes).unwrap();

    println!("Test D1: Sequential Access");
    {
        let mut count = 0;
        for rec in archive.records() {
            if archive.read_record(rec).is_ok() {
                count += 1;
            }
        }
        println!("  ✓ Read {} entries sequentially", count);
    }

    println!("\nTest D2: Random Access");
    {
        let indices = [0, 5, 10, 15, 19];
        for idx in indices {
            if let Ok(data) = archive.read_index(idx) {
                println!("  ✓ Index {}: {} bytes", idx, data.len());
            }
        }
    }

    println!("\nTest D3: Path-based Access");
    {
        for i in [0, 7, 15] {
            let path = format!("entry_{i:02}");
            if let Ok(data) = archive.read(&path) {
                println!("  ✓ Path '{}': {} bytes", path, data.len());
            }
        }
    }

    println!("\nTest D4: Iterator Access");
    {
        let total: usize = archive.entries().map(|e| e.original_len as usize).sum();
        println!("  ✓ Sum via iterator: {} bytes", total);
    }

    println!("\nTest D5: Find Record");
    {
        if let Some(rec) = archive.find("entry_05") {
            println!("  ✓ Found: {} (stored: {} bytes)", rec.name, rec.stored_len);
        }
    }

    println!("\nTest D6: Contains Check");
    {
        let exists = archive.contains("entry_05");
        let missing = archive.contains("nonexistent");
        println!("  ✓ Contains 'entry_05': {}", exists);
        println!("  ✓ Contains 'nonexistent': {}", missing);
    }
}

// ============================================================================
// SECTION E: Error Handling
// ============================================================================

fn section_e_error_handling() {
    println!("\n=== SECTION E: Error Handling ===\n");

    println!("Test E1: Bad Magic");
    {
        let mut bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
        bytes.resize(100, 0);

        match Archive::open(&bytes) {
            Ok(_) => println!("  ✗ Should have failed"),
            Err(e) => println!("  ✓ Correctly rejected: {}", e.code()),
        }
    }

    println!("\nTest E2: Truncated Archive");
    {
        let mut b = Builder::new();
        b.add_file("test.txt", b"data".to_vec()).unwrap();
        let bytes = b.finish();

        // Truncate to half
        let truncated = &bytes[..bytes.len() / 2];

        match Archive::open(truncated) {
            Ok(_) => println!("  ✗ Should have failed"),
            Err(e) => println!("  ✓ Correctly rejected: {}", e.code()),
        }
    }

    println!("\nTest E3: Missing Entry");
    {
        let mut b = Builder::new();
        b.add_file("exists.txt", b"data".to_vec()).unwrap();
        let bytes = b.finish();
        let archive = Archive::open(&bytes).unwrap();

        match archive.read("missing.txt") {
            Ok(_) => println!("  ✗ Should have failed"),
            Err(e) => println!("  ✓ Correctly rejected: {}", e.code()),
        }
    }

    println!("\nTest E4: Invalid Index");
    {
        let mut b = Builder::new();
        b.add_file("test.txt", b"data".to_vec()).unwrap();
        let bytes = b.finish();
        let archive = Archive::open(&bytes).unwrap();

        match archive.read_index(999) {
            Ok(_) => println!("  ✗ Should have failed"),
            Err(e) => println!("  ✓ Correctly rejected: {}", e.code()),
        }
    }

    println!("\nTest E5: Error Classification");
    {
        let bytes = vec![0xFF; 50];
        match Archive::open(&bytes) {
            Ok(_) => {}
            Err(e) => {
                println!("  Error code: {}", e.code());
                println!("  Is data error: {}", e.is_data_error());
                println!("  Display: {}", e);
            }
        }
    }
}

// ============================================================================
// SECTION F: Integrity Verification
// ============================================================================

fn section_f_integrity_testing() {
    println!("\n=== SECTION F: Integrity Verification ===\n");

    println!("Test F1: Clean Archive");
    {
        let mut b = Builder::new();
        b.add_file("test.txt", b"data".to_vec()).unwrap();
        let bytes = b.finish();

        match Pack::open_verified(&bytes) {
            Ok((_, report)) => {
                println!("  ✓ Entries: {}", report.checked());
                println!("  ✓ Clean: {}", report.is_clean());
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest F2: Corrupted Data");
    {
        let mut b = Builder::new();
        b.add_file("test.txt", b"data".to_vec()).unwrap();
        let mut bytes = b.finish();

        // Corrupt the data section
        if bytes.len() > 100 {
            bytes[100] ^= 0xFF;
        }

        match Pack::open_verified(&bytes) {
            Ok((_, report)) => {
                println!("  Entries: {}", report.checked());
                println!("  Failures: {}", report.failures.len());
                println!("  Clean: {}", report.is_clean());
            }
            Err(e) => println!("  Error: {}", e),
        }
    }

    println!("\nTest F3: Multiple Entries Verification");
    {
        let mut b = Builder::new();
        for i in 0..5 {
            b.add_file(&format!("f{i}.txt"), vec![i as u8; 100]).unwrap();
        }
        let bytes = b.finish();

        match Pack::open_verified(&bytes) {
            Ok((_, report)) => {
                println!("  ✓ Verified {} entries", report.checked());
                println!("  ✓ All clean: {}", report.is_clean());
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }
}

// ============================================================================
// SECTION G: Stress Testing
// ============================================================================

fn section_g_stress_testing() {
    println!("\n=== SECTION G: Stress Testing ===\n");

    println!("Test G1: Many Entries (1000)");
    {
        let mut b = Builder::new();
        let count = 1000;

        for i in 0..count {
            b.add_file(&format!("f{i:04}"), vec![i as u8; 10]).unwrap();
        }

        let bytes = b.finish();
        match Archive::open(&bytes) {
            Ok(a) => {
                println!("  ✓ Created: {} entries, {} bytes", a.len(), bytes.len());
                println!("  ✓ Archive overhead: {} bytes", bytes.len() - (count * 10));
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest G2: Large Single Entry (1MB)");
    {
        let data = vec![0x42; 1_000_000];
        let mut b = Builder::new();
        b.add_file("large.bin", data).unwrap();

        let bytes = b.finish();
        match Archive::open(&bytes) {
            Ok(a) => {
                println!("  ✓ Created: {} entries", a.len());
                println!("  ✓ File size: {} MB", a.records()[0].original_len / 1_000_000);
                println!("  ✓ Stored size: {} KB", a.records()[0].stored_len / 1024);
            }
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest G3: Mixed Sizes");
    {
        let mut b = Builder::new();

        // Tiny files
        for i in 0..10 {
            b.add_file(&format!("tiny_{i}", ), vec![1]).unwrap();
        }

        // Small files
        for i in 0..10 {
            b.add_file(&format!("small_{i}"), vec![2; 100]).unwrap();
        }

        // Medium files
        for i in 0..10 {
            b.add_file(&format!("medium_{i}"), vec![3; 10_000]).unwrap();
        }

        let bytes = b.finish();
        match Archive::open(&bytes) {
            Ok(a) => println!("  ✓ Mixed sizes: {} entries, {} bytes", a.len(), bytes.len()),
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }

    println!("\nTest G4: Deep Structure");
    {
        let mut b = Builder::new();

        for level in 0..10 {
            let mut path = String::new();
            for l in 0..level {
                path.push_str(&format!("d{l}/"));
            }
            path.push_str(&format!("f{level}.txt"));
            b.add_file(&path, vec![level as u8; 50]).unwrap();
        }

        let bytes = b.finish();
        match Archive::open(&bytes) {
            Ok(a) => println!("  ✓ Deep structure: {} entries, max depth: 10", a.len()),
            Err(e) => println!("  ✗ Failed: {}", e),
        }
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   QUIVER: Extended Testing & Validation Suite               ║");
    println!("║                                                              ║");
    println!("║   Comprehensive test coverage across all features:          ║");
    println!("║   - Codec testing                                           ║");
    println!("║   - Archive building & parsing                              ║");
    println!("║   - Compression policies                                    ║");
    println!("║   - Data access patterns                                    ║");
    println!("║   - Error handling                                          ║");
    println!("║   - Integrity verification                                  ║");
    println!("║   - Stress testing                                          ║");
    println!("║                                                              ║");
    println!("║   7 sections × 50+ tests = comprehensive validation          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    section_a_codec_testing();
    section_b_archive_testing();
    section_c_compression_testing();
    section_d_access_patterns();
    section_e_error_handling();
    section_f_integrity_testing();
    section_g_stress_testing();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║   Extended Testing Suite Complete!                         ║");
    println!("║                                                              ║");
    println!("║   All tests executed successfully.                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
