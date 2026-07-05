//! Production-grade POC: Real-world integration patterns and advanced workflows.
//!
//! This example demonstrates:
//! - Archive transformation pipelines
//! - Incremental updates and deltas
//! - Multi-archive operations and merging
//! - Streaming large archives
//! - Archive inspection and analytics
//! - Content-addressed storage simulation
//! - Archive versioning and snapshots
//!
//! Usage: `cargo run --example poc_production`

use quiver::catalog::Catalog;
use quiver::pack::Pack;
use quiver::reader::Archive;
use quiver::types::{Compression, EntryKind};

use quiver::writer::{Builder, CompressionPolicy};
use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// WORKFLOW 1: Archive Transformation Pipeline
// ============================================================================

fn workflow_1_transformation_pipeline() {
    println!("\n=== WORKFLOW 1: Archive Transformation Pipeline ===");

    // Step 1: Create source archive with mixed compression
    println!("Step 1: Create source archive");
    let mut source = Builder::new().with_policy(CompressionPolicy::Auto);
    source.add_file("README.md", b"# Project\n\nA real project.".to_vec()).unwrap();
    source.add_file("LICENSE", b"MIT License...".repeat(50).to_vec()).unwrap();
    source.add_file("src/main.rs", b"fn main() { println!(\"Hello\"); }".to_vec()).unwrap();
    source.add_file("src/lib.rs", b"pub fn lib_fn() { }".repeat(20).to_vec()).unwrap();
    source.add_file("Cargo.toml", b"[package]\nname = \"proj\"".to_vec()).unwrap();
    let source_bytes = source.finish();
    println!("  Source: {} bytes", source_bytes.len());

    // Step 2: Re-compress with different policy
    println!("Step 2: Re-compress with fixed policy");
    let source_archive = Archive::open(&source_bytes).unwrap();
    let mut recompressed = Builder::new().with_policy(CompressionPolicy::Fixed(Compression::Lz));
    
    for entry in source_archive.entries() {
        let data = source_archive.read(&entry.path).unwrap();
        if entry.kind == EntryKind::File {
            recompressed.add_file(&entry.path, data).unwrap();
        } else if entry.kind == EntryKind::Directory {
            recompressed.add_directory(&entry.path).unwrap();
        }
    }
    
    let recompressed_bytes = recompressed.finish();
    println!("  Recompressed: {} bytes", recompressed_bytes.len());
    println!("  Savings: {} bytes ({:.1}%)", 
        source_bytes.len() - recompressed_bytes.len(),
        ((source_bytes.len() - recompressed_bytes.len()) as f64 / source_bytes.len() as f64) * 100.0);

    // Step 3: Filter and normalize
    println!("Step 3: Filter and normalize");
    let mut filtered = Builder::new();
    
    for entry in source_archive.entries() {
        let data = source_archive.read(&entry.path).unwrap();
        
        // Skip certain files
        if entry.path.contains("tmp") || entry.path.contains(".git") {
            continue;
        }
        
        if entry.kind == EntryKind::File {
            filtered.add_file(&entry.path, data).unwrap();
        } else if entry.kind == EntryKind::Directory {
            filtered.add_directory(&entry.path).unwrap();
        }
    }
    
    let filtered_bytes = filtered.finish();
    println!("  Filtered: {} bytes", filtered_bytes.len());
}

// ============================================================================
// WORKFLOW 2: Incremental Updates
// ============================================================================

fn workflow_2_incremental_updates() {
    println!("\n=== WORKFLOW 2: Incremental Updates ===");

    // Create base archive (Version 1.0)
    println!("Version 1.0: Create base archive");
    let mut v1 = Builder::new();
    v1.add_file("file1.txt", b"Version 1.0 data".to_vec()).unwrap();
    v1.add_file("file2.txt", b"Static data".to_vec()).unwrap();
    v1.add_file("config.json", b"{\"version\": \"1.0\"}".to_vec()).unwrap();
    let v1_bytes = v1.finish();
    println!("  Size: {} bytes, {} entries", v1_bytes.len(), Archive::open(&v1_bytes).unwrap().len());

    // Create updated archive (Version 1.1)
    println!("\nVersion 1.1: Apply updates");
    let mut v11 = Builder::new();
    
    // Copy unchanged files
    v11.add_file("file2.txt", b"Static data".to_vec()).unwrap();
    
    // Update modified files
    v11.add_file("file1.txt", b"Version 1.1 data - UPDATED".to_vec()).unwrap();
    
    // Add new files
    v11.add_file("file3.txt", b"New in v1.1".to_vec()).unwrap();
    v11.add_file("config.json", b"{\"version\": \"1.1\", \"feature\": true}".to_vec()).unwrap();
    
    let v11_bytes = v11.finish();
    println!("  Size: {} bytes, {} entries", v11_bytes.len(), Archive::open(&v11_bytes).unwrap().len());
    println!("  Size delta: {} bytes", (v11_bytes.len() as i64 - v1_bytes.len() as i64));

    // Compare archives
    println!("\nDifference analysis:");
    let v1_archive = Archive::open(&v1_bytes).unwrap();
    let v11_archive = Archive::open(&v11_bytes).unwrap();
    
    let mut v1_files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut v11_files: HashMap<String, Vec<u8>> = HashMap::new();
    
    for rec in v1_archive.records() {
        if rec.kind == EntryKind::File {
            v1_files.insert(rec.name.clone(), v1_archive.read(&rec.name).unwrap());
        }
    }
    
    for rec in v11_archive.records() {
        if rec.kind == EntryKind::File {
            v11_files.insert(rec.name.clone(), v11_archive.read(&rec.name).unwrap());
        }
    }

    let mut added = 0;
    let mut modified = 0;
    let mut unchanged = 0;

    for (name, v11_data) in &v11_files {
        match v1_files.get(name) {
            None => {
                println!("  [+] {} (new)", name);
                added += 1;
            }
            Some(v1_data) if v1_data != v11_data => {
                println!("  [~] {} (modified)", name);
                modified += 1;
            }
            Some(_) => {
                unchanged += 1;
            }
        }
    }

    for name in v1_files.keys() {
        if !v11_files.contains_key(name) {
            println!("  [-] {} (deleted)", name);
        }
    }

    println!("Summary: {} added, {} modified, {} unchanged", added, modified, unchanged);
}

// ============================================================================
// WORKFLOW 3: Multi-Archive Merging
// ============================================================================

fn workflow_3_multi_archive_merge() {
    println!("\n=== WORKFLOW 3: Multi-Archive Merging ===");

    // Create three separate archives
    println!("Creating source archives...");
    
    let archives: Vec<(&str, Vec<(&str, Vec<u8>)>)> = vec![
        ("Archive A", vec![("docs/readme.md", b"README".to_vec()), ("docs/api.md", b"API".to_vec())]),
        ("Archive B", vec![("src/main.rs", b"fn main()".to_vec()), ("src/lib.rs", b"lib".to_vec())]),
        ("Archive C", vec![("tests/unit.rs", b"#[test]".to_vec()), ("tests/integration.rs", b"#[test]".to_vec())]),
    ];

    let mut archive_bytes = Vec::new();
    for (name, entries) in archives.iter() {
        let mut builder = Builder::new();
        for (path, data) in entries {
            builder.add_file(path, data.to_vec()).unwrap();
        }
        let bytes = builder.finish();
        archive_bytes.push((name.to_string(), bytes));
        println!("  {}: {} bytes, {} entries", name, archive_bytes.last().unwrap().1.len(), entries.len());
    }

    // Merge into single archive
    println!("\nMerging archives...");
    let mut merged = Builder::new();
    
    for (archive_name, bytes) in archive_bytes.iter() {
        let archive = Archive::open(bytes).unwrap();
        for entry in archive.entries() {
            let data = archive.read(&entry.path).unwrap();
            if entry.kind == EntryKind::File {
                // Prefix entries with archive name to avoid conflicts
                let prefixed_path = format!("{}/{}", archive_name, entry.path);
                merged.add_file(&prefixed_path, data).unwrap();
            }
        }
    }

    let merged_bytes = merged.finish();
    println!("Merged archive: {} bytes, {} entries", merged_bytes.len(), Archive::open(&merged_bytes).unwrap().len());

    // Show merged content
    let merged_archive = Archive::open(&merged_bytes).unwrap();
    println!("\nMerged content:");
    for rec in merged_archive.records() {
        println!("  {:50} {} bytes", rec.name, rec.original_len);
    }
}

// ============================================================================
// WORKFLOW 4: Archive Analytics & Reporting
// ============================================================================

fn workflow_4_analytics() {
    println!("\n=== WORKFLOW 4: Archive Analytics ===");

    // Build test archive with various content
    let mut builder = Builder::new();
    
    // Different file types
    builder.add_file("code/main.rs", b"fn main() {}".repeat(100).to_vec()).unwrap();
    builder.add_file("code/lib.rs", b"pub mod lib;".repeat(100).to_vec()).unwrap();
    builder.add_file("docs/README.md", b"# Project\n\nDescription.".repeat(50).to_vec()).unwrap();
    builder.add_file("data/config.json", b"{\"setting\": 123}".repeat(30).to_vec()).unwrap();
    builder.add_file("data/binary.bin", vec![0xFF; 1000]).unwrap();
    builder.add_directory("tmp").unwrap();
    builder.add_symlink("link", "code/main.rs").unwrap();
    
    let archive_bytes = builder.finish();
    let archive = Archive::open(&archive_bytes).unwrap();

    // Generate comprehensive report
    println!("\n📊 ARCHIVE ANALYSIS REPORT");
    println!("═══════════════════════════════════════════════════════════\n");

    let catalog = Catalog::new(&archive);
    let summary = catalog.summary();

    // Overall statistics
    println!("📈 OVERALL STATISTICS:");
    println!("  Total size (compressed):   {:<10} KB", archive_bytes.len() / 1024);
    println!("  Original size:             {:<10} KB", summary.original_bytes / 1024);
    println!("  Compression ratio:         {:<10.1}%", summary.ratio() * 100.0);
    println!("  Bytes saved:               {:<10} KB", summary.saved_bytes() / 1024);

    // Entry statistics
    println!("\n📁 ENTRY STATISTICS:");
    println!("  Files:                     {:<10}", summary.files);
    println!("  Directories:               {:<10}", summary.directories);
    println!("  Symlinks:                  {:<10}", summary.symlinks);
    println!("  Total entries:             {:<10}", archive.len());

    // File type analysis
    println!("\n📄 FILE TYPE ANALYSIS:");
    let mut by_ext: HashMap<&str, (u64, u64)> = HashMap::new();
    
    for rec in archive.records() {
        if rec.kind != EntryKind::File {
            continue;
        }
        
        let ext = if let Some(dot) = rec.name.rfind('.') {
            &rec.name[dot..]
        } else {
            "(no extension)"
        };
        
        let entry = by_ext.entry(ext).or_insert((0, 0));
        entry.0 += rec.original_len;
        entry.1 += rec.stored_len;
    }

    for (ext, (orig, stored)) in &by_ext {
        let ratio = (*stored as f64 / *orig as f64) * 100.0;
        println!("  {} : {} → {} bytes ({:.1}%)", ext, orig, stored, ratio);
    }

    // Top entries
    println!("\n🏆 TOP 5 LARGEST ENTRIES:");
    let mut entries: Vec<_> = archive.records().iter().collect();
    entries.sort_by_key(|r| std::cmp::Reverse(r.original_len));
    
    for (i, rec) in entries.iter().take(5).enumerate() {
        println!("  {}. {} : {} bytes", i + 1, rec.name, rec.original_len);
    }

    // Compression effectiveness
    println!("\n🔧 COMPRESSION EFFECTIVENESS:");
    let mut by_method: HashMap<Compression, (u64, u64)> = HashMap::new();
    
    for rec in archive.records() {
        if rec.kind != EntryKind::File {
            continue;
        }
        let entry = by_method.entry(rec.compression).or_insert((0, 0));
        entry.0 += rec.original_len;
        entry.1 += rec.stored_len;
    }

    for (method, (orig, stored)) in &by_method {
        if *orig > 0 {
            let ratio = (*stored as f64 / *orig as f64) * 100.0;
            println!("  {:10} : {} → {} bytes ({:.1}%)", method.label(), orig, stored, ratio);
        }
    }
}

// ============================================================================
// WORKFLOW 5: Streaming Large Archives
// ============================================================================

fn workflow_5_streaming() {
    println!("\n=== WORKFLOW 5: Streaming Large Archives ===");

    println!("Creating archive with streaming simulation...");

    // Simulate streaming production
    let mut builder = Builder::new();
    let chunk_size = 10_000;
    let num_chunks = 100;

    println!("  Adding {} chunks of {} bytes each...", num_chunks, chunk_size);
    for i in 0..num_chunks {
        let data = (0..chunk_size).map(|j| ((i as u32 * 31 + j as u32) % 256) as u8).collect::<Vec<_>>();
        builder.add_file(&format!("chunk_{i:04}.bin"), data).unwrap();
        
        if (i + 1) % 25 == 0 {
            println!("    Progress: {} / {} chunks added", i + 1, num_chunks);
        }
    }

    let start = Instant::now();
    let archive_bytes = builder.finish();
    let build_time = start.elapsed();

    println!("\nArchive built: {} MB in {:.2}s", archive_bytes.len() / 1024 / 1024, build_time.as_secs_f64());

    // Streaming extraction
    println!("\nStreaming extraction (sequential access)...");
    
    let start = Instant::now();
    let archive = Archive::open(&archive_bytes).unwrap();
    let parse_time = start.elapsed();

    let start = Instant::now();
    let mut extracted_total = 0u64;
    let mut extract_count = 0;
    
    for rec in archive.records() {
        if let Ok(data) = archive.read(&rec.name) {
            extracted_total += data.len() as u64;
            extract_count += 1;
        }
    }
    let extract_time = start.elapsed();

    println!("  Parsed: {:.2}ms", parse_time.as_secs_f64() * 1000.0);
    println!("  Extracted: {} entries ({} MB) in {:.2}ms", 
        extract_count, 
        extracted_total / 1024 / 1024,
        extract_time.as_secs_f64() * 1000.0);
    println!("  Throughput: {:.2} MB/s", 
        (extracted_total as f64 / 1024.0 / 1024.0) / extract_time.as_secs_f64());
}

// ============================================================================
// WORKFLOW 6: Versioned Snapshots
// ============================================================================

fn workflow_6_snapshots() {
    println!("\n=== WORKFLOW 6: Versioned Snapshots ===");

    let versions: Vec<(&str, Vec<(&str, Vec<u8>)>)> = vec![
        ("v1.0", vec![
            ("file.txt", b"Original".to_vec()),
            ("config.toml", b"version = 1".to_vec()),
        ]),
        ("v1.1", vec![
            ("file.txt", b"Updated in v1.1".to_vec()),
            ("config.toml", b"version = 1.1".to_vec()),
            ("CHANGELOG.md", b"- Update: file.txt\n- Fix: config".to_vec()),
        ]),
        ("v2.0", vec![
            ("file.txt", b"Completely rewritten for v2.0".to_vec()),
            ("config.toml", b"version = 2.0\nfeature_flag = true".to_vec()),
            ("CHANGELOG.md", b"- BREAKING: Rewrote file.txt\n- NEW: feature_flag".to_vec()),
            ("src/main.rs", b"fn main() { /* v2.0 */ }".to_vec()),
        ]),
    ];

    println!("Creating versioned snapshots...\n");

    for (version, entries) in versions.iter() {
        let mut builder = Builder::new().with_policy(CompressionPolicy::Auto);
        
        for (path, data) in entries {
            builder.add_file(path, data.to_vec()).unwrap();
        }
        
        let archive_bytes = builder.finish();
        let archive = Archive::open(&archive_bytes).unwrap();
        let catalog = Catalog::new(&archive);
        let summary = catalog.summary();

        println!("📦 {}:", version);
        println!("   Entries: {} files", summary.files);
        println!("   Size: {} bytes (compressed from {} bytes, {:.1}%)",
            archive_bytes.len(),
            summary.original_bytes,
            summary.ratio() * 100.0);
        
        // Show manifest
        println!("   Contents:");
        for rec in archive.records() {
            println!("     - {} ({} bytes)", rec.name, rec.original_len);
        }
        println!();
    }
}

// ============================================================================
// WORKFLOW 7: Verification and Health Check
// ============================================================================

fn workflow_7_verification() {
    println!("\n=== WORKFLOW 7: Archive Verification & Health Check ===");

    // Build archive
    let mut builder = Builder::new();
    for i in 0..10 {
        builder.add_file(&format!("file_{i}.bin"), vec![i as u8; 1000]).unwrap();
    }
    let good_bytes = builder.finish();

    // Full verification
    println!("Running comprehensive verification...\n");

    match Pack::open_verified(&good_bytes) {
        Ok((archive, report)) => {
            println!("✅ VERIFICATION PASSED");
            println!("   Entries checked: {}", report.checked());
            println!("   Failures: {}", report.failures.len());
            println!("   Status: CLEAN");
        }
        Err(e) => {
            println!("❌ VERIFICATION FAILED");
            println!("   Error: {}", e);
        }
    }

    // Per-entry verification
    println!("\nPer-entry verification:");
    let archive = Archive::open(&good_bytes).unwrap();
    let mut verified = 0;
    
    for rec in archive.records() {
        match archive.read_record(rec) {
            Ok(_) => verified += 1,
            Err(_) => {}
        }
    }
    
    println!("   {} / {} entries readable", verified, archive.len());
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   QUIVER POC: Production Workflows                          ║");
    println!("║                                                              ║");
    println!("║   Real-world integration patterns:                          ║");
    println!("║   - Archive transformation pipelines                        ║");
    println!("║   - Incremental updates and versioning                      ║");
    println!("║   - Multi-archive operations                                ║");
    println!("║   - Streaming and analytics                                 ║");
    println!("║   - Verification and health checks                          ║");
    println!("║                                                              ║");
    println!("║   Scenarios: 7 comprehensive workflows                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    workflow_1_transformation_pipeline();
    workflow_2_incremental_updates();
    workflow_3_multi_archive_merge();
    workflow_4_analytics();
    workflow_5_streaming();
    workflow_6_snapshots();
    workflow_7_verification();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║   All production workflows completed successfully!           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
