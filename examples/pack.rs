//! Build a small archive in memory and report how it packed.
//!
//! Usage: `cargo run --example pack`
//!
//! Demonstrates the writer, the automatic compression policy, and the catalog
//! summary. A real tool would read files from disk; to stay self-contained this
//! example synthesises a few entries with different compressibility.

use quiver::catalog::Catalog;
use quiver::reader::Archive;
use quiver::writer::Builder;

fn main() {
    let mut builder = Builder::new();

    // Highly compressible: a long run.
    builder
        .add_file("logs/access.log", vec![b'.'; 20_000])
        .unwrap();
    // Repetitive text: compresses via LZ back-references.
    builder
        .add_file("config/app.toml", b"key = \"value\"\n".repeat(400).to_vec())
        .unwrap();
    // Incompressible-ish: pseudo-random bytes stay stored.
    let noise: Vec<u8> = (0..4096u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
        .collect();
    builder.add_file("assets/noise.bin", noise).unwrap();
    builder.add_directory("logs").unwrap();
    builder
        .add_symlink("latest.log", "logs/access.log")
        .unwrap();

    let bytes = builder.finish();

    let archive = Archive::open(&bytes).unwrap();
    let summary = Catalog::new(&archive).summary();

    println!("built archive: {} bytes", bytes.len());
    println!(
        "entries: {} files, {} dirs, {} symlinks",
        summary.files, summary.directories, summary.symlinks
    );
    println!("original bytes : {}", summary.original_bytes);
    println!("stored bytes   : {}", summary.stored_bytes);
    println!(
        "compression    : {:.1}% of original ({} bytes saved)",
        summary.ratio() * 100.0,
        summary.saved_bytes()
    );
    println!();
    println!("per-entry:");
    for rec in archive.records() {
        println!(
            "  {:<22} {:>6} -> {:>6}  [{}]",
            rec.name,
            rec.original_len,
            rec.stored_len,
            rec.compression.label()
        );
    }
}
