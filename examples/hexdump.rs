//! Hex-dump a freshly built archive so you can see the on-disk layout.
//!
//! Usage: `cargo run --example hexdump`
//!
//! The output annotates the header, a data block, the index region, and the
//! footer, which is handy when reading `docs/FORMAT.md` alongside real bytes.

use quiver::format::{FileHeader, FOOTER_LEN, HEADER_LEN};
use quiver::util::{to_hex, ByteReader};
use quiver::writer::Builder;

fn main() {
    let mut b = Builder::new();
    b.add_file("hi.txt", b"hello, quiver".to_vec()).unwrap();
    let bytes = b.finish();

    // Parse the header so we can label the regions.
    let mut r = ByteReader::new(&bytes);
    let header = FileHeader::read(&mut r).unwrap();
    let index_off = header.index_offset as usize;
    let footer_off = bytes.len() - FOOTER_LEN;

    println!("archive: {} bytes\n", bytes.len());
    println!("offset    hex                                               region");
    for (row, chunk) in bytes.chunks(16).enumerate() {
        let off = row * 16;
        let region = region_of(off, index_off, footer_off);
        println!("{off:08x}  {:<48}  {region}", to_hex(chunk));
    }

    println!();
    println!(
        "header    : bytes 0..{HEADER_LEN} (magic QVR1, {} entries)",
        header.entry_count
    );
    println!("data      : bytes {HEADER_LEN}..{index_off}");
    println!("index     : bytes {index_off}..{footer_off}");
    println!(
        "footer    : bytes {footer_off}..{} (index CRC + QVRX)",
        bytes.len()
    );
}

fn region_of(off: usize, index_off: usize, footer_off: usize) -> &'static str {
    if off < HEADER_LEN {
        "header"
    } else if off >= footer_off {
        "footer"
    } else if off >= index_off {
        "index"
    } else {
        "data"
    }
}
