//! Advanced troubleshooting and diagnostic guide with deep-dive scenarios.
//!
//! This comprehensive guide covers:
//! - Advanced troubleshooting techniques
//! - Diagnostic procedures
//! - Root cause analysis
//! - Performance profiling
//! - Edge case documentation
//! - Recovery strategies
//!
//! Usage: `cargo run --example diagnostics_guide`

use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::util::Limits;
use quiver::writer::{Builder, CompressionPolicy};
use std::time::Instant;

// ============================================================================
// DIAGNOSTIC MODULE 1: Archive Integrity Analysis
// ============================================================================

fn diagnostic_1_integrity_analysis() {
    println!("\n=== DIAGNOSTIC 1: Integrity Analysis ===\n");

    println!("Analysis 1.1: Clean Archive Baseline");
    {
        println!("  Creating baseline archive");
        let mut builder = Builder::new();
        for i in 0..10 {
            builder.add_file(&format!("file_{i:02}.txt"), format!("Content {i}").into_bytes()).unwrap();
        }
        let bytes = builder.finish();

        println!("  Analyzing integrity");
        match Pack::open_verified(&bytes) {
            Ok((archive, report)) => {
                println!("  ✓ Archive size: {} bytes", bytes.len());
                println!("  ✓ Entries: {}", archive.len());
                println!("  ✓ Verified entries: {}", report.checked());
                println!("  ✓ Status: {}", if report.is_clean() { "CLEAN" } else { "CORRUPTED" });
                println!("  ✓ Corruption points: {}", report.failures.len());
            }
            Err(e) => println!("  Error: {}", e),
        }
    }

    println!("\nAnalysis 1.2: Bitflip Detection");
    {
        println!("  Creating test archive");
        let mut builder = Builder::new();
        builder.add_file("data.bin", vec![0xAA; 1000]).unwrap();
        let mut bytes = builder.finish();

        println!("  Baseline verification");
        if let Ok((_, baseline)) = Pack::open_verified(&bytes) {
            println!("  Baseline: {} entries verified, clean: {}", baseline.checked(), baseline.is_clean());
        }

        println!("  Introducing single bit corruption");
        if bytes.len() > 100 {
            bytes[100] ^= 0x01;
        }

        println!("  Post-corruption verification");
        if let Ok((_, after)) = Pack::open_verified(&bytes) {
            println!("  After: {} entries checked, clean: {}", after.checked(), after.is_clean());
            println!("  Corruptions detected: {}", after.failures.len());
        }
    }

    println!("\nAnalysis 1.3: Multi-Entry Corruption Mapping");
    {
        println!("  Creating multi-entry archive");
        let mut builder = Builder::new();
        for i in 0..20 {
            builder.add_file(&format!("entry_{i:02}"), vec![i as u8; 100]).unwrap();
        }
        let mut bytes = builder.finish();

        println!("  Introducing multiple corruptions");
        if bytes.len() > 200 {
            bytes[150] ^= 0xFF;
            bytes[250] ^= 0xFF;
        }

        println!("  Detailed corruption analysis");
        match Pack::open_verified(&bytes) {
            Ok((archive, report)) => {
                println!("  Total entries: {}", archive.len());
                println!("  Entries verified: {}", report.checked());
                println!("  Entries corrupted: {}", report.failures.len());
                
                if !report.is_clean() {
                    println!("  Corrupted entries:");
                    for (entry_name, _error) in &report.failures {
                        println!("    - {}", entry_name);
                    }
                }
            }
            Err(e) => println!("  Analysis error: {}", e),
        }
    }

    println!("\nAnalysis 1.4: Selective Recovery");
    {
        println!("  Creating recoverable archive");
        let mut builder = Builder::new();
        builder.add_file("critical.txt", b"CRITICAL DATA".to_vec()).unwrap();
        builder.add_file("important.txt", b"Important data".to_vec()).unwrap();
        builder.add_file("temp.txt", b"Temporary".to_vec()).unwrap();
        let mut bytes = builder.finish();

        // Corrupt the archive
        if bytes.len() > 100 {
            bytes[100] ^= 0xFF;
        }

        println!("  Attempting recovery");
        match Archive::open(&bytes) {
            Ok(archive) => {
                println!("  Archive opened despite corruption");
                let mut recovered = 0;
                let mut failed = 0;

                for rec in archive.records() {
                    match archive.read_record(rec) {
                        Ok(data) => {
                            println!("    ✓ Recovered {}: {} bytes", rec.name, data.len());
                            recovered += 1;
                        }
                        Err(_) => {
                            println!("    ✗ Failed to recover {}", rec.name);
                            failed += 1;
                        }
                    }
                }

                println!("  Summary: {} recovered, {} failed", recovered, failed);
            }
            Err(e) => println!("  Cannot open archive: {}", e),
        }
    }
}

// ============================================================================
// DIAGNOSTIC MODULE 2: Compression Analysis
// ============================================================================

fn diagnostic_2_compression_analysis() {
    println!("\n=== DIAGNOSTIC 2: Compression Analysis ===\n");

    println!("Analysis 2.1: Compression Ratio Profiles");
    {
        println!("  Testing various data patterns");

        let patterns = vec![
            ("All zeros", vec![0u8; 10000]),
            ("All ones", vec![0xFFu8; 10000]),
            ("Alternating", (0..10000u32).map(|i| if i % 2 == 0 { 0x00 } else { 0xFF }).collect::<Vec<_>>()),
            ("Incremental", (0..10000u16).map(|i| (i % 256) as u8).collect::<Vec<_>>()),
            ("Random", (0..10000u32).map(|i| (i.wrapping_mul(1103515245) >> 16) as u8).collect::<Vec<_>>()),
        ];

        for (name, data) in patterns {
            println!("\n  Pattern: {} ({} bytes original)", name, data.len());

            for method in [Compression::Store, Compression::Rle, Compression::Lz] {
                let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(method));
                builder.add_file("data", data.clone()).unwrap();
                let bytes = builder.finish();

                let overhead = bytes.len().saturating_sub(data.len());
                let ratio = (bytes.len() as f64 / data.len() as f64) * 100.0;
                println!("    {:?}: {} bytes (ratio: {:.1}%, overhead: {} bytes)", method, bytes.len(), ratio, overhead);
            }
        }
    }

    println!("\nAnalysis 2.2: Compression Method Selection");
    {
        println!("  Analyzing Auto compression decisions");

        let test_cases = vec![
            ("Empty", vec![]),
            ("Single byte", vec![0x42]),
            ("Two bytes", vec![0x42, 0x42]),
            ("Small", vec![0x42; 50]),
            ("Medium", vec![0x42; 5000]),
            ("Large", vec![0x42; 100000]),
        ];

        for (name, data) in test_cases {
            let mut builder = Builder::new().with_policy(CompressionPolicy::Auto);
            builder.add_file("test", data.clone()).unwrap();
            let bytes = builder.finish();

            let archive = Archive::open(&bytes).unwrap();
            let rec = &archive.records()[0];
            println!("  {:<15} {} bytes -> {:<20} {:?}", 
                name, data.len(), format!("{} bytes", rec.stored_len), rec.compression);
        }
    }

    println!("\nAnalysis 2.3: Mixed Compression Archive");
    {
        println!("  Creating archive with mixed compression methods");

        let mut builder = Builder::new();

        // Add uncompressible data
        builder.add_file("random.bin", (0..5000u32).map(|i| (i.wrapping_mul(2654435761) >> 16) as u8).collect::<Vec<u8>>()).unwrap();

        // Add highly compressible data
        builder.add_file("zeros.bin", vec![0u8; 5000]).unwrap();

        // Add text (mixed)
        builder.add_file("text.txt", b"The quick brown fox jumps over the lazy dog. ".repeat(100)).unwrap();

        let bytes = builder.finish();
        let archive = Archive::open(&bytes).unwrap();

        println!("  Archive analysis:");
        for rec in archive.records() {
            let ratio = (rec.stored_len as f64 / rec.original_len as f64) * 100.0;
            println!("    {:<20} {} -> {} ({:.1}%) [{:?}]", 
                rec.name, rec.original_len, rec.stored_len, ratio, rec.compression);
        }
    }

    println!("\nAnalysis 2.4: Compression Performance Trade-offs");
    {
        println!("  Measuring compression speed vs ratio");

        let data = vec![0x42u8; 50000];

        for method in [Compression::Store, Compression::Rle, Compression::Lz] {
            let start = Instant::now();
            for _ in 0..10 {
                let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(method));
                builder.add_file("data", data.clone()).unwrap();
                let _bytes = builder.finish();
            }
            let elapsed = start.elapsed();

            let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(method));
            builder.add_file("data", data.clone()).unwrap();
            let bytes = builder.finish();

            let ratio = (bytes.len() as f64 / data.len() as f64) * 100.0;
            let avg_time = elapsed.as_secs_f64() / 10.0;

            println!("  {:?}: {:.1}% ratio, {:.3}ms per iteration", method, ratio, avg_time * 1000.0);
        }
    }
}

// ============================================================================
// DIAGNOSTIC MODULE 3: Limits & Resource Analysis
// ============================================================================

fn diagnostic_3_limits_analysis() {
    println!("\n=== DIAGNOSTIC 3: Limits & Resource Analysis ===\n");

    println!("Analysis 3.1: Limit Profiles");
    {
        println!("  Testing different limit configurations");

        let limits_configs = vec![
            ("STRICT", Limits::STRICT),
            ("DEFAULT", Limits::DEFAULT),
            ("GENEROUS", Limits {
                max_entries: 1_000_000,
                max_name_len: 65535,
                max_total_decoded: 10_000_000_000,
                max_decoded_block: 1_000_000_000,
                max_path_depth: 1000,
            }),
        ];

        for (name, limits) in limits_configs {
            println!("\n  Limits: {}", name);
            println!("    max_entries:        {}", limits.max_entries);
            println!("    max_name_len:       {}", limits.max_name_len);
            println!("    max_total_decoded:  {}", limits.max_total_decoded);
            println!("    max_decoded_block:  {}", limits.max_decoded_block);
            println!("    max_path_depth:     {}", limits.max_path_depth);
        }
    }

    println!("\nAnalysis 3.2: Edge Case: Maximum Limits");
    {
        println!("  Testing near-limit operations");

        let mut builder = Builder::new();

        // Create archive close to entry limit
        for i in 0..100 {
            builder.add_file(&format!("file_{i:04}"), vec![i as u8; 10]).unwrap();
        }

        let bytes = builder.finish();

        // Try opening with tight limits
        let tight_limits = Limits {
            max_entries: 100,
            ..Limits::DEFAULT
        };

        match Archive::open_with(&bytes, tight_limits) {
            Ok(archive) => println!("  ✓ Opened with max_entries=100: {} entries", archive.len()),
            Err(e) => println!("  ✗ Rejected with tight limits: {}", e.code()),
        }
    }

    println!("\nAnalysis 3.3: Memory Profile");
    {
        println!("  Analyzing memory usage patterns");

        let test_sizes = vec![
            ("Tiny", 100),
            ("Small", 10_000),
            ("Medium", 100_000),
            ("Large", 1_000_000),
        ];

        for (name, bytes_per_entry) in test_sizes {
            let mut builder = Builder::new();
            let num_entries = 1000;

            for i in 0..num_entries {
                builder.add_file(&format!("file_{i:04}"), vec![0u8; bytes_per_entry]).unwrap();
            }

            let bytes = builder.finish();
            let avg_size_per_entry = bytes.len() / num_entries;

            println!("  {}: {} entries × {} bytes/entry = {} KB", 
                name, num_entries, bytes_per_entry, bytes.len() / 1024);
            println!("    Average per entry: {} bytes", avg_size_per_entry);
        }
    }
}

// ============================================================================
// DIAGNOSTIC MODULE 4: Performance Profiling
// ============================================================================

fn diagnostic_4_performance_profiling() {
    println!("\n=== DIAGNOSTIC 4: Performance Profiling ===\n");

    println!("Profile 4.1: Operation Latencies");
    {
        println!("  Measuring individual operation timings");

        let mut builder = Builder::new();
        for i in 0..500 {
            builder.add_file(&format!("f{i:03}"), vec![i as u8; 100]).unwrap();
        }
        let bytes = builder.finish();

        // Archive opening
        let start = Instant::now();
        let archive = Archive::open(&bytes).unwrap();
        let open_time = start.elapsed();

        // Index-based access
        let start = Instant::now();
        let _data = archive.read_index(250).unwrap();
        let index_time = start.elapsed();

        // Name-based access
        let start = Instant::now();
        let _data = archive.read("f250").unwrap();
        let name_time = start.elapsed();

        // Iteration
        let start = Instant::now();
        let count = archive.entries().count();
        let iter_time = start.elapsed();

        println!("  Open:  {:.2}µs", open_time.as_secs_f64() * 1_000_000.0);
        println!("  Index: {:.2}µs", index_time.as_secs_f64() * 1_000_000.0);
        println!("  Name:  {:.2}µs", name_time.as_secs_f64() * 1_000_000.0);
        println!("  Iter:  {:.2}µs for {} items", iter_time.as_secs_f64() * 1_000_000.0, count);
    }

    println!("\nProfile 4.2: Throughput Benchmarks");
    {
        println!("  Measuring throughput metrics");

        let mut builder = Builder::new();
        for i in 0..100 {
            builder.add_file(&format!("f{i:03}"), vec![i as u8; 10000]).unwrap();
        }
        let bytes = builder.finish();
        let archive = Archive::open(&bytes).unwrap();

        // Sequential read throughput
        let start = Instant::now();
        let mut total = 0u64;
        for rec in archive.records() {
            if let Ok(data) = archive.read_record(rec) {
                total += data.len() as u64;
            }
        }
        let seq_time = start.elapsed();
        let throughput = (total as f64) / (seq_time.as_secs_f64() * 1_000_000_000.0);

        println!("  Sequential read: {:.2} GB/s ({} bytes in {:.2}ms)", 
            throughput, total, seq_time.as_secs_f64() * 1000.0);
    }

    println!("\nProfile 4.3: Scalability Analysis");
    {
        println!("  Testing scalability with increasing entry counts");

        for entry_count in [10, 100, 1000, 10000].iter() {
            let mut builder = Builder::new();
            for i in 0..*entry_count {
                builder.add_file(&format!("f{i:06}"), vec![i as u8; 10]).unwrap();
            }

            let start = Instant::now();
            let bytes = builder.finish();
            let build_time = start.elapsed();

            let start = Instant::now();
            let _archive = Archive::open(&bytes).unwrap();
            let open_time = start.elapsed();

            let avg_build_ms = build_time.as_secs_f64() * 1000.0 / *entry_count as f64;

            println!("  {} entries: build {:.2}ms ({:.3}ms/entry), open {:.2}µs", 
                entry_count, build_time.as_secs_f64() * 1000.0, avg_build_ms,
                open_time.as_secs_f64() * 1_000_000.0);
        }
    }
}

// ============================================================================
// DIAGNOSTIC MODULE 5: Error Classification
// ============================================================================

fn diagnostic_5_error_classification() {
    println!("\n=== DIAGNOSTIC 5: Error Classification ===\n");

    println!("Classification 5.1: Format Errors");
    {
        println!("  Detecting format-related errors");

        let error_cases = vec![
            ("Empty bytes", vec![]),
            ("Single byte", vec![0xFF]),
            ("Wrong magic", vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
            ("Truncated header", vec![0x51, 0x75, 0x69, 0x76]),
        ];

        for (desc, data) in error_cases {
            match Archive::open(&data) {
                Ok(_) => println!("  {}: UNEXPECTED SUCCESS", desc),
                Err(e) => println!("  {}: {}", desc, e.code()),
            }
        }
    }

    println!("\nClassification 5.2: Data Errors");
    {
        println!("  Detecting data integrity errors");

        let mut builder = Builder::new();
        builder.add_file("test.txt", b"data".to_vec()).unwrap();
        let mut bytes = builder.finish();

        // Corrupt data
        if bytes.len() > 50 {
            bytes[50] ^= 0xFF;
        }

        match Archive::open(&bytes) {
            Ok(archive) => {
                match archive.read("test.txt") {
                    Ok(_) => println!("  Data read despite corruption"),
                    Err(e) => println!("  Corruption detected: {}", e.code()),
                }
            }
            Err(e) => println!("  Archive rejected: {}", e.code()),
        }
    }

    println!("\nClassification 5.3: Usage Errors");
    {
        println!("  Detecting usage-related errors");

        let mut builder = Builder::new();
        builder.add_file("single.txt", b"data".to_vec()).unwrap();
        let bytes = builder.finish();

        let archive = Archive::open(&bytes).unwrap();

        // Try to access non-existent entry
        match archive.read("nonexistent.txt") {
            Ok(_) => println!("  Found non-existent entry"),
            Err(e) => println!("  Missing entry: {}", e.code()),
        }

        // Try to access invalid index
        match archive.read_index(999) {
            Ok(_) => println!("  Found invalid index"),
            Err(e) => println!("  Invalid index: {}", e.code()),
        }
    }
}

// ============================================================================
// DIAGNOSTIC MODULE 6: Cross-Cutting Concerns
// ============================================================================

fn diagnostic_6_cross_cutting() {
    println!("\n=== DIAGNOSTIC 6: Cross-Cutting Concerns ===\n");

    println!("Concern 6.1: Archive Evolution");
    {
        println!("  Simulating archive version evolution");

        let mut v1 = Builder::new();
        v1.add_file("data.txt", b"Version 1".to_vec()).unwrap();
        let v1_bytes = v1.finish();

        let mut v2 = Builder::new();
        v2.add_file("data.txt", b"Version 2".to_vec()).unwrap();
        v2.add_file("new.txt", b"Added in v2".to_vec()).unwrap();
        let v2_bytes = v2.finish();

        let mut v3 = Builder::new();
        v3.add_file("data.txt", b"Version 3".to_vec()).unwrap();
        v3.add_file("new.txt", b"Updated in v3".to_vec()).unwrap();
        v3.add_file("extra.txt", b"Added in v3".to_vec()).unwrap();
        let v3_bytes = v3.finish();

        println!("  V1: {} bytes, {} entries", v1_bytes.len(), Archive::open(&v1_bytes).unwrap().len());
        println!("  V2: {} bytes, {} entries", v2_bytes.len(), Archive::open(&v2_bytes).unwrap().len());
        println!("  V3: {} bytes, {} entries", v3_bytes.len(), Archive::open(&v3_bytes).unwrap().len());
    }

    println!("\nConcern 6.2: Compatibility Matrix");
    {
        println!("  Testing compatibility combinations");

        println!("  Compression method compatibility:");
        for method in [Compression::Store, Compression::Rle, Compression::Lz] {
            let mut builder = Builder::new().with_policy(CompressionPolicy::Fixed(method));
            builder.add_file("test", b"test data".to_vec()).unwrap();
            let bytes = builder.finish();

            match Archive::open(&bytes) {
                Ok(archive) => {
                    let rec = &archive.records()[0];
                    println!("    {:?}: Successfully read (stored as {:?})", method, rec.compression);
                }
                Err(e) => println!("    {:?}: Error - {}", method, e),
            }
        }
    }

    println!("\nConcern 6.3: Interoperability");
    {
        println!("  Testing data interchange patterns");

        // Export scenario
        println!("  Export scenario:");
        let mut export_builder = Builder::new();
        export_builder.add_file("exported.json", b"{{\"data\":\"value\"}}".to_vec()).unwrap();
        let export_bytes = export_builder.finish();

        // Import scenario
        println!("  Import scenario:");
        let archive = Archive::open(&export_bytes).unwrap();
        let data = archive.read("exported.json").unwrap();
        println!("    Imported {} bytes", data.len());

        // Re-export scenario
        println!("  Re-export scenario:");
        let mut reimport_builder = Builder::new();
        reimport_builder.add_file("reimported.json", data).unwrap();
        let _reimport_bytes = reimport_builder.finish();
        println!("    Re-exported successfully");
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   QUIVER: Diagnostics & Troubleshooting Guide              ║");
    println!("║                                                              ║");
    println!("║   6 comprehensive diagnostic modules:                       ║");
    println!("║   1. Integrity Analysis                                     ║");
    println!("║   2. Compression Analysis                                   ║");
    println!("║   3. Limits & Resource Analysis                             ║");
    println!("║   4. Performance Profiling                                  ║");
    println!("║   5. Error Classification                                   ║");
    println!("║   6. Cross-Cutting Concerns                                 ║");
    println!("║                                                              ║");
    println!("║   Deep-dive coverage of library internals                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    diagnostic_1_integrity_analysis();
    diagnostic_2_compression_analysis();
    diagnostic_3_limits_analysis();
    diagnostic_4_performance_profiling();
    diagnostic_5_error_classification();
    diagnostic_6_cross_cutting();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║   Diagnostics Complete!                                    ║");
    println!("║                                                              ║");
    println!("║   All diagnostic scenarios executed successfully.           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
