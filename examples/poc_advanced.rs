//! Advanced POC: Deep-dive into edge cases, fuzzing targets, and adversarial scenarios.
//!
//! This example demonstrates:
//! - Edge case handling (boundaries, limits, special cases)
//! - Fuzzing-like adversarial inputs
//! - Memory efficiency under stress
//! - Complex recovery scenarios
//! - Performance edge cases
//!
//! Usage: `cargo run --example poc_advanced`

use quiver::codec::*;
use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::util::Limits;
use quiver::writer::Builder;

// ============================================================================
// SECTION 1: Codec Edge Cases
// ============================================================================

fn section_1_codec_edge_cases() {
    println!("\n=== SECTION 1: Codec Edge Cases ===");

    // Test 1: Empty input
    println!("\nTest 1a: Empty Store codec");
    match decode(Compression::Store, &[], 1024) {
        Ok(data) => println!("  ✓ Empty decoded: {} bytes", data.len()),
        Err(e) => println!("  Error: {}", e),
    }

    println!("Test 1b: Empty RLE codec");
    match decode(Compression::Rle, &[], 1024) {
        Ok(data) => println!("  ✓ Empty decoded: {} bytes", data.len()),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 2: Single byte
    println!("\nTest 2a: Single byte Store");
    match decode(Compression::Store, &[0x42], 1024) {
        Ok(data) => println!("  ✓ Decoded: {:02X}", data[0]),
        Err(e) => println!("  Error: {}", e),
    }

    // Test 3: Maximum repetition
    println!("\nTest 3: Maximum length single value");
    let mut data = Vec::new();
    for _chunk in (0..1_000_000).step_by(255) {
        data.push(255u8);
        data.push(0x42u8);
    }
    let encoded = encode(Compression::Rle, &data);
    println!("  ✓ RLE encoded: {} -> {} bytes", data.len(), encoded.len());
    match decode(Compression::Rle, &encoded, 2_000_000) {
        Ok(decoded) => {
            if decoded.len() == data.len() {
                println!("  ✓ Roundtrip successful");
            } else {
                println!("  ✗ Length mismatch: {} != {}", decoded.len(), data.len());
            }
        }
        Err(e) => println!("  ✗ Decode error: {}", e),
    }

    // Test 4: All-zeros (LZ back-ref heaven)
    println!("\nTest 4: Highly repetitive data (LZ compression)");
    let zeros = vec![0u8; 100_000];
    let lz_encoded = encode(Compression::Lz, &zeros);
    println!("  100KB of zeros -> {} bytes via LZ", lz_encoded.len());

    // Test 5: Alternating bytes
    println!("\nTest 5: Alternating pattern (challenging for compression)");
    let alternating: Vec<u8> = (0..10_000).map(|i| if i % 2 == 0 { 0xAA } else { 0x55 }).collect();
    for (method, label) in [(Compression::Store, "Store"), (Compression::Rle, "RLE"), (Compression::Lz, "LZ")] {
        let encoded = encode(method, &alternating);
        let ratio = (encoded.len() as f64 / alternating.len() as f64) * 100.0;
        println!("  {}: {} -> {} bytes ({:.1}%)", label, alternating.len(), encoded.len(), ratio);
    }
}

// ============================================================================
// SECTION 2: Archive Format Edge Cases
// ============================================================================

fn section_2_archive_edge_cases() {
    println!("\n=== SECTION 2: Archive Format Edge Cases ===");

    // Test 1: Single entry
    println!("\nTest 1: Minimal archive (1 entry)");
    let bytes = Pack::create([("x", b"y".to_vec())]).unwrap();
    let archive = Archive::open(&bytes).unwrap();
    println!("  ✓ Archive size: {} bytes, entries: {}", bytes.len(), archive.len());

    // Test 2: Many empty entries
    println!("\nTest 2: Archive with empty files");
    let mut builder = Builder::new();
    for i in 0..10 {
        builder.add_file(&format!("empty_{i}.txt"), Vec::new()).unwrap();
    }
    let bytes = builder.finish();
    let archive = Archive::open(&bytes).unwrap();
    println!("  ✓ 10 empty files: archive size {} bytes", bytes.len());

    // Test 3: Deep nesting
    println!("\nTest 3: Deep directory nesting");
    let mut builder = Builder::new();
    let mut path = String::new();
    for i in 0..20 {
        path.push_str(&format!("d{i}/"));
        builder.add_directory(&path).unwrap();
    }
    path.push_str("file.txt");
    builder.add_file(&path, b"deep".to_vec()).unwrap();
    let bytes = builder.finish();
    match Archive::open(&bytes) {
        Ok(a) => println!("  ✓ Deep nesting works: {} entries", a.len()),
        Err(e) => println!("  ✗ Failed: {}", e),
    }

    // Test 4: Very long filename
    println!("\nTest 4: Long filename");
    let mut builder = Builder::new();
    let long_name = "a".repeat(256);
    match builder.add_file(&long_name, b"data".to_vec()) {
        Ok(_) => {
            let _bytes = builder.finish();
            println!("  ✓ Accepted 256-char filename");
        }
        Err(e) => println!("  ✗ Rejected: {}", e),
    }

    // Test 5: Maximum entries (approach limit)
    println!("\nTest 5: Many entries (stress test)");
    let mut builder = Builder::new();
    let count = 5000;
    for i in 0..count {
        let path = format!("f{i:05}.bin");
        builder.add_file(&path, vec![i as u8; 10]).unwrap();
    }
    let bytes = builder.finish();
    let archive = Archive::open(&bytes).unwrap();
    println!("  ✓ Successfully built archive with {} entries ({} KB)", count, bytes.len() / 1024);

    // Test 6: Read via index vs. path
    println!("\nTest 6: Index vs. path access");
    let mut builder = Builder::new();
    let test_files = vec![("one.txt", b"1".to_vec()), ("two.txt", b"2".to_vec()), ("three.txt", b"3".to_vec())];
    for (name, data) in test_files {
        builder.add_file(name, data).unwrap();
    }
    let bytes = builder.finish();
    let archive = Archive::open(&bytes).unwrap();
    
    // Access first entry both ways
    let via_index = archive.read_index(0).unwrap();
    let via_path = archive.read("one.txt").unwrap();
    assert_eq!(via_index, via_path, "Index and path access should yield same data");
    println!("  ✓ Index and path access consistent");
}

// ============================================================================
// SECTION 3: Limits and Resilience
// ============================================================================

fn section_3_limits_and_resilience() {
    println!("\n=== SECTION 3: Limits and Resilience ===");

    // Build test archive
    let mut builder = Builder::new();
    for i in 0..100 {
        builder.add_file(&format!("file_{i:03}.bin"), vec![0xAB; 1_000]).unwrap();
    }
    let archive_bytes = builder.finish();

    // Test 1: Progressively stricter limits
    println!("\nTest 1: Progressive limit tightening");
    let limits_to_test = vec![
        ("Default", Limits::DEFAULT),
        ("Strict", Limits::STRICT),
        ("Ultra-strict", Limits {
            max_entries: 50,
            max_name_len: 50,
            max_total_decoded: 100_000,
            max_decoded_block: 100_000,
            max_path_depth: 5,
        }),
    ];

    for (name, limits) in limits_to_test {
        match Archive::open_with(&archive_bytes, limits) {
            Ok(a) => println!("  [{}] ✓ Opened: {} entries", name, a.len()),
            Err(e) => println!("  [{}] ✗ Rejected: {}", name, e.code()),
        }
    }

    // Test 2: Boundary conditions
    println!("\nTest 2: Entry count boundaries");
    for count in vec![1, 10, 100, 1_000] {
        let mut builder = Builder::new();
        for i in 0..count {
            builder.add_file(&format!("f{i}"), vec![i as u8; 10]).unwrap();
        }
        let bytes = builder.finish();
        let archive = Archive::open(&bytes).unwrap();
        println!("  {} entries: archive is {} bytes", count, bytes.len());
    }
}

// ============================================================================
// SECTION 4: Symlink Handling
// ============================================================================

fn section_4_symlink_handling() {
    println!("\n=== SECTION 4: Symlink and Entry Kind Handling ===");

    let mut builder = Builder::new();

    // Create various entry types
    builder.add_file("regular.txt", b"regular file".to_vec()).unwrap();
    builder.add_directory("dir1").unwrap();
    builder.add_symlink("link_to_file", "regular.txt").unwrap();
    builder.add_symlink("link_to_dir", "dir1").unwrap();
    builder.add_symlink("link_chain", "link_to_file").unwrap();

    let bytes = builder.finish();
    let archive = Archive::open(&bytes).unwrap();

    println!("Entry kinds in archive:");
    for rec in archive.records() {
        let kind_str = match rec.kind {
            quiver::types::EntryKind::File => "FILE",
            quiver::types::EntryKind::Directory => "DIR ",
            quiver::types::EntryKind::Symlink => "LINK",
        };
        let data_len = rec.original_len;
        println!("  [{}] {} : {} bytes", kind_str, rec.name, data_len);
    }

    // Read symlink targets
    println!("\nSymlink targets:");
    for rec in archive.records() {
        if rec.kind == quiver::types::EntryKind::Symlink {
            match archive.read(&rec.name) {
                Ok(data) => {
                    let target = String::from_utf8_lossy(&data);
                    println!("  {} -> {}", rec.name, target);
                }
                Err(e) => println!("  {} : error {}", rec.name, e),
            }
        }
    }
}

// ============================================================================
// SECTION 5: Recovery from Errors
// ============================================================================

fn section_5_error_recovery() {
    println!("\n=== SECTION 5: Error Recovery Patterns ===");

    // Test 1: Graceful fallback on bad entry
    println!("\nTest 1: Selective extraction with error handling");
    let mut builder = Builder::new();
    builder.add_file("good1.txt", b"data1".to_vec()).unwrap();
    builder.add_file("good2.txt", b"data2".to_vec()).unwrap();
    builder.add_file("good3.txt", b"data3".to_vec()).unwrap();
    let bytes = builder.finish();

    let archive = Archive::open(&bytes).unwrap();
    let mut successful = 0;
    let mut failed = 0;

    println!("Attempting to extract all entries:");
    for rec in archive.records() {
        match archive.read_record(rec) {
            Ok(data) => {
                successful += 1;
                println!("  ✓ {} ({} bytes)", rec.name, data.len());
            }
            Err(e) => {
                failed += 1;
                println!("  ✗ {}: {}", rec.name, e);
            }
        }
    }
    println!("Result: {}/{} entries extracted successfully", successful, successful + failed);

    // Test 2: Partial corruption recovery
    println!("\nTest 2: Archive with partial corruption");
    let mut archive_bytes = bytes.clone();
    
    // Try to corrupt middle of data section
    if archive_bytes.len() > 100 {
        archive_bytes[100] ^= 0xFF;
    }

    match Archive::open(&archive_bytes) {
        Ok(a) => {
            println!("  Archive opened despite corruption (may be in benign location)");
            let mut count = 0;
            for rec in a.records() {
                if a.read_record(rec).is_ok() {
                    count += 1;
                }
            }
            println!("  {} out of {} entries readable", count, a.len());
        }
        Err(e) => println!("  Archive rejected: {}", e),
    }

    // Test 3: Limit exceeded handling
    println!("\nTest 3: Handling limit exceeded");
    let impossible_limits = Limits {
        max_entries: 1,
        max_name_len: 5,
        max_total_decoded: 100,
        max_decoded_block: 100,
        max_path_depth: 2,
    };

    match Archive::open_with(&bytes, impossible_limits) {
        Ok(_) => println!("  Unexpectedly opened"),
        Err(e) => println!("  ✓ Correctly rejected: {}", e.code()),
    }
}

// ============================================================================
// SECTION 6: Performance Under Load
// ============================================================================

fn section_6_performance_load() {
    println!("\n=== SECTION 6: Performance Under Load ===");

    use std::time::Instant;

    // Test 1: Large single file
    println!("\nTest 1: Large single entry");
    let large_data = vec![0x42; 1_000_000]; // 1MB
    let mut builder = Builder::new();
    builder.add_file("large.bin", large_data).unwrap();

    let start = Instant::now();
    let bytes = builder.finish();
    let build_time = start.elapsed();

    let start = Instant::now();
    let archive = Archive::open(&bytes).unwrap();
    let parse_time = start.elapsed();

    let start = Instant::now();
    let extracted = archive.read("large.bin").unwrap();
    let read_time = start.elapsed();

    println!("  1MB file: build {:.2}ms, parse {:.2}ms, read {:.2}ms", 
        build_time.as_secs_f64() * 1000.0,
        parse_time.as_secs_f64() * 1000.0,
        read_time.as_secs_f64() * 1000.0);
    println!("  Archive overhead: {} bytes", bytes.len() - extracted.len());

    // Test 2: Many small files
    println!("\nTest 2: Many small entries");
    let mut builder = Builder::new();
    let count = 10_000;
    for i in 0..count {
        builder.add_file(&format!("f{i:05}"), vec![i as u8; 10]).unwrap();
    }

    let start = Instant::now();
    let bytes = builder.finish();
    let build_time = start.elapsed();

    let start = Instant::now();
    let archive = Archive::open(&bytes).unwrap();
    let parse_time = start.elapsed();

    println!("  {} entries: build {:.2}ms, parse {:.2}ms, size {} KB", 
        count,
        build_time.as_secs_f64() * 1000.0,
        parse_time.as_secs_f64() * 1000.0,
        bytes.len() / 1024);

    // Test 3: Random access patterns
    println!("\nTest 3: Random access patterns");
    let start = Instant::now();
    for i in (0..count).step_by(100) {
        let _ = archive.read(&format!("f{i:05}"));
    }
    let random_time = start.elapsed();
    println!("  {} random accesses: {:.2}ms ({:.2}μs per access)", 
        count / 100,
        random_time.as_secs_f64() * 1000.0,
        random_time.as_secs_f64() * 1_000_000.0 / (count as f64 / 100.0));
}

// ============================================================================
// SECTION 7: Data Pattern Analysis
// ============================================================================

fn section_7_data_patterns() {
    println!("\n=== SECTION 7: Data Pattern Analysis ===");

    let patterns = vec![
        ("All zeros", vec![0u8; 10_000]),
        ("All 0xFF", vec![0xFFu8; 10_000]),
        ("Incrementing", (0..10_000u32).map(|i| (i % 256) as u8).collect::<Vec<_>>()),
        ("Random-like", (0..10_000u32).map(|i| (i.wrapping_mul(1103515245).wrapping_add(12345) >> 16) as u8).collect::<Vec<_>>()),
        ("Sparse", {
            let mut v = vec![0u8; 9_000];
            v.extend_from_slice(&[0xAAu8; 1_000]);
            v
        }),
    ];

    println!("Compression effectiveness by pattern:");
    for (name, data) in patterns {
        let mut builder = Builder::new();
        builder.add_file("test", data.clone()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        let rec = archive.records().iter().find(|r| r.name == "test").unwrap();
        
        let ratio = (rec.stored_len as f64 / rec.original_len as f64) * 100.0;
        println!("  {:20} {:>7}→{:<7} ({:>5.1}%) [{}]", 
            name, 
            rec.original_len, 
            rec.stored_len, 
            ratio,
            rec.compression.label());
    }
}

// ============================================================================
// SECTION 8: Metadata Preservation
// ============================================================================

fn section_8_metadata() {
    println!("\n=== SECTION 8: Metadata Preservation ===");

    let mut builder = Builder::new();

    // Create entry with custom metadata
    let mut meta = quiver::types::EntryMeta::file("custom.txt".to_string());
    meta.mode = 0o755;
    meta.mtime = 1234567890;

    builder.add_file_with(meta.clone(), b"content".to_vec()).unwrap();

    let bytes = builder.finish();
    let archive = Archive::open(&bytes).unwrap();
    let rec = archive.records().iter().find(|r| r.name == "custom.txt").unwrap();

    println!("Metadata preservation:");
    println!("  Original mode: {:o}", meta.mode);
    println!("  Stored mode:   {:o}", rec.mode);
    println!("  Original mtime: {}", meta.mtime);
    println!("  Stored mtime:   {}", rec.mtime);
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   QUIVER POC: Advanced & Edge Cases                         ║");
    println!("║                                                              ║");
    println!("║   Deep exploration of challenging scenarios:                ║");
    println!("║   - Codec edge cases                                        ║");
    println!("║   - Archive format boundaries                               ║");
    println!("║   - Limits and resilience                                   ║");
    println!("║   - Symlink handling                                        ║");
    println!("║   - Error recovery                                          ║");
    println!("║   - Performance under load                                  ║");
    println!("║   - Data pattern analysis                                   ║");
    println!("║   - Metadata handling                                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    section_1_codec_edge_cases();
    section_2_archive_edge_cases();
    section_3_limits_and_resilience();
    section_4_symlink_handling();
    section_5_error_recovery();
    section_6_performance_load();
    section_7_data_patterns();
    section_8_metadata();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║   Advanced scenarios completed!                             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
