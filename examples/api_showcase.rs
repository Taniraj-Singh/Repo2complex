//! Complete API showcase: Demonstrates all major APIs and common patterns.
//!
//! This example provides:
//! - Complete API surface coverage
//! - Common usage patterns
//! - Best practices
//! - Real-world examples
//! - Integration techniques
//!
//! Usage: `cargo run --example api_showcase`

use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::writer::{Builder, CompressionPolicy};

// ============================================================================
// SECTION 1: Basic API Usage
// ============================================================================

fn section_1_basic_api() {
    println!("\n=== SECTION 1: Basic API Usage ===\n");

    println!("Example 1.1: Create Archive");
    {
        let mut builder = Builder::new();
        builder.add_file("hello.txt", b"Hello, Quiver!".to_vec()).unwrap();
        builder.add_file("world.txt", b"World".to_vec()).unwrap();
        let bytes = builder.finish();
        println!("  Created archive: {} bytes", bytes.len());
    }

    println!("\nExample 1.2: Open Archive");
    {
        let mut builder = Builder::new();
        builder.add_file("data.txt", b"important data".to_vec()).unwrap();
        let bytes = builder.finish();

        match Archive::open(&bytes) {
            Ok(archive) => {
                println!("  Opened archive: {} entries", archive.len());
                for rec in archive.records() {
                    println!("    - {}: {} bytes (stored: {})", 
                        rec.name, rec.original_len, rec.stored_len);
                }
            }
            Err(e) => println!("  Error: {}", e),
        }
    }

    println!("\nExample 1.3: Read Entry");
    {
        let mut builder = Builder::new();
        builder.add_file("readme.txt", b"This is the readme file.".to_vec()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        match archive.read("readme.txt") {
            Ok(data) => println!("  Read {} bytes: {:?}", data.len(), String::from_utf8_lossy(&data)),
            Err(e) => println!("  Error: {}", e),
        }
    }

    println!("\nExample 1.4: Iterate Entries");
    {
        let mut builder = Builder::new();
        for i in 0..5 {
            builder.add_file(&format!("file_{i}"), vec![i as u8; 50]).unwrap();
        }
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        println!("  Entries:");
        for entry in archive.entries() {
            println!("    - {}: {} bytes", entry.path, entry.original_len);
        }
    }

    println!("\nExample 1.5: Access by Index");
    {
        let mut builder = Builder::new();
        builder.add_file("a.txt", b"data a".to_vec()).unwrap();
        builder.add_file("b.txt", b"data b".to_vec()).unwrap();
        builder.add_file("c.txt", b"data c".to_vec()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        for idx in 0..archive.len() {
            match archive.read_index(idx) {
                Ok(data) => println!("  Index {}: {} bytes", idx, data.len()),
                Err(e) => println!("  Index {} error: {}", idx, e),
            }
        }
    }
}

// ============================================================================
// SECTION 2: Compression
// ============================================================================

fn section_2_compression() {
    println!("\n=== SECTION 2: Compression ===\n");

    println!("Example 2.1: No Compression");
    {
        let data = b"uncompressed data".to_vec();
        let mut builder = Builder::new().with_policy(CompressionPolicy::None);
        builder.add_file("store.txt", data.clone()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        let rec = &archive.records()[0];
        println!("  Store: {} -> {} bytes ({:?})", rec.original_len, rec.stored_len, rec.compression);
    }

    println!("\nExample 2.2: RLE Compression");
    {
        let data = vec![0x42u8; 1000];
        let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(Compression::Rle));
        builder.add_file("rle.bin", data.clone()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        let rec = &archive.records()[0];
        let ratio = (rec.stored_len as f64 / rec.original_len as f64) * 100.0;
        println!("  RLE: {} -> {} bytes ({:.1}%, {:?})", rec.original_len, rec.stored_len, ratio, rec.compression);
    }

    println!("\nExample 2.3: LZ Compression");
    {
        let data = b"repeated text repeated text repeated text".repeat(25);
        let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(Compression::Lz));
        builder.add_file("lz.txt", data.clone()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        let rec = &archive.records()[0];
        let ratio = (rec.stored_len as f64 / rec.original_len as f64) * 100.0;
        println!("  LZ: {} -> {} bytes ({:.1}%, {:?})", rec.original_len, rec.stored_len, ratio, rec.compression);
    }

    println!("\nExample 2.4: Auto Compression");
    {
        let test_data = vec![
            ("random", (0..5000u32).map(|i| (i.wrapping_mul(1103515245) >> 16) as u8).collect::<Vec<_>>()),
            ("repetitive", vec![0xAB; 5000]),
            ("text", b"Hello world hello world".repeat(200)),
        ];

        for (name, data) in test_data {
            let mut builder = Builder::new().with_policy(CompressionPolicy::Auto);
            builder.add_file(name, data.clone()).unwrap();
            let bytes = builder.finish();

            let archive = Archive::open(&bytes).unwrap();
            let rec = &archive.records()[0];
            let ratio = (rec.stored_len as f64 / rec.original_len as f64) * 100.0;
            println!("  {:<12} {:.1}% [{:?}]", name, ratio, rec.compression);
        }
    }

    println!("\nExample 2.5: Policy Comparison");
    {
        let data = vec![0x42; 10000];

        for method in [Compression::Store, Compression::Rle, Compression::Lz] {
            let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(method));
            builder.add_file("data", data.clone()).unwrap();
            let bytes = builder.finish();

            println!("  {:?}: {} bytes archive", method, bytes.len());
        }
    }
}

// ============================================================================
// SECTION 3: Error Handling
// ============================================================================

fn section_3_error_handling() {
    println!("\n=== SECTION 3: Error Handling ===\n");

    println!("Example 3.1: Open Error");
    {
        let bad_data = vec![0xFF; 50];
        match Archive::open(&bad_data) {
            Ok(_) => println!("  Unexpectedly opened"),
            Err(e) => {
                println!("  Error code: {}", e.code());
                println!("  Display: {}", e);
                println!("  Is data error: {}", e.is_data_error());
            }
        }
    }

    println!("\nExample 3.2: Read Error");
    {
        let mut builder = Builder::new();
        builder.add_file("existing.txt", b"data".to_vec()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        match archive.read("nonexistent.txt") {
            Ok(_) => println!("  Unexpectedly read"),
            Err(e) => println!("  Not found: {}", e.code()),
        }
    }

    println!("\nExample 3.3: Index Error");
    {
        let mut builder = Builder::new();
        builder.add_file("only_one.txt", b"data".to_vec()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        match archive.read_index(999) {
            Ok(_) => println!("  Unexpectedly read"),
            Err(e) => println!("  Index out of bounds: {}", e.code()),
        }
    }

    println!("\nExample 3.4: Error Recovery");
    {
        let mut builder = Builder::new();
        builder.add_file("file1.txt", b"data1".to_vec()).unwrap();
        builder.add_file("file2.txt", b"data2".to_vec()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        let mut read_count = 0;
        let mut error_count = 0;

        for rec in archive.records() {
            match archive.read_record(rec) {
                Ok(_) => read_count += 1,
                Err(_) => error_count += 1,
            }
        }

        println!("  Read: {}, Errors: {}", read_count, error_count);
    }

    println!("\nExample 3.5: Entry Type Checking");
    {
        let mut builder = Builder::new();
        builder.add_file("file.txt", b"data".to_vec()).unwrap();
        builder.add_directory("dir").unwrap();
        builder.add_symlink("link", "file.txt").unwrap();

        let bytes = builder.finish();
        let archive = Archive::open(&bytes).unwrap();

        for rec in archive.records() {
            match rec.kind {
                quiver::types::EntryKind::File => println!("  File: {}", rec.name),
                quiver::types::EntryKind::Directory => println!("  Dir: {}", rec.name),
                quiver::types::EntryKind::Symlink => println!("  Link: {}", rec.name),
            }
        }
    }
}

// ============================================================================
// SECTION 4: Verification
// ============================================================================

fn section_4_verification() {
    println!("\n=== SECTION 4: Verification ===\n");

    println!("Example 4.1: Clean Archive Verification");
    {
        let mut builder = Builder::new();
        builder.add_file("data1.txt", b"data 1".to_vec()).unwrap();
        builder.add_file("data2.txt", b"data 2".to_vec()).unwrap();
        let bytes = builder.finish();

        match Pack::open_verified(&bytes) {
            Ok((archive, report)) => {
                println!("  Entries checked: {}", report.checked());
                println!("  Is clean: {}", report.is_clean());
                println!("  Archive has {} entries", archive.len());
            }
            Err(e) => println!("  Verification error: {}", e),
        }
    }

    println!("\nExample 4.2: Corrupted Archive");
    {
        let mut builder = Builder::new();
        builder.add_file("data.txt", b"important".to_vec()).unwrap();
        let mut bytes = builder.finish();

        if bytes.len() > 100 {
            bytes[100] ^= 0xFF;
        }

        match Pack::open_verified(&bytes) {
            Ok((_, report)) => {
                println!("  Entries checked: {}", report.checked());
                println!("  Is clean: {}", report.is_clean());
                println!("  Failures: {}", report.failures.len());
            }
            Err(e) => println!("  Error: {}", e),
        }
    }

    println!("\nExample 4.3: Partial Verification");
    {
        let mut builder = Builder::new();
        for i in 0..10 {
            builder.add_file(&format!("f{i}"), vec![i as u8; 100]).unwrap();
        }
        let bytes = builder.finish();

        match Pack::open_verified(&bytes) {
            Ok((_, report)) => {
                println!("  Verified {} entries", report.checked());
                println!("  All good: {}", report.is_clean());
            }
            Err(e) => println!("  Error: {}", e),
        }
    }

    println!("\nExample 4.4: Selective Opening");
    {
        let mut builder = Builder::new();
        builder.add_file("safe.txt", b"safe data".to_vec()).unwrap();
        let bytes = builder.finish();

        // Try with strict limits
        match Archive::open_with(&bytes, quiver::util::Limits::STRICT) {
            Ok(archive) => println!("  Opened with STRICT: {} entries", archive.len()),
            Err(_) => {
                // Fall back to default
                match Archive::open(&bytes) {
                    Ok(archive) => println!("  Opened with DEFAULT: {} entries", archive.len()),
                    Err(e) => println!("  Error: {}", e),
                }
            }
        }
    }
}

// ============================================================================
// SECTION 5: Real-World Patterns
// ============================================================================

fn section_5_real_world() {
    println!("\n=== SECTION 5: Real-World Patterns ===\n");

    println!("Pattern 1: File Backup");
    {
        println!("  Creating backup");
        let mut backup = Builder::new().with_policy(CompressionPolicy::Auto);
        backup.add_file("config.toml", b"[app]\nversion=1.0".to_vec()).unwrap();
        backup.add_file("data.db", vec![0xAB; 500]).unwrap();
        let bytes = backup.finish();

        println!("  Verifying backup");
        match Pack::open_verified(&bytes) {
            Ok((_, report)) => println!("  ✓ Backup verified: {} entries", report.checked()),
            Err(e) => println!("  ✗ Backup failed: {}", e),
        }
    }

    println!("\nPattern 2: Configuration Package");
    {
        let mut config = Builder::new();
        config.add_file("env/prod.toml", b"env=production".to_vec()).unwrap();
        config.add_file("env/dev.toml", b"env=development".to_vec()).unwrap();
        config.add_file("app.conf", b"debug=false".to_vec()).unwrap();
        let bytes = config.finish();

        let archive = Archive::open(&bytes).unwrap();
        println!("  Package contains {} configs", archive.len());
    }

    println!("\nPattern 3: Data Archive");
    {
        let mut archive_builder = Builder::new();
        for i in 0..5 {
            let data = vec![(i * 42) as u8; 1000];
            archive_builder.add_file(&format!("dataset_{i}"), data).unwrap();
        }
        let bytes = archive_builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        let total: u64 = archive.entries().map(|e| e.original_len as u64).sum();
        println!("  Archive: {} entries, {} KB total", archive.len(), total / 1024);
    }

    println!("\nPattern 4: Bulk Processing");
    {
        let mut builder = Builder::new();
        for i in 0..100 {
            builder.add_file(&format!("item_{i:03}"), vec![i as u8; 50]).unwrap();
        }
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        let mut count = 0;
        for rec in archive.records() {
            if let Ok(_) = archive.read_record(rec) {
                count += 1;
            }
        }
        println!("  Processed {} items", count);
    }

    println!("\nPattern 5: Filtered Reading");
    {
        let mut builder = Builder::new();
        builder.add_file("readme.txt", b"readme".to_vec()).unwrap();
        builder.add_file("data.json", b"{{}}".to_vec()).unwrap();
        builder.add_file("config.toml", b"[config]".to_vec()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();
        println!("  Files matching *.txt:");
        for entry in archive.entries() {
            if entry.path.ends_with(".txt") {
                println!("    - {}", entry.path);
            }
        }
    }
}

// ============================================================================
// SECTION 6: Performance
// ============================================================================

fn section_6_performance() {
    println!("\n=== SECTION 6: Performance ===\n");

    println!("Benchmark 1: Archive Creation");
    {
        let start = std::time::Instant::now();
        let mut builder = Builder::new();
        for i in 0..1000 {
            builder.add_file(&format!("file_{i:04}"), vec![i as u8; 50]).unwrap();
        }
        let bytes = builder.finish();
        let elapsed = start.elapsed();

        println!("  Created 1000 entries in {:.2}ms", elapsed.as_secs_f64() * 1000.0);
        println!("  Archive size: {} KB", bytes.len() / 1024);
    }

    println!("\nBenchmark 2: Archive Opening");
    {
        let mut builder = Builder::new();
        for i in 0..500 {
            builder.add_file(&format!("f{i:03}"), vec![i as u8; 100]).unwrap();
        }
        let bytes = builder.finish();

        let start = std::time::Instant::now();
        let archive = Archive::open(&bytes).unwrap();
        let elapsed = start.elapsed();

        println!("  Opened {} entries in {:.2}µs", archive.len(), elapsed.as_secs_f64() * 1_000_000.0);
    }

    println!("\nBenchmark 3: Entry Lookup");
    {
        let mut builder = Builder::new();
        for i in 0..100 {
            builder.add_file(&format!("entry_{i:03}"), vec![i as u8; 200]).unwrap();
        }
        let bytes = builder.finish();
        let archive = Archive::open(&bytes).unwrap();

        let start = std::time::Instant::now();
        let _data = archive.read("entry_050").unwrap();
        let elapsed = start.elapsed();

        println!("  Lookup latency: {:.2}µs", elapsed.as_secs_f64() * 1_000_000.0);
    }

    println!("\nBenchmark 4: Sequential Read");
    {
        let mut builder = Builder::new();
        for i in 0..100 {
            builder.add_file(&format!("f{i:03}"), vec![i as u8; 1000]).unwrap();
        }
        let bytes = builder.finish();
        let archive = Archive::open(&bytes).unwrap();

        let start = std::time::Instant::now();
        let mut count = 0;
        for rec in archive.records() {
            if let Ok(_) = archive.read_record(rec) {
                count += 1;
            }
        }
        let elapsed = start.elapsed();

        println!("  Read {} entries in {:.2}ms", count, elapsed.as_secs_f64() * 1000.0);
    }

    println!("\nBenchmark 5: Compression Efficiency");
    {
        let data = vec![0x42u8; 100_000];
        
        for method in [Compression::Store, Compression::Rle, Compression::Lz] {
            let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(method));
            builder.add_file("data", data.clone()).unwrap();
            let bytes = builder.finish();

            let ratio = (bytes.len() as f64 / (data.len() + 100) as f64) * 100.0;
            println!("  {:?}: {} bytes ({:.1}%)", method, bytes.len(), ratio);
        }
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   QUIVER: Complete API Showcase                            ║");
    println!("║                                                              ║");
    println!("║   6 sections covering all major APIs:                       ║");
    println!("║   1. Basic API Usage (5 examples)                           ║");
    println!("║   2. Compression (5 examples)                               ║");
    println!("║   3. Error Handling (5 examples)                            ║");
    println!("║   4. Verification (4 examples)                              ║");
    println!("║   5. Real-World Patterns (5 examples)                       ║");
    println!("║   6. Performance (5 benchmarks)                             ║");
    println!("║                                                              ║");
    println!("║   Total: 29 examples demonstrating complete API surface     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    section_1_basic_api();
    section_2_compression();
    section_3_error_handling();
    section_4_verification();
    section_5_real_world();
    section_6_performance();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║   API Showcase Complete!                                   ║");
    println!("║                                                              ║");
    println!("║   All examples executed successfully.                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
