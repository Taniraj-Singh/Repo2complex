//! Advanced proof-of-concept: comprehensive, hard scenarios for Quiver.
//!
//! This POC demonstrates:
//! - Complex archive construction with mixed content types
//! - Error handling and recovery strategies
//! - Performance-critical operations
//! - Real-world use cases (backup/restore, container-like scenarios)
//! - Large-scale operations and memory efficiency
//! - Streaming and incremental access patterns
//! - Integrity verification at scale
//!
//! Usage: `cargo run --example poc_hard`

use quiver::catalog::Catalog;
use quiver::integrity;
use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::types::{Compression, EntryKind};
use quiver::util::Limits;
use quiver::writer::{Builder, CompressionPolicy};
use std::time::Instant;

// ============================================================================
// SCENARIO 1: Complex multi-type archive with mixed compression
// ============================================================================

fn scenario_1_mixed_content_archive() {
    println!("\n=== SCENARIO 1: Mixed Content with Intelligent Compression ===");

    let mut builder = Builder::new().with_policy(CompressionPolicy::Auto);

    // 1a. Add highly compressible text-based configs
    let configs = vec![
        ("config/app.toml", b"key = \"value\"\n".repeat(500).to_vec()),
        ("config/db.json", serde_json_like_content()),
        ("config/env.sh", shell_script_content()),
    ];

    for (path, data) in configs {
        builder.add_file(path, data).unwrap();
    }

    // 1b. Add directory structure
    builder.add_directory("src").unwrap();
    builder.add_directory("src/modules").unwrap();
    builder.add_directory("assets/images").unwrap();
    builder.add_directory("logs").unwrap();

    // 1c. Add source code (repetitive, compresses well)
    let rust_code = rust_source_code();
    for i in 0..5 {
        builder
            .add_file(&format!("src/modules/mod_{i}.rs"), rust_code.clone())
            .unwrap();
    }

    // 1d. Add binary data (random, won't compress)
    let binary_noise: Vec<u8> = (0..10_000u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();
    builder.add_file("assets/binary.bin", binary_noise).unwrap();

    // 1e. Add mostly-empty repetitive data (RLE heaven)
    let sparse_data: Vec<u8> = vec![0u8; 1000].into_iter().chain(vec![0xFFu8; 100]).collect();
    builder.add_file("assets/sparse.dat", sparse_data).unwrap();

    // 1f. Add symlinks
    builder
        .add_symlink("src/lib.rs", "src/modules/mod_0.rs")
        .unwrap();
    builder
        .add_symlink("latest_log", "logs/current.log")
        .unwrap();

    let archive_bytes = builder.finish();
    let archive = Archive::open(&archive_bytes).unwrap();
    let catalog = Catalog::new(&archive);
    let summary = catalog.summary();

    println!("Archive size: {} bytes", archive_bytes.len());
    println!("Entries: {} files, {} dirs, {} symlinks", summary.files, summary.directories, summary.symlinks);
    println!(
        "Compression ratio: {:.1}% ({} -> {} bytes)",
        summary.ratio() * 100.0,
        summary.original_bytes,
        summary.stored_bytes
    );

    // Show top 3 largest entries
    let mut entries: Vec<_> = archive.records().iter().collect();
    entries.sort_by_key(|r| std::cmp::Reverse(r.original_len));
    println!("\nTop 3 largest entries:");
    for (i, rec) in entries.iter().take(3).enumerate() {
        let ratio = if rec.stored_len == 0 {
            0.0
        } else {
            rec.stored_len as f64 / rec.original_len as f64
        };
        println!(
            "  {}: {} -> {} bytes ({:.1}% stored) [{}]",
            i + 1,
            rec.original_len,
            rec.stored_len,
            ratio * 100.0,
            rec.compression.label()
        );
    }
}

// ============================================================================
// SCENARIO 2: Large-scale archive with many entries
// ============================================================================

fn scenario_2_large_scale_archive() {
    println!("\n=== SCENARIO 2: Large-Scale Archive (1000+ entries) ===");

    let start = Instant::now();

    let mut builder = Builder::new();

    // Simulate a project structure with many files
    let num_dirs = 20;
    let num_files_per_dir = 100;

    // Add directory structure
    for d in 0..num_dirs {
        builder
            .add_directory(&format!("dir_{d}"))
            .unwrap();
    }

    // Add many files
    for d in 0..num_dirs {
        for f in 0..num_files_per_dir {
            let path = format!("dir_{d}/file_{f:04}.txt");
            let size = (f % 10 + 1) * 100; // Files of varying sizes
            let data = vec![((d + f) % 256) as u8; size];
            builder.add_file(&path, data).unwrap();
        }
    }

    let archive_bytes = builder.finish();
    let elapsed = start.elapsed();

    println!("Created archive with {} entries in {:.2}ms", num_dirs + (num_dirs * num_files_per_dir), elapsed.as_secs_f64() * 1000.0);
    println!("Archive size: {} KB", archive_bytes.len() / 1024);

    // Now read back selectively
    let start = Instant::now();
    let archive = Archive::open(&archive_bytes).unwrap();
    let read_elapsed = start.elapsed();

    println!("Parsed archive in {:.2}ms", read_elapsed.as_secs_f64() * 1000.0);
    println!("Total entries in archive: {}", archive.len());

    // Demonstrate random access efficiency
    let test_path = format!("dir_10/file_0050.txt");
    let start = Instant::now();
    let data = archive.read(&test_path).unwrap();
    let access_elapsed = start.elapsed();

    println!("Random access to '{}': {:.2}μs", test_path, access_elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Retrieved {} bytes", data.len());
}

// ============================================================================
// SCENARIO 3: Incremental extraction with streaming
// ============================================================================

fn scenario_3_streaming_extraction() {
    println!("\n=== SCENARIO 3: Streaming & Incremental Extraction ===");

    // Build archive
    let mut builder = Builder::new();
    for i in 0..5 {
        builder
            .add_file(&format!("chunk_{i}.dat"), vec![i as u8; 5_000])
            .unwrap();
    }
    let archive_bytes = builder.finish();

    // Open archive
    let archive = Archive::open(&archive_bytes).unwrap();

    println!("Archive has {} entries, processing sequentially:", archive.len());

    let mut total_extracted = 0u64;
    for rec in archive.records() {
        if rec.kind == EntryKind::File {
            match archive.read(&rec.name) {
                Ok(data) => {
                    total_extracted += data.len() as u64;
                    println!("  ✓ {} : {} bytes", rec.name, data.len());
                }
                Err(e) => {
                    println!("  ✗ {} : ERROR - {}", rec.name, e);
                }
            }
        }
    }

    println!("Total extracted: {} bytes", total_extracted);
}

// ============================================================================
// SCENARIO 4: Comprehensive integrity verification
// ============================================================================

fn scenario_4_integrity_verification() {
    println!("\n=== SCENARIO 4: Multi-Level Integrity Verification ===");

    // Build a good archive
    let mut builder = Builder::new();
    builder
        .add_file("readme.md", b"# Project\n\nThis is a test.".to_vec())
        .unwrap();
    builder
        .add_file("data.bin", vec![0xDEu8; 2_000])
        .unwrap();
    builder.add_directory("docs").unwrap();
    let good_bytes = builder.finish();

    // Verify the good archive
    let (_archive, report) = Pack::open_verified(&good_bytes).unwrap();
    println!("Good archive: {} entries checked", report.checked());
    println!("Is clean: {}", report.is_clean());

    // Now create corrupted variants and verify they're caught
    println!("\nTesting corruption detection:");

    // Corruption 1: Flip a bit in the data
    let mut bad_data = good_bytes.clone();
    if bad_data.len() > 100 {
        bad_data[100] ^= 0xFF;
    }

    match Pack::open_verified(&bad_data) {
        Ok((_, report)) => {
            println!("  Data corruption: {} entries ok", report.checked());
            println!("    Result: is_clean={}, failures={}", report.is_clean(), report.failures.len());
        }
        Err(e) => println!("  Data corruption caught: {}", e),
    }

    // Corruption 2: Truncate archive
    let truncated = &good_bytes[..good_bytes.len() / 2];
    match Pack::open(&truncated) {
        Ok(_) => println!("  Truncation: NOT DETECTED (unexpected)"),
        Err(e) => println!("  Truncation caught: {}", e),
    }
}

// ============================================================================
// SCENARIO 5: Custom limits and untrusted input handling
// ============================================================================

fn scenario_5_untrusted_input_handling() {
    println!("\n=== SCENARIO 5: Untrusted Input with Custom Limits ===");

    // Build a normal archive
    let mut builder = Builder::new();
    for i in 0..100 {
        builder
            .add_file(&format!("file_{i}.txt"), b"data".to_vec())
            .unwrap();
    }
    let archive_bytes = builder.finish();

    // Try opening with different limit strategies

    // 1. Very restrictive limits (security sandbox)
    println!("1. STRICT limits (security sandbox):");
    let strict_limits = Limits::STRICT;
    match Archive::open_with(&archive_bytes, strict_limits) {
        Ok(a) => println!("   ✓ Opened: {} entries", a.len()),
        Err(e) => println!("   ✗ Rejected: {}", e),
    }

    // 2. Default limits (recommended)
    println!("2. DEFAULT limits:");
    let default_limits = Limits::DEFAULT;
    match Archive::open_with(&archive_bytes, default_limits) {
        Ok(a) => println!("   ✓ Opened: {} entries", a.len()),
        Err(e) => println!("   ✗ Rejected: {}", e),
    }

    // 3. Custom limits (application-specific)
    println!("3. Custom limits (max 50 entries, 1MB per entry):");
    let custom_limits = Limits {
        max_entries: 50,
        max_name_len: 256,
        max_total_decoded: 1024 * 1024,
        max_decoded_block: 1024 * 1024,
        max_path_depth: 10,
    };
    match Archive::open_with(&archive_bytes, custom_limits) {
        Ok(a) => println!("   ✓ Opened: {} entries", a.len()),
        Err(e) => println!("   ✗ Exceeded limits: {}", e),
    }
}

// ============================================================================
// SCENARIO 6: Compression policy analysis
// ============================================================================

fn scenario_6_compression_analysis() {
    println!("\n=== SCENARIO 6: Compression Policy Analysis ===");

    let test_data: Vec<(&str, Vec<u8>)> = vec![
        ("Random", (0..5_000u32).map(|i| (i.wrapping_mul(1_103_515_245).wrapping_add(12345) >> 8) as u8).collect()),
        ("Repetitive", vec![b'A'; 5_000]),
        ("Text", b"The quick brown fox jumps over the lazy dog. ".repeat(114)),
        ("Binary runs", vec![0x00u8; 2_500].into_iter().chain(vec![0xFFu8; 2_500]).collect()),
    ];

    for policy in [CompressionPolicy::None, CompressionPolicy::Fixed(Compression::Rle), CompressionPolicy::Fixed(Compression::Lz), CompressionPolicy::Auto] {
        println!("\nPolicy: {:?}", policy);

        for (name, data) in &test_data {
            let mut builder = Builder::new().with_policy(policy);
            builder.add_file(&name.to_lowercase(), data.clone()).unwrap();
            let archive = builder.finish();

            let archive = Archive::open(&archive).unwrap();
            let rec = archive.records().iter().find(|r| r.name == name.to_lowercase()).unwrap();

            let ratio = (rec.stored_len as f64 / rec.original_len as f64) * 100.0;
            println!("  {:<15} {:>6}→{:<6} ({:>5.1}%) [{}]", name, rec.original_len, rec.stored_len, ratio, rec.compression.label());
        }
    }
}

// ============================================================================
// SCENARIO 7: VFS (Virtual File System) traversal
// ============================================================================

fn scenario_7_vfs_traversal() {
    println!("\n=== SCENARIO 7: Virtual Filesystem Traversal ===");

    // Build complex directory structure
    let mut builder = Builder::new();
    builder.add_directory("src").unwrap();
    builder.add_directory("src/core").unwrap();
    builder.add_directory("src/util").unwrap();
    builder.add_directory("tests").unwrap();
    builder.add_directory("docs").unwrap();
    builder.add_directory("assets/images").unwrap();

    builder.add_file("README.md", b"# Project".to_vec()).unwrap();
    builder.add_file("Cargo.toml", b"[package]".to_vec()).unwrap();
    builder.add_file("src/main.rs", b"fn main() {}".to_vec()).unwrap();
    builder.add_file("src/core/lib.rs", b"pub mod core;".to_vec()).unwrap();
    builder.add_file("src/util/math.rs", b"pub fn add(a, b) { a + b }".to_vec()).unwrap();
    builder.add_file("tests/integration.rs", b"#[test]".to_vec()).unwrap();
    builder.add_file("docs/ARCHITECTURE.md", b"## Architecture".to_vec()).unwrap();

    let archive_bytes = builder.finish();

    // Build VFS tree
    match Pack::tree(&archive_bytes) {
        Ok(tree) => {
            println!("Root contains:");
            if let Some(items) = tree.list("") {
                for item in items {
                    let node = tree.get(item).unwrap();
                    let kind = if node.is_dir { "[DIR]" } else { "[FILE]" };
                    println!("  {} {}", kind, item);
                }
            } else {
                println!("  (empty)");
            }

            println!("\nsrc/ contains:");
            if let Some(items) = tree.list("src") {
                for item in items {
                    let full_path = format!("src/{}", item);
                    let node = tree.get(&full_path).unwrap();
                    let kind = if node.is_dir { "[DIR]" } else { "[FILE]" };
                    println!("  {} {}", kind, item);
                }
            } else {
                println!("  (empty or not found)");
            }
        }
        Err(e) => println!("VFS error: {}", e),
    }
}

// ============================================================================
// SCENARIO 8: Backup/restore simulation
// ============================================================================

fn scenario_8_backup_restore() {
    println!("\n=== SCENARIO 8: Backup/Restore Workflow ===");

    println!("Phase 1: CREATE BACKUP");
    
    // Simulate backing up a project
    let mut backup = Builder::new().with_policy(CompressionPolicy::Auto);
    
    let backup_items = vec![
        ("project.toml", b"[package]\nname = \"myproj\"".to_vec()),
        ("source/main.rs", b"fn main() { println!(\"Hello\"); }".to_vec()),
        ("source/lib.rs", b"pub fn lib_fn() {}".to_vec()),
        ("data/config.json", serde_json_like_content()),
        ("data/logs.txt", b"2024-01-01: Started\n2024-01-02: Updated\n".repeat(100)),
    ];

    for (path, data) in backup_items.iter() {
        backup.add_file(path, data.clone()).unwrap();
    }

    let backup_bytes = backup.finish();
    println!("  Backup created: {} bytes (compressed)", backup_bytes.len());

    println!("\nPhase 2: VERIFY BACKUP");
    match Pack::open_verified(&backup_bytes) {
        Ok((archive, report)) => {
            println!("  ✓ Archive valid, {} entries verified", report.checked());
            println!("  All checksums clean: {}", report.is_clean());
        }
        Err(e) => println!("  ✗ Backup verification failed: {}", e),
    }

    println!("\nPhase 3: EXTRACT & RESTORE");
    match Pack::extract_all(&backup_bytes) {
        Ok(entries) => {
            println!("  Extracting {} items:", entries.len());
            for (meta, data) in entries {
                println!("    {} : {} bytes", meta.path, data.len());
            }
        }
        Err(e) => println!("  Restore failed: {}", e),
    }
}

// ============================================================================
// SCENARIO 9: Error resilience
// ============================================================================

fn scenario_9_error_resilience() {
    println!("\n=== SCENARIO 9: Error Handling & Resilience ===");

    // Build a valid archive
    let mut builder = Builder::new();
    builder.add_file("good.txt", b"valid data".to_vec()).unwrap();
    let archive_bytes = builder.finish();

    println!("Test 1: Reading non-existent entry");
    match Archive::open(&archive_bytes) {
        Ok(a) => match a.read("missing.txt") {
            Ok(_) => println!("  Unexpected success"),
            Err(e) => println!("  ✓ Correctly rejected: {}", e),
        },
        Err(e) => println!("  Archive open failed: {}", e),
    }

    println!("\nTest 2: Invalid path format");
    match Archive::open(&archive_bytes) {
        Ok(a) => match a.read("../escape") {
            Ok(_) => println!("  Unexpected success"),
            Err(e) => println!("  ✓ Correctly rejected: {}", e),
        },
        Err(e) => println!("  Archive open failed: {}", e),
    }

    println!("\nTest 3: Empty archive operations");
    let empty = Builder::new().finish();
    match Archive::open(&empty) {
        Ok(a) => {
            println!("  Archive is empty: {}", a.is_empty());
            println!("  Entry count: {}", a.len());
        }
        Err(e) => println!("  Error: {}", e),
    }
}

// ============================================================================
// SCENARIO 10: Performance benchmarking
// ============================================================================

fn scenario_10_performance() {
    println!("\n=== SCENARIO 10: Performance Characteristics ===");

    let sizes = vec![100, 500, 1000];

    for size in sizes {
        let mut builder = Builder::new();
        for i in 0..size {
            builder
                .add_file(&format!("f{i:05}.bin"), vec![0xAA; 100])
                .unwrap();
        }

        let start = Instant::now();
        let archive_bytes = builder.finish();
        let build_time = start.elapsed();

        let start = Instant::now();
        let archive = Archive::open(&archive_bytes).unwrap();
        let parse_time = start.elapsed();

        let start = Instant::now();
        let _ = integrity::verify(&archive);
        let verify_time = start.elapsed();

        println!(
            "\n{} entries: build {:.2}ms, parse {:.2}ms, verify {:.2}ms, total size {} KB",
            size,
            build_time.as_secs_f64() * 1000.0,
            parse_time.as_secs_f64() * 1000.0,
            verify_time.as_secs_f64() * 1000.0,
            archive_bytes.len() / 1024
        );
    }
}

// ============================================================================
// Helper functions for test data
// ============================================================================

fn serde_json_like_content() -> Vec<u8> {
    b"{\"name\": \"config\", \"version\": \"1.0\", \"settings\": {\"timeout\": 30, \"retries\": 3}}".repeat(10).to_vec()
}

fn shell_script_content() -> Vec<u8> {
    b"#!/bin/bash\nset -e\necho \"Running setup\"\nexport VAR=\"value\"\nmake build\nmake test\n".repeat(20).to_vec()
}

fn rust_source_code() -> Vec<u8> {
    b"use std::collections::HashMap;\n\npub struct Config {\n    settings: HashMap<String, String>,\n}\n\nimpl Config {\n    pub fn new() -> Self {\n        Config { settings: HashMap::new() }\n    }\n    pub fn get(&self, key: &str) -> Option<&String> {\n        self.settings.get(key)\n    }\n}\n".repeat(15).to_vec()
}

// ============================================================================
// Main entry point
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   QUIVER POC: Hard Scenarios & Advanced Use Cases           ║");
    println!("║                                                              ║");
    println!("║   Demonstrating comprehensive library capabilities:         ║");
    println!("║   - Complex archive construction                            ║");
    println!("║   - Large-scale operations                                  ║");
    println!("║   - Streaming & incremental access                          ║");
    println!("║   - Integrity verification                                  ║");
    println!("║   - Untrusted input handling                                ║");
    println!("║   - Performance characteristics                             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    scenario_1_mixed_content_archive();
    scenario_2_large_scale_archive();
    scenario_3_streaming_extraction();
    scenario_4_integrity_verification();
    scenario_5_untrusted_input_handling();
    scenario_6_compression_analysis();
    scenario_7_vfs_traversal();
    scenario_8_backup_restore();
    scenario_9_error_resilience();
    scenario_10_performance();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║   All scenarios completed successfully!                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
