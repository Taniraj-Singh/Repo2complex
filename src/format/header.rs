//! The archive file header.
//!
//! Every archive begins with a fixed-size, 20-byte header:
//!
//! ```text
//!   magic[4] "QVR1" | version u8 | flags u8 | reserved u16
//!   entry_count u32 (BE) | index_offset u64 (BE)
//! ```
//!
//! The header is deliberately fixed-width (no varints) so a reader can validate
//! and locate the index with a single bounded read.

use crate::errors::{FormatError, Result};
use crate::util::{ByteReader, ByteWriter};

/// The 4-byte archive magic.
pub const MAGIC: [u8; 4] = *b"QVR1";

/// The current archive format version.
pub const VERSION: u8 = 1;

/// The fixed on-disk size of the header in bytes.
pub const HEADER_LEN: usize = 20;

/// A parsed archive header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileHeader {
    /// Format version.
    pub version: u8,
    /// Header flags. Currently reserved; must be zero.
    pub flags: u8,
    /// The number of entries described by the index.
    pub entry_count: u32,
    /// Byte offset of the start of the index region.
    pub index_offset: u64,
}

impl FileHeader {
    /// Builds a header.
    pub fn new(entry_count: u32, index_offset: u64) -> Self {
        FileHeader {
            version: VERSION,
            flags: 0,
            entry_count,
            index_offset,
        }
    }

    /// Serialises the header (exactly [`HEADER_LEN`] bytes) into `w`.
    pub fn write(&self, w: &mut ByteWriter) {
        w.write_bytes(&MAGIC);
        w.write_u8(self.version);
        w.write_u8(self.flags);
        w.write_u16(0); // reserved
        w.write_u32(self.entry_count);
        w.write_u64(self.index_offset);
    }

    /// Parses a header from the front of `r`, validating magic, version, and the
    /// reserved fields.
    pub fn read(r: &mut ByteReader<'_>) -> Result<FileHeader> {
        let magic: [u8; 4] = r.read_array()?;
        if magic != MAGIC {
            return Err(FormatError::BadMagic { found: magic }.into());
        }
        let version = r.read_u8()?;
        if version != VERSION {
            return Err(FormatError::UnsupportedVersion(version).into());
        }
        let flags = r.read_u8()?;
        let reserved = r.read_u16()?;
        if flags != 0 || reserved != 0 {
            return Err(FormatError::ReservedField.into());
        }
        let entry_count = r.read_u32()?;
        let index_offset = r.read_u64()?;
        Ok(FileHeader {
            version,
            flags,
            entry_count,
            index_offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = FileHeader::new(3, 128);
        let mut w = ByteWriter::new();
        h.write(&mut w);
        let bytes = w.into_vec();
        assert_eq!(bytes.len(), HEADER_LEN);
        let mut r = ByteReader::new(&bytes);
        assert_eq!(FileHeader::read(&mut r).unwrap(), h);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = vec![0u8; HEADER_LEN];
        bytes[..4].copy_from_slice(b"XXXX");
        let mut r = ByteReader::new(&bytes);
        assert!(matches!(
            FileHeader::read(&mut r).unwrap_err(),
            crate::errors::Error::Format(FormatError::BadMagic { .. })
        ));
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut w = ByteWriter::new();
        w.write_bytes(&MAGIC);
        w.write_u8(99);
        w.write_u8(0);
        w.write_u16(0);
        w.write_u32(0);
        w.write_u64(0);
        let bytes = w.into_vec();
        let mut r = ByteReader::new(&bytes);
        assert!(matches!(
            FileHeader::read(&mut r).unwrap_err(),
            crate::errors::Error::Format(FormatError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn reserved_field_must_be_zero() {
        let mut w = ByteWriter::new();
        w.write_bytes(&MAGIC);
        w.write_u8(VERSION);
        w.write_u8(0);
        w.write_u16(7); // non-zero reserved
        w.write_u32(0);
        w.write_u64(0);
        let bytes = w.into_vec();
        let mut r = ByteReader::new(&bytes);
        assert!(FileHeader::read(&mut r).is_err());
    }

    #[test]
    fn truncated_header_rejected() {
        let mut r = ByteReader::new(&MAGIC);
        assert!(FileHeader::read(&mut r).is_err());
    }
}
