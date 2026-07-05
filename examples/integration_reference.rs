//! Complete integration reference: Comprehensive guide for integrating Quiver
//! into production systems with best practices and complete examples.
//!
//! This reference provides:
//! - Complete integration examples
//! - Best practices and patterns
//! - Production-ready code snippets
//! - Performance optimization guides
//! - Troubleshooting and debugging
//! - Migration strategies
//!
//! Usage: `cargo run --example integration_reference`

use quiver::catalog::Catalog;
use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::writer::{Builder, CompressionPolicy};
use std::time::Instant;

// ============================================================================
// PART I: Basic Integration Scenarios
// ============================================================================

fn part_i_basic_integration() {
    println!("\n=== PART I: Basic Integration Scenarios ===\n");

    // Scenario 1: File backup
    println!("Scenario 1: File Backup");
    {
        println!("  Setup: Collecting files to backup");
        let mut backup = Builder::new().with_policy(CompressionPolicy::Auto);
        
        let files = vec![
            ("config/app.conf", b"app_setting=value".to_vec()),
            ("config/db.conf", b"db_host=localhost".to_vec()),
            ("data/users.db", vec![0u8; 1000]),
            ("data/cache.dat", vec![0xAB; 500]),
        ];
        
        for (path, content) in files {
            backup.add_file(path, content).unwrap();
        }
        
        println!("  Creating backup archive");
        let backup_bytes = backup.finish();
        println!("  Archive size: {} bytes", backup_bytes.len());
        
        println!("  Verifying backup");
        match Pack::open_verified(&backup_bytes) {
            Ok((archive, report)) => {
                println!("  ✓ Verified {} entries, all clean: {}", report.checked(), report.is_clean());
                println!("  ✓ Backup ready for storage");
            }
            Err(e) => println!("  ✗ Verification failed: {}", e),
        }
    }

    // Scenario 2: Configuration deployment
    println!("\nScenario 2: Configuration Deployment");
    {
        println!("  Creating deployment package");
        let mut deploy = Builder::new();
        
        // Environment configs
        deploy.add_file("env/prod.toml", b"[env]\nname=production".to_vec()).unwrap();
        deploy.add_file("env/staging.toml", b"[env]\nname=staging".to_vec()).unwrap();
        deploy.add_file("env/dev.toml", b"[env]\nname=development".to_vec()).unwrap();
        
        // Application configs
        deploy.add_file("app/main.toml", b"[app]\nversion=1.0".to_vec()).unwrap();
        deploy.add_file("app/features.toml", b"[features]\ndev=true".to_vec()).unwrap();
        
        let deploy_bytes = deploy.finish();
        
        println!("  Deployment package: {} bytes", deploy_bytes.len());
        
        let archive = Archive::open(&deploy_bytes).unwrap();
        println!("  Contents:");
        for rec in archive.records() {
            println!("    - {}", rec.name);
        }
    }

    // Scenario 3: Data archive
    println!("\nScenario 3: Data Archive");
    {
        println!("  Creating data archive");
        let mut archive_builder = Builder::new().with_policy(CompressionPolicy::Auto);
        
        // Add structured data
        for dataset in 0..3 {
            let data = vec![(dataset * 10) as u8; 10000];
            archive_builder.add_file(&format!("dataset_{}/data.bin", dataset), data).unwrap();
            archive_builder.add_file(&format!("dataset_{}/metadata.json", dataset), b"{}".to_vec()).unwrap();
        }
        
        let bytes = archive_builder.finish();
        let archive = Archive::open(&bytes).unwrap();
        let catalog = Catalog::new(&archive);
        let summary = catalog.summary();
        
        println!("  Archive created");
        println!("    Entries: {}", summary.files);
        println!("    Original: {} KB", summary.original_bytes / 1024);
        println!("    Stored: {} KB", summary.stored_bytes / 1024);
        println!("    Ratio: {:.1}%", summary.ratio() * 100.0);
    }
}

// ============================================================================
// PART II: Advanced Integration Patterns
// ============================================================================

fn part_ii_advanced_patterns() {
    println!("\n=== PART II: Advanced Integration Patterns ===\n");

    // Pattern 1: Incremental archiving
    println!("Pattern 1: Incremental Archiving");
    {
        println!("  Creating base archive");
        let mut base = Builder::new();
        base.add_file("file1.txt", b"version 1".to_vec()).unwrap();
        base.add_file("file2.txt", b"static data".to_vec()).unwrap();
        let base_bytes = base.finish();
        
        println!("  Creating incremental archive");
        let mut incremental = Builder::new();
        // Only changed files
        incremental.add_file("file1.txt", b"version 2".to_vec()).unwrap();
        incremental.add_file("file3.txt", b"new file".to_vec()).unwrap();
        let inc_bytes = incremental.finish();
        
        println!("  Base: {} bytes, Incremental: {} bytes", base_bytes.len(), inc_bytes.len());
    }

    // Pattern 2: Archive composition
    println!("\nPattern 2: Archive Composition");
    {
        println!("  Creating sub-archives");
        
        // Source archives
        let mut sources = Vec::new();
        for i in 0..3 {
            let mut builder = Builder::new();
            for j in 0..5 {
                let id = i * 5 + j;
                builder.add_file(&format!("item_{id}"), vec![id as u8; 50]).unwrap();
            }
            sources.push(builder.finish());
        }
        
        println!("  Composing into single archive");
        let mut composed = Builder::new();
        for (idx, source_bytes) in sources.iter().enumerate() {
            let archive = Archive::open(source_bytes).unwrap();
            for entry in archive.entries() {
                let data = archive.read(&entry.path).unwrap();
                let prefixed = format!("source_{}/{}", idx, entry.path);
                composed.add_file(&prefixed, data).unwrap();
            }
        }
        
        let composed_bytes = composed.finish();
        println!("  Composed archive: {} bytes", composed_bytes.len());
    }

    // Pattern 3: Streaming ingestion
    println!("\nPattern 3: Streaming Ingestion");
    {
        println!("  Simulating data stream ingestion");
        let mut archive = Builder::new();
        
        let chunk_size = 1000;
        let num_chunks = 50;
        
        for chunk_id in 0..num_chunks {
            // Simulate reading from stream
            let data: Vec<u8> = (0..chunk_size).map(|i| ((chunk_id * i) % 256) as u8).collect();
            archive.add_file(&format!("chunk_{chunk_id:04}"), data).unwrap();
        }
        
        let bytes = archive.finish();
        println!("  Ingested {} chunks, archive: {} KB", num_chunks, bytes.len() / 1024);
    }

    // Pattern 4: Content-addressed storage
    println!("\nPattern 4: Content-Addressed Storage (Simulation)");
    {
        println!("  Creating content-addressed entries");
        let mut archive = Builder::new();
        
        let contents = vec![
            ("file1", b"unique content 1".to_vec()),
            ("file2", b"unique content 2".to_vec()),
            ("file3", b"unique content 1".to_vec()), // Duplicate
        ];
        
        for (name, content) in contents {
            // In real scenario, compute content hash
            let content_hash = format!("{:x}", content.len()); // Simplified hash
            archive.add_file(&format!("content/{}/{}", content_hash, name), content).unwrap();
        }
        
        let bytes = archive.finish();
        let final_archive = Archive::open(&bytes).unwrap();
        println!("  Content-addressed archive: {} entries", final_archive.len());
    }

    // Pattern 5: Multi-format handling
    println!("\nPattern 5: Multi-Format Handling");
    {
        println!("  Handling multiple file formats");
        let mut archive_builder = Builder::new();
        
        // JSON
        archive_builder.add_file("data.json", b"{\"key\":\"value\"}".to_vec()).unwrap();
        
        // TOML
        archive_builder.add_file("config.toml", b"[section]\nkey=\"value\"".to_vec()).unwrap();
        
        // Binary
        archive_builder.add_file("binary.bin", vec![0xFF; 100]).unwrap();
        
        // Text
        archive_builder.add_file("readme.txt", b"# README\n\nProject documentation.".to_vec()).unwrap();
        
        let bytes = archive_builder.finish();
        println!("  Multi-format archive: 4 entries, {} bytes", bytes.len());
    }
}

// ============================================================================
// PART III: Error Handling & Recovery
// ============================================================================

fn part_iii_error_handling() {
    println!("\n=== PART III: Error Handling & Recovery ===\n");

    // Strategy 1: Defensive reading
    println!("Strategy 1: Defensive Reading");
    {
        let mut builder = Builder::new();
        builder.add_file("good1.txt", b"data1".to_vec()).unwrap();
        builder.add_file("good2.txt", b"data2".to_vec()).unwrap();
        let bytes = builder.finish();
        
        match Archive::open(&bytes) {
            Ok(archive) => {
                println!("  Archive opened, entries: {}", archive.len());
                
                let mut successes = 0;
                let mut failures = 0;
                
                for rec in archive.records() {
                    match archive.read_record(rec) {
                        Ok(_) => successes += 1,
                        Err(e) => {
                            failures += 1;
                            println!("  Warning: Failed to read {}: {}", rec.name, e.code());
                        }
                    }
                }
                
                println!("  Result: {}/{} entries read successfully", successes, successes + failures);
            }
            Err(e) => println!("  Archive open failed: {}", e),
        }
    }

    // Strategy 2: Graceful degradation
    println!("\nStrategy 2: Graceful Degradation");
    {
        let bytes = vec![0xFF; 100];
        
        // Try strict limits first
        match Archive::open_with(&bytes, quiver::util::Limits::STRICT) {
            Ok(a) => println!("  ✓ Opened with STRICT limits: {} entries", a.len()),
            Err(_) => {
                println!("  STRICT limits failed, trying DEFAULT");
                match Archive::open(&bytes) {
                    Ok(a) => println!("  ✓ Opened with DEFAULT limits: {} entries", a.len()),
                    Err(e) => println!("  ✗ All attempts failed: {}", e.code()),
                }
            }
        }
    }

    // Strategy 3: Validation before processing
    println!("\nStrategy 3: Validation Before Processing");
    {
        let mut builder = Builder::new();
        builder.add_file("data.txt", b"important data".to_vec()).unwrap();
        let bytes = builder.finish();
        
        println!("  Performing validation checks");
        
        // Check 1: Can open
        if Archive::open(&bytes).is_err() {
            println!("  ✗ Cannot open archive");
            return;
        }
        println!("  ✓ Archive opens successfully");
        
        // Check 2: Can verify
        match Pack::open_verified(&bytes) {
            Ok((_, report)) => {
                if report.is_clean() {
                    println!("  ✓ Archive integrity verified");
                } else {
                    println!("  ✗ Archive has integrity issues");
                    return;
                }
            }
            Err(e) => {
                println!("  ✗ Verification failed: {}", e);
                return;
            }
        }
        
        println!("  ✓ All validation passed, safe to process");
    }

    // Strategy 4: Retry with backoff
    println!("\nStrategy 4: Retry with Backoff");
    {
        let mut attempt = 0;
        let max_attempts = 3;
        let bytes = vec![0xFF; 50];
        
        loop {
            attempt += 1;
            match Archive::open(&bytes) {
                Ok(a) => {
                    println!("  ✓ Success on attempt {}: {} entries", attempt, a.len());
                    break;
                }
                Err(e) if attempt < max_attempts => {
                    println!("  Attempt {}: failed ({}), retrying...", attempt, e.code());
                    // In real scenario, would implement backoff
                }
                Err(e) => {
                    println!("  ✗ Failed after {} attempts: {}", max_attempts, e);
                    break;
                }
            }
        }
    }
}

// ============================================================================
// PART IV: Performance Optimization
// ============================================================================

fn part_iv_performance() {
    println!("\n=== PART IV: Performance Optimization ===\n");

    // Optimization 1: Batch operations
    println!("Optimization 1: Batch Operations");
    {
        println!("  Comparing single vs. batch");
        
        // Single archive with many files
        let start = Instant::now();
        let mut single = Builder::new();
        for i in 0..1000 {
            single.add_file(&format!("f{i:04}"), vec![i as u8; 50]).unwrap();
        }
        let single_bytes = single.finish();
        let single_time = start.elapsed();
        
        println!("  Single archive: {} ms ({} bytes)", single_time.as_millis(), single_bytes.len());
    }

    // Optimization 2: Compression selection
    println!("\nOptimization 2: Compression Selection");
    {
        let test_cases = vec![
            ("Random", (0..5000u32).map(|i| (i.wrapping_mul(1103515245) >> 8) as u8).collect::<Vec<_>>()),
            ("Text", b"Repetitive text content. ".repeat(200)),
            ("Binary", vec![0xAB; 5000]),
        ];
        
        for (name, data) in test_cases {
            let auto_start = Instant::now();
            let mut auto = Builder::new().with_policy(CompressionPolicy::Auto);
            auto.add_file("data", data.clone()).unwrap();
            let auto_bytes = auto.finish();
            let auto_time = auto_start.elapsed();
            
            let store_start = Instant::now();
            let mut store = Builder::new().with_policy(CompressionPolicy::None);
            store.add_file("data", data.clone()).unwrap();
            let store_bytes = store.finish();
            let store_time = store_start.elapsed();
            
            println!("  {}: auto {:.2}ms ({} bytes), store {:.2}ms ({} bytes)",
                name,
                auto_time.as_secs_f64() * 1000.0,
                auto_bytes.len(),
                store_time.as_secs_f64() * 1000.0,
                store_bytes.len());
        }
    }

    // Optimization 3: Lazy evaluation
    println!("\nOptimization 3: Lazy Evaluation");
    {
        let mut builder = Builder::new();
        for i in 0..100 {
            builder.add_file(&format!("f{i:03}"), vec![i as u8; 100]).unwrap();
        }
        let bytes = builder.finish();
        
        let archive = Archive::open(&bytes).unwrap();
        
        let start = Instant::now();
        let iter = archive.entries();
        println!("  Iterator creation: {:.2}µs", start.elapsed().as_secs_f64() * 1_000_000.0);
        
        let start = Instant::now();
        let filtered: Vec<_> = iter.filter(|e| e.original_len > 50).take(10).collect();
        println!("  Filtered iteration: {:.2}µs, {} items", 
            start.elapsed().as_secs_f64() * 1_000_000.0,
            filtered.len());
    }

    // Optimization 4: Memory efficiency
    println!("\nOptimization 4: Memory Efficiency");
    {
        println!("  Processing without loading entire archive");
        
        let mut builder = Builder::new();
        for i in 0..50 {
            builder.add_file(&format!("file_{i:04}"), vec![i as u8; 1000]).unwrap();
        }
        let bytes = builder.finish();
        
        let archive = Archive::open(&bytes).unwrap();
        
        let mut total = 0u64;
        for rec in archive.records() {
            // Process one at a time
            if let Ok(data) = archive.read_record(rec) {
                total += data.len() as u64;
            }
        }
        
        println!("  Processed {} entries, total {} bytes", archive.len(), total);
    }
}

// ============================================================================
// PART V: Troubleshooting Guide
// ============================================================================

fn part_v_troubleshooting() {
    println!("\n=== PART V: Troubleshooting Guide ===\n");

    // Issue 1: Cannot open archive
    println!("Issue 1: Cannot Open Archive");
    {
        println!("  Diagnosing: Invalid format error");
        
        let bad_data = vec![0xFF; 50];
        match Archive::open(&bad_data) {
            Ok(_) => println!("  Unexpected success"),
            Err(e) => {
                println!("  Error code: {}", e.code());
                println!("  Display: {}", e);
                println!("  Is data error: {}", e.is_data_error());
                println!("  Recommendation: Check file corruption or wrong format");
            }
        }
    }

    // Issue 2: Entry not found
    println!("\nIssue 2: Entry Not Found");
    {
        println!("  Diagnosing: Missing entry");
        
        let mut builder = Builder::new();
        builder.add_file("readme.txt", b"content".to_vec()).unwrap();
        let bytes = builder.finish();
        let archive = Archive::open(&bytes).unwrap();
        
        match archive.read("missing.txt") {
            Ok(_) => println!("  Unexpected success"),
            Err(e) => {
                println!("  Error: {}", e.code());
                println!("  Available entries: {:?}", archive.records().iter().map(|r| &r.name).collect::<Vec<_>>());
                println!("  Recommendation: Check entry name spelling");
            }
        }
    }

    // Issue 3: Verification failures
    println!("\nIssue 3: Verification Failures");
    {
        println!("  Diagnosing: Integrity check failure");
        
        let mut builder = Builder::new();
        builder.add_file("data.txt", b"critical data".to_vec()).unwrap();
        let bytes = builder.finish();
        
        match Pack::open_verified(&bytes) {
            Ok((_, report)) => {
                println!("  Entries verified: {}", report.checked());
                println!("  Failures: {}", report.failures.len());
                if report.is_clean() {
                    println!("  Status: CLEAN - no issues found");
                } else {
                    println!("  Status: CORRUPTED - corruption detected");
                    for (entry_name, _error) in &report.failures {
                        println!("    - {} failed verification", entry_name);
                    }
                }
            }
            Err(e) => println!("  Error: {}", e),
        }
    }

    // Issue 4: Performance degradation
    println!("\nIssue 4: Performance Degradation");
    {
        println!("  Diagnosing: Slow operations");
        
        let mut builder = Builder::new();
        let count = 100;
        for i in 0..count {
            builder.add_file(&format!("f{i:03}"), vec![i as u8; 100]).unwrap();
        }
        
        let start = Instant::now();
        let bytes = builder.finish();
        let build_ms = start.elapsed().as_millis();
        
        let start = Instant::now();
        let archive = Archive::open(&bytes).unwrap();
        let open_ms = start.elapsed().as_millis();
        
        println!("  Build time: {}ms for {} entries", build_ms, count);
        println!("  Open time: {}ms", open_ms);
        println!("  Recommendation: Consider batch operations if too slow");
    }
}

// ============================================================================
// PART VI: Migration Guide
// ============================================================================

fn part_vi_migration() {
    println!("\n=== PART VI: Migration Guide ===\n");

    // Migration 1: From simple file format
    println!("Migration 1: From Simple File Format");
    {
        println!("  Step 1: Read old format files");
        let old_files = vec![
            ("file1.txt", b"file 1 content"),
            ("file2.txt", b"file 2 content"),
        ];
        
        println!("  Step 2: Convert to Quiver archive");
        let mut archive = Builder::new();
        for (name, content) in old_files {
            archive.add_file(name, content.to_vec()).unwrap();
        }
        
        println!("  Step 3: Verify new format");
        let bytes = archive.finish();
        match Archive::open(&bytes) {
            Ok(a) => println!("  ✓ Migration successful: {} entries", a.len()),
            Err(e) => println!("  ✗ Migration failed: {}", e),
        }
    }

    // Migration 2: From zip/tar
    println!("\nMigration 2: From Other Archive Formats");
    {
        println!("  Simulating migration from TAR format");
        
        // In real scenario, would parse TAR
        let tar_entries = vec![
            ("dir/", None),
            ("dir/file.txt", Some(b"content".to_vec())),
        ];
        
        let mut archive = Builder::new();
        for (name, content) in tar_entries {
            match content {
                Some(data) => archive.add_file(name.trim_end_matches('/'), data).unwrap(),
                None => {
                    if !name.ends_with('/') {
                        archive.add_file(name, vec![]).unwrap();
                    }
                }
            }
        }
        
        let bytes = archive.finish();
        println!("  Migrated archive: {} bytes", bytes.len());
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   QUIVER: Complete Integration Reference                   ║");
    println!("║                                                              ║");
    println!("║   Comprehensive guide for production integration:           ║");
    println!("║   - Basic integration scenarios                             ║");
    println!("║   - Advanced integration patterns                           ║");
    println!("║   - Error handling & recovery                               ║");
    println!("║   - Performance optimization                                ║");
    println!("║   - Troubleshooting guide                                   ║");
    println!("║   - Migration strategies                                    ║");
    println!("║                                                              ║");
    println!("║   6 parts covering production deployment                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    part_i_basic_integration();
    part_ii_advanced_patterns();
    part_iii_error_handling();
    part_iv_performance();
    part_v_troubleshooting();
    part_vi_migration();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║   Integration Reference Complete!                          ║");
    println!("║                                                              ║");
    println!("║   Ready for production deployment.                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
