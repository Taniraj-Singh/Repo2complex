//! Entry records — the elements of the archive index.
//!
//! A record ties an entry's metadata to the location and shape of its data on
//! disk. On-disk layout (all integers big-endian):
//!
//! ```text
//!   name_len u16 | name[name_len] | kind u8 | compression u8
//!   data_offset u64 | stored_len u64 | original_len u64
//!   crc32 u32 | mode u32 | mtime i64
//! ```
//!
//! `data_offset` points at the entry's [`BlockHeader`](super::BlockHeader);
//! `crc32` covers the entry's *original* (decompressed) bytes for end-to-end
//! verification, distinct from the block header's CRC over the stored bytes.

use crate::errors::{Error, FormatError, Result};
use crate::types::{Compression, EntryKind, EntryMeta};
use crate::util::{ByteReader, ByteWriter, Limits};

/// The fixed portion of a record (everything but the variable-length name).
pub const RECORD_FIXED_LEN: usize = 1 + 1 + 8 + 8 + 8 + 4 + 4 + 8;

/// One entry as stored in the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRecord {
    /// The entry's normalized path.
    pub name: String,
    /// What the entry is.
    pub kind: EntryKind,
    /// How the entry's data is stored.
    pub compression: Compression,
    /// Byte offset of the entry's block header (0 for entries without data).
    pub data_offset: u64,
    /// Stored (on-disk) data length.
    pub stored_len: u64,
    /// Original (decompressed) data length.
    pub original_len: u64,
    /// CRC-32 of the original data.
    pub crc32: u32,
    /// Unix permission bits.
    pub mode: u32,
    /// Modification time (seconds since the Unix epoch).
    pub mtime: i64,
}

impl EntryRecord {
    /// Serialises the record into `w`.
    pub fn write(&self, w: &mut ByteWriter) {
        // Names longer than u16::MAX are rejected by the builder before we get
        // here; clamp defensively so serialisation is always well-formed.
        let name_len = self.name.len().min(u16::MAX as usize) as u16;
        w.write_u16(name_len);
        w.write_bytes(&self.name.as_bytes()[..name_len as usize]);
        w.write_u8(self.kind.as_byte());
        w.write_u8(self.compression.as_byte());
        w.write_u64(self.data_offset);
        w.write_u64(self.stored_len);
        w.write_u64(self.original_len);
        w.write_u32(self.crc32);
        w.write_u32(self.mode);
        w.write_i64(self.mtime);
    }

    /// Parses a record from `r`, enforcing `limits` on the name length.
    pub fn read(r: &mut ByteReader<'_>, limits: &Limits) -> Result<EntryRecord> {
        let name_len = r.read_u16()? as u32;
        if name_len > limits.max_name_len {
            return Err(Error::LimitExceeded {
                limit: "max_name_len",
                cap: limits.max_name_len as u64,
                got: name_len as u64,
            });
        }
        let name_bytes = r.read_bytes(name_len as usize)?;
        let name = core::str::from_utf8(name_bytes)
            .map_err(|_| Error::Format(FormatError::InvalidUtf8))?;
        let kind = EntryKind::from_byte(r.read_u8()?)?;
        let compression = Compression::from_byte(r.read_u8()?)?;
        let data_offset = r.read_u64()?;
        let stored_len = r.read_u64()?;
        let original_len = r.read_u64()?;
        let crc32 = r.read_u32()?;
        let mode = r.read_u32()?;
        let mtime = r.read_i64()?;
        Ok(EntryRecord {
            name: name.to_owned(),
            kind,
            compression,
            data_offset,
            stored_len,
            original_len,
            crc32,
            mode,
            mtime,
        })
    }

    /// The full on-disk size of this record, including its name.
    pub fn encoded_len(&self) -> usize {
        2 + self.name.len().min(u16::MAX as usize) + RECORD_FIXED_LEN
    }

    /// Projects the record onto the higher-level [`EntryMeta`] view.
    pub fn to_meta(&self) -> EntryMeta {
        EntryMeta {
            path: self.name.clone(),
            kind: self.kind,
            compression: self.compression,
            original_len: self.original_len,
            mode: self.mode,
            mtime: self.mtime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EntryRecord {
        EntryRecord {
            name: "dir/file.txt".into(),
            kind: EntryKind::File,
            compression: Compression::Rle,
            data_offset: 20,
            stored_len: 40,
            original_len: 100,
            crc32: 0x1234_5678,
            mode: 0o644,
            mtime: 1_700_000_000,
        }
    }

    #[test]
    fn record_roundtrip() {
        let rec = sample();
        let mut w = ByteWriter::new();
        rec.write(&mut w);
        let bytes = w.into_vec();
        assert_eq!(bytes.len(), rec.encoded_len());
        let mut r = ByteReader::new(&bytes);
        let back = EntryRecord::read(&mut r, &Limits::DEFAULT).unwrap();
        assert_eq!(back, rec);
    }

    #[test]
    fn name_len_limit_enforced() {
        let rec = sample();
        let mut w = ByteWriter::new();
        rec.write(&mut w);
        let bytes = w.into_vec();
        let tight = Limits {
            max_name_len: 4,
            ..Limits::DEFAULT
        };
        let mut r = ByteReader::new(&bytes);
        assert!(EntryRecord::read(&mut r, &tight).is_err());
    }

    #[test]
    fn invalid_utf8_name_rejected() {
        let mut w = ByteWriter::new();
        w.write_u16(2);
        w.write_bytes(&[0xff, 0xfe]);
        w.write_u8(0);
        w.write_u8(0);
        w.write_u64(0);
        w.write_u64(0);
        w.write_u64(0);
        w.write_u32(0);
        w.write_u32(0);
        w.write_i64(0);
        let bytes = w.into_vec();
        let mut r = ByteReader::new(&bytes);
        assert!(EntryRecord::read(&mut r, &Limits::DEFAULT).is_err());
    }

    #[test]
    fn to_meta_projects_fields() {
        let rec = sample();
        let meta = rec.to_meta();
        assert_eq!(meta.path, rec.name);
        assert_eq!(meta.original_len, rec.original_len);
    }
}
