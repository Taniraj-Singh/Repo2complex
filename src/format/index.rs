//! The archive index region and footer.
//!
//! After the data blocks comes the index: `entry_count` [`EntryRecord`]s written
//! back-to-back. The file ends with an 8-byte footer:
//!
//! ```text
//!   index_crc u32 (BE) | footer_magic[4] "QVRX"
//! ```
//!
//! `index_crc` is the CRC-32 of the whole index region, letting a reader detect
//! corruption of the directory before it trusts any offsets within it.

use crate::errors::{Error, FormatError, Result};
use crate::util::{ByteReader, ByteWriter, Crc32, Limits};

use super::record::EntryRecord;

/// The 4-byte footer magic.
pub const FOOTER_MAGIC: [u8; 4] = *b"QVRX";

/// The fixed on-disk size of the footer.
pub const FOOTER_LEN: usize = 8;

/// Serialises `records` into an index region and returns `(bytes, crc)`, where
/// `crc` is the CRC-32 of the region — the value to store in the footer.
pub fn encode_index(records: &[EntryRecord]) -> (Vec<u8>, u32) {
    let mut w = ByteWriter::new();
    for rec in records {
        rec.write(&mut w);
    }
    let bytes = w.into_vec();
    let crc = Crc32::hash(&bytes);
    (bytes, crc)
}

/// Appends the footer for an index whose CRC is `index_crc`.
pub fn write_footer(w: &mut ByteWriter, index_crc: u32) {
    w.write_u32(index_crc);
    w.write_bytes(&FOOTER_MAGIC);
}

/// Parses the footer from the last [`FOOTER_LEN`] bytes of `file`, returning the
/// stored index CRC.
pub fn read_footer(file: &[u8]) -> Result<u32> {
    if file.len() < FOOTER_LEN {
        return Err(Error::truncated(0, FOOTER_LEN, file.len()));
    }
    let footer = &file[file.len() - FOOTER_LEN..];
    let mut r = ByteReader::new(footer);
    let index_crc = r.read_u32()?;
    let magic: [u8; 4] = r.read_array()?;
    if magic != FOOTER_MAGIC {
        return Err(FormatError::BadFooterMagic.into());
    }
    Ok(index_crc)
}

/// Parses `entry_count` records from an index region.
///
/// Enforces `limits` on per-name length and on the total number of entries, and
/// requires the region to be exactly consumed (no trailing bytes), which
/// catches a truncated or padded index.
pub fn decode_index(region: &[u8], entry_count: u32, limits: &Limits) -> Result<Vec<EntryRecord>> {
    if entry_count as u64 > limits.max_entries {
        return Err(Error::LimitExceeded {
            limit: "max_entries",
            cap: limits.max_entries,
            got: entry_count as u64,
        });
    }
    let mut records = Vec::with_capacity((entry_count as usize).min(1024));
    let mut r = ByteReader::new(region);
    for _ in 0..entry_count {
        records.push(EntryRecord::read(&mut r, limits)?);
    }
    if !r.is_empty() {
        // Extra bytes after the declared records mean a malformed index.
        return Err(FormatError::ReservedField.into());
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Compression, EntryKind};

    fn rec(name: &str) -> EntryRecord {
        EntryRecord {
            name: name.into(),
            kind: EntryKind::File,
            compression: Compression::Store,
            data_offset: 20,
            stored_len: 3,
            original_len: 3,
            crc32: 0,
            mode: 0o644,
            mtime: 0,
        }
    }

    #[test]
    fn index_roundtrip() {
        let records = vec![rec("a.txt"), rec("dir/b.bin"), rec("c")];
        let (bytes, crc) = encode_index(&records);
        assert_eq!(Crc32::hash(&bytes), crc);
        let back = decode_index(&bytes, records.len() as u32, &Limits::DEFAULT).unwrap();
        assert_eq!(back, records);
    }

    #[test]
    fn footer_roundtrip() {
        let mut w = ByteWriter::new();
        write_footer(&mut w, 0xABCD_1234);
        let bytes = w.into_vec();
        assert_eq!(bytes.len(), FOOTER_LEN);
        assert_eq!(read_footer(&bytes).unwrap(), 0xABCD_1234);
    }

    #[test]
    fn bad_footer_magic_rejected() {
        let mut w = ByteWriter::new();
        w.write_u32(0);
        w.write_bytes(b"NOPE");
        let bytes = w.into_vec();
        assert!(read_footer(&bytes).is_err());
    }

    #[test]
    fn entry_count_limit_enforced() {
        let (bytes, _) = encode_index(&[rec("a")]);
        let tight = Limits {
            max_entries: 0,
            ..Limits::DEFAULT
        };
        assert!(decode_index(&bytes, 1, &tight).is_err());
    }

    #[test]
    fn trailing_index_bytes_rejected() {
        let (mut bytes, _) = encode_index(&[rec("a")]);
        bytes.push(0);
        assert!(decode_index(&bytes, 1, &Limits::DEFAULT).is_err());
    }

    #[test]
    fn short_count_leaves_trailing_and_errors() {
        // Two records encoded but we claim one -> trailing bytes -> error.
        let (bytes, _) = encode_index(&[rec("a"), rec("b")]);
        assert!(decode_index(&bytes, 1, &Limits::DEFAULT).is_err());
    }
}
