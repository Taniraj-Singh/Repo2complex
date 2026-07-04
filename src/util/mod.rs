//! Low-level utilities shared across the crate: byte I/O, checksums, resource
//! limits, and path validation. This layer depends only on `std` and the crate
//! error type.

pub mod bytes;
pub mod checksum;
pub mod limits;
pub mod path;

pub use bytes::{ByteReader, ByteWriter};
pub use checksum::{adler32, Crc32};
pub use limits::Limits;

/// Renders `bytes` as a lowercase hex string. Small helper used by the
/// `hexdump` example and by some error diagnostics.
pub fn to_hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encoding() {
        assert_eq!(to_hex(&[0x00, 0xff, 0x10]), "00ff10");
    }
}
