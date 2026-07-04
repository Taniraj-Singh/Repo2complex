//! The random-access archive reader.
//!
//! [`Archive`] borrows a byte slice and parses it into a validated directory of
//! entries without copying the data. Opening an archive validates the header,
//! footer, index CRC, and every entry's data range up front, so that later
//! `read_*` calls operate on already-checked offsets. Decoding an entry's data
//! additionally verifies the block CRC, the decoded length, and the end-to-end
//! content CRC.
//!
//! Every step is bounds- and limit-checked; no input can make the reader panic,
//! read out of bounds, or allocate unboundedly.

use std::collections::HashMap;

use crate::codec;
use crate::errors::{Error, FormatError, IntegrityError, Result};
use crate::format::{
    decode_index, read_footer, BlockHeader, EntryRecord, FileHeader, BLOCK_HEADER_LEN, FOOTER_LEN,
    HEADER_LEN,
};
use crate::types::EntryMeta;
use crate::util::{ByteReader, Crc32, Limits};

/// A parsed, validated archive over a borrowed byte buffer.
pub struct Archive<'a> {
    data: &'a [u8],
    records: Vec<EntryRecord>,
    by_name: HashMap<String, usize>,
    limits: Limits,
}

impl<'a> Archive<'a> {
    /// Opens an archive with the default [`Limits`].
    pub fn open(data: &'a [u8]) -> Result<Archive<'a>> {
        Archive::open_with(data, Limits::DEFAULT)
    }

    /// Opens an archive, enforcing `limits` throughout parsing and decoding.
    pub fn open_with(data: &'a [u8], limits: Limits) -> Result<Archive<'a>> {
        // 1. Header.
        let mut r = ByteReader::new(data);
        let header = FileHeader::read(&mut r)?;
        let file_len = data.len() as u64;

        // 2. Locate the index region between the header and the footer.
        let index_end = file_len
            .checked_sub(FOOTER_LEN as u64)
            .ok_or_else(|| Error::truncated(0, FOOTER_LEN, data.len()))?;
        if header.index_offset < HEADER_LEN as u64 || header.index_offset > index_end {
            return Err(FormatError::IndexOutOfRange {
                offset: header.index_offset,
                size: file_len,
            }
            .into());
        }

        // 3. Footer + index CRC.
        let stored_crc = read_footer(data)?;
        let region = &data[header.index_offset as usize..index_end as usize];
        let actual_crc = Crc32::hash(region);
        if actual_crc != stored_crc {
            return Err(IntegrityError::IndexChecksum {
                expected: stored_crc,
                actual: actual_crc,
            }
            .into());
        }

        // 4. Parse records and validate every data range.
        let records = decode_index(region, header.entry_count, &limits)?;
        for rec in &records {
            validate_range(rec, header.index_offset)?;
        }

        // 5. Build the name index.
        let mut by_name = HashMap::with_capacity(records.len());
        for (i, rec) in records.iter().enumerate() {
            by_name.insert(rec.name.clone(), i);
        }

        Ok(Archive {
            data,
            records,
            by_name,
            limits,
        })
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the archive has no entries.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The raw entry records, in index order.
    pub fn records(&self) -> &[EntryRecord] {
        &self.records
    }

    /// Iterates entry metadata in index order.
    pub fn entries(&self) -> impl Iterator<Item = EntryMeta> + '_ {
        self.records.iter().map(EntryRecord::to_meta)
    }

    /// Looks up a record by exact path.
    pub fn find(&self, path: &str) -> Option<&EntryRecord> {
        self.by_name.get(path).map(|&i| &self.records[i])
    }

    /// Whether an entry with the given path exists.
    pub fn contains(&self, path: &str) -> bool {
        self.by_name.contains_key(path)
    }

    /// Reads and decodes the data of the entry at index `i`.
    pub fn read_index(&self, i: usize) -> Result<Vec<u8>> {
        let rec = self
            .records
            .get(i)
            .ok_or_else(|| Error::NotFound(format!("#{i}")))?;
        self.read_record(rec)
    }

    /// Reads and decodes the data of the entry with the given path.
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let rec = self
            .find(path)
            .ok_or_else(|| Error::NotFound(path.to_owned()))?;
        self.read_record(rec)
    }

    /// Reads and fully verifies an entry's data given its record.
    pub fn read_record(&self, rec: &EntryRecord) -> Result<Vec<u8>> {
        if !rec.kind.has_data() || rec.original_len == 0 {
            return Ok(Vec::new());
        }

        // Parse the block header at the entry's data offset.
        let mut r = ByteReader::at(self.data, rec.data_offset as usize)?;
        let block = BlockHeader::read(&mut r)?;

        // The block header and the record must agree on the stored shape.
        if block.stored_len as u64 != rec.stored_len
            || block.original_len as u64 != rec.original_len
            || block.method != rec.compression
        {
            return Err(FormatError::BadBlockHeader.into());
        }

        let stored = r.read_bytes(block.stored_len as usize)?;

        // Verify the stored-bytes CRC before decoding.
        let stored_crc = Crc32::hash(stored);
        if stored_crc != block.crc32 {
            return Err(IntegrityError::BlockChecksum {
                expected: block.crc32,
                actual: stored_crc,
            }
            .into());
        }

        // Decode with a per-block output ceiling.
        let cap = self
            .limits
            .max_decoded_block
            .min(rec.original_len as usize + 1);
        let decoded = codec::decode(rec.compression, stored, cap)?;

        // The decoded length must match the record, and the content CRC too.
        if decoded.len() as u64 != rec.original_len {
            return Err(IntegrityError::LengthMismatch {
                expected: rec.original_len,
                actual: decoded.len() as u64,
            }
            .into());
        }
        let content_crc = Crc32::hash(&decoded);
        if content_crc != rec.crc32 {
            return Err(IntegrityError::BlockChecksum {
                expected: rec.crc32,
                actual: content_crc,
            }
            .into());
        }

        Ok(decoded)
    }

    /// The limits this archive was opened with.
    pub fn limits(&self) -> Limits {
        self.limits
    }
}

/// Validates that an entry's stored data range lies wholly between the header
/// and the index region.
fn validate_range(rec: &EntryRecord, index_offset: u64) -> Result<()> {
    if !rec.kind.has_data() || rec.stored_len == 0 {
        return Ok(());
    }
    let start = rec.data_offset;
    let end = start
        .checked_add(BLOCK_HEADER_LEN as u64)
        .and_then(|v| v.checked_add(rec.stored_len));
    match end {
        Some(end) if start >= HEADER_LEN as u64 && end <= index_offset => Ok(()),
        _ => Err(FormatError::EntryRangeOutOfRange {
            offset: rec.data_offset,
            len: rec.stored_len,
        }
        .into()),
    }
}

#[cfg(test)]
mod tests {
    // Full open/read behaviour is covered by the writer round-trip tests in
    // `crate::writer` and the integration tests; here we check the range
    // validator in isolation.
    use super::*;
    use crate::types::{Compression, EntryKind};

    fn rec(offset: u64, stored: u64) -> EntryRecord {
        EntryRecord {
            name: "x".into(),
            kind: EntryKind::File,
            compression: Compression::Store,
            data_offset: offset,
            stored_len: stored,
            original_len: stored,
            crc32: 0,
            mode: 0,
            mtime: 0,
        }
    }

    #[test]
    fn range_within_bounds_ok() {
        // header(20) + block(16) + 10 stored = ends at 46 <= index_offset 100.
        assert!(validate_range(&rec(20, 10), 100).is_ok());
    }

    #[test]
    fn range_overrunning_index_rejected() {
        assert!(validate_range(&rec(20, 1000), 100).is_err());
    }

    #[test]
    fn range_before_header_rejected() {
        assert!(validate_range(&rec(0, 10), 100).is_err());
    }

    #[test]
    fn overflowing_range_rejected() {
        assert!(validate_range(&rec(u64::MAX - 1, 10), 100).is_err());
    }
}
