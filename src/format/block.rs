//! The per-entry data block header.
//!
//! Each entry that carries data is stored as a block: a fixed 16-byte header
//! followed by `stored_len` bytes of (possibly compressed) data.
//!
//! ```text
//!   magic[2] "QB" | method u8 | reserved u8
//!   stored_len u32 (BE) | original_len u32 (BE) | crc32 u32 (BE)
//! ```
//!
//! `crc32` is computed over the *stored* (on-disk) bytes, so a reader can verify
//! block integrity before spending effort decompressing.

use crate::errors::{FormatError, Result};
use crate::types::Compression;
use crate::util::{ByteReader, ByteWriter};

/// The 2-byte block magic.
pub const BLOCK_MAGIC: [u8; 2] = *b"QB";

/// The fixed on-disk size of a block header.
pub const BLOCK_HEADER_LEN: usize = 16;

/// A parsed data-block header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHeader {
    /// The compression method used for this block.
    pub method: Compression,
    /// The length of the stored (on-disk) bytes following the header.
    pub stored_len: u32,
    /// The length after decompression.
    pub original_len: u32,
    /// CRC-32 of the stored bytes.
    pub crc32: u32,
}

impl BlockHeader {
    /// Serialises the header into `w`.
    pub fn write(&self, w: &mut ByteWriter) {
        w.write_bytes(&BLOCK_MAGIC);
        w.write_u8(self.method.as_byte());
        w.write_u8(0); // reserved
        w.write_u32(self.stored_len);
        w.write_u32(self.original_len);
        w.write_u32(self.crc32);
    }

    /// Parses a block header, validating the magic and compression byte.
    pub fn read(r: &mut ByteReader<'_>) -> Result<BlockHeader> {
        let magic: [u8; 2] = r.read_array()?;
        if magic != BLOCK_MAGIC {
            return Err(FormatError::BadBlockHeader.into());
        }
        let method = Compression::from_byte(r.read_u8()?)?;
        let reserved = r.read_u8()?;
        if reserved != 0 {
            return Err(FormatError::ReservedField.into());
        }
        let stored_len = r.read_u32()?;
        let original_len = r.read_u32()?;
        let crc32 = r.read_u32()?;
        Ok(BlockHeader {
            method,
            stored_len,
            original_len,
            crc32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_header_roundtrip() {
        let h = BlockHeader {
            method: Compression::Lz,
            stored_len: 100,
            original_len: 250,
            crc32: 0xdead_beef,
        };
        let mut w = ByteWriter::new();
        h.write(&mut w);
        let bytes = w.into_vec();
        assert_eq!(bytes.len(), BLOCK_HEADER_LEN);
        let mut r = ByteReader::new(&bytes);
        assert_eq!(BlockHeader::read(&mut r).unwrap(), h);
    }

    #[test]
    fn bad_block_magic_rejected() {
        let bytes = [0u8; BLOCK_HEADER_LEN];
        let mut r = ByteReader::new(&bytes);
        assert!(matches!(
            BlockHeader::read(&mut r).unwrap_err(),
            crate::errors::Error::Format(FormatError::BadBlockHeader)
        ));
    }

    #[test]
    fn unknown_method_rejected() {
        let mut w = ByteWriter::new();
        w.write_bytes(&BLOCK_MAGIC);
        w.write_u8(200); // bad method
        w.write_u8(0);
        w.write_u32(0);
        w.write_u32(0);
        w.write_u32(0);
        let bytes = w.into_vec();
        let mut r = ByteReader::new(&bytes);
        assert!(BlockHeader::read(&mut r).is_err());
    }
}
