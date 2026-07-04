//! Building archives.
//!
//! [`Builder`] stages entries in memory and serialises them into a complete
//! archive on [`finish`](Builder::finish). It handles path validation, per-entry
//! compression selection, CRC computation, and the two-pass layout (data blocks
//! first, then the index, with the header's index offset back-patched once the
//! block region size is known).

use std::collections::HashSet;

use crate::codec;
use crate::errors::{Error, Result};
use crate::format::{write_footer, BlockHeader, EntryRecord, FileHeader};
use crate::types::{Compression, EntryKind, EntryMeta};
use crate::util::path::normalize;
use crate::util::{ByteWriter, Crc32};

/// The compression policy applied to added entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionPolicy {
    /// Store everything uncompressed.
    None,
    /// Use exactly this method for every entry.
    Fixed(Compression),
    /// Pick, per entry, whichever method yields the smallest block.
    Auto,
}

struct Staged {
    meta: EntryMeta,
    data: Vec<u8>,
}

/// Stages entries and serialises them into an archive.
pub struct Builder {
    staged: Vec<Staged>,
    seen: HashSet<String>,
    policy: CompressionPolicy,
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

impl Builder {
    /// A new builder using the `Auto` compression policy.
    pub fn new() -> Self {
        Builder {
            staged: Vec::new(),
            seen: HashSet::new(),
            policy: CompressionPolicy::Auto,
        }
    }

    /// Sets the compression policy, returning `self` for chaining.
    pub fn with_policy(mut self, policy: CompressionPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// The number of staged entries.
    pub fn len(&self) -> usize {
        self.staged.len()
    }

    /// Whether no entries have been staged.
    pub fn is_empty(&self) -> bool {
        self.staged.is_empty()
    }

    /// Adds a regular file with default metadata.
    pub fn add_file(&mut self, path: &str, data: impl Into<Vec<u8>>) -> Result<()> {
        let path = normalize(path)?;
        self.add(EntryMeta::file(path), data.into())
    }

    /// Adds a regular file with explicit metadata. The metadata's path is
    /// normalized; its `original_len` is set from `data`.
    pub fn add_file_with(&mut self, mut meta: EntryMeta, data: impl Into<Vec<u8>>) -> Result<()> {
        meta.path = normalize(&meta.path)?;
        meta.kind = EntryKind::File;
        let data = data.into();
        meta.original_len = data.len() as u64;
        self.add(meta, data)
    }

    /// Adds a directory entry (carries no data).
    pub fn add_directory(&mut self, path: &str) -> Result<()> {
        let path = normalize(path)?;
        self.add(EntryMeta::directory(path), Vec::new())
    }

    /// Adds a symbolic link whose target is `target`.
    pub fn add_symlink(&mut self, path: &str, target: &str) -> Result<()> {
        let path = normalize(path)?;
        let mut meta = EntryMeta::file(path);
        meta.kind = EntryKind::Symlink;
        meta.mode = 0o777;
        self.add(meta, target.as_bytes().to_vec())
    }

    fn add(&mut self, mut meta: EntryMeta, data: Vec<u8>) -> Result<()> {
        if meta.path.len() > u16::MAX as usize {
            return Err(Error::LimitExceeded {
                limit: "name_len",
                cap: u16::MAX as u64,
                got: meta.path.len() as u64,
            });
        }
        if !self.seen.insert(meta.path.clone()) {
            return Err(Error::Usage("duplicate entry path"));
        }
        meta.original_len = data.len() as u64;
        self.staged.push(Staged { meta, data });
        Ok(())
    }

    /// Serialises all staged entries into a complete archive and returns the
    /// bytes. Consumes the builder.
    pub fn finish(self) -> Vec<u8> {
        let mut w = ByteWriter::with_capacity(1024);

        // Reserve the header; index_offset is patched once the blocks are written.
        // The index-offset field is the last 8 bytes of the 20-byte header.
        FileHeader::new(self.staged.len() as u32, 0).write(&mut w);
        let index_offset_slot = crate::format::HEADER_LEN - 8;

        // First pass: write data blocks, remembering each entry's record.
        let mut records = Vec::with_capacity(self.staged.len());
        for staged in &self.staged {
            let record = self.write_block(&mut w, staged);
            records.push(record);
        }

        // Second pass: the index region and its footer.
        let index_offset = w.offset();
        let (index_bytes, index_crc) = crate::format::encode_index(&records);
        w.write_bytes(&index_bytes);
        write_footer(&mut w, index_crc);

        // Back-patch the header's index offset.
        w.patch_u64(index_offset_slot, index_offset);
        w.into_vec()
    }

    /// Writes one entry's data block (if any) and returns its index record.
    fn write_block(&self, w: &mut ByteWriter, staged: &Staged) -> EntryRecord {
        let meta = &staged.meta;
        let content_crc = Crc32::hash(&staged.data);

        // Directories, and empty data entries, carry no block.
        if !meta.kind.has_data() || staged.data.is_empty() {
            return EntryRecord {
                name: meta.path.clone(),
                kind: meta.kind,
                compression: Compression::Store,
                data_offset: 0,
                stored_len: 0,
                original_len: staged.data.len() as u64,
                crc32: content_crc,
                mode: meta.mode,
                mtime: meta.mtime,
            };
        }

        let (method, stored) = self.compress(meta.compression, &staged.data);
        let stored_crc = Crc32::hash(&stored);
        let data_offset = w.offset();

        BlockHeader {
            method,
            stored_len: stored.len() as u32,
            original_len: staged.data.len() as u32,
            crc32: stored_crc,
        }
        .write(w);
        w.write_bytes(&stored);

        EntryRecord {
            name: meta.path.clone(),
            kind: meta.kind,
            compression: method,
            data_offset,
            stored_len: stored.len() as u64,
            original_len: staged.data.len() as u64,
            crc32: content_crc,
            mode: meta.mode,
            mtime: meta.mtime,
        }
    }

    /// Chooses the stored bytes for `data` per the builder's policy. `hint` is
    /// the entry's own preferred method, honoured when the policy defers to it.
    fn compress(&self, hint: Compression, data: &[u8]) -> (Compression, Vec<u8>) {
        match self.policy {
            CompressionPolicy::None => (Compression::Store, data.to_vec()),
            CompressionPolicy::Fixed(method) => (method, codec::encode(method, data)),
            CompressionPolicy::Auto => {
                // Respect a non-default per-entry hint; otherwise pick the best.
                if hint != Compression::Store {
                    (hint, codec::encode(hint, data))
                } else {
                    codec::best(data)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::Archive;

    #[test]
    fn builds_and_reads_back() {
        let mut b = Builder::new();
        b.add_file("hello.txt", b"hello world".to_vec()).unwrap();
        b.add_file("data.bin", vec![7u8; 5000]).unwrap();
        b.add_directory("emptydir").unwrap();
        let bytes = b.finish();

        let archive = Archive::open(&bytes).unwrap();
        assert_eq!(archive.len(), 3);
        assert_eq!(archive.read("hello.txt").unwrap(), b"hello world");
        assert_eq!(archive.read("data.bin").unwrap(), vec![7u8; 5000]);
        assert_eq!(archive.read("emptydir").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn rejects_duplicate_and_bad_paths() {
        let mut b = Builder::new();
        b.add_file("a.txt", b"1".to_vec()).unwrap();
        assert!(b.add_file("a.txt", b"2".to_vec()).is_err());
        assert!(b.add_file("../escape", b"x".to_vec()).is_err());
        assert!(b.add_file("/abs", b"x".to_vec()).is_err());
    }

    #[test]
    fn policies_all_roundtrip() {
        let data = b"ABCABCABCABCABCABC".repeat(50);
        for policy in [
            CompressionPolicy::None,
            CompressionPolicy::Fixed(Compression::Rle),
            CompressionPolicy::Fixed(Compression::Lz),
            CompressionPolicy::Auto,
        ] {
            let mut b = Builder::new().with_policy(policy);
            b.add_file("f", data.clone()).unwrap();
            let bytes = b.finish();
            let archive = Archive::open(&bytes).unwrap();
            assert_eq!(archive.read("f").unwrap(), data, "policy {policy:?}");
        }
    }

    #[test]
    fn symlink_roundtrips() {
        let mut b = Builder::new();
        b.add_symlink("link", "target/path").unwrap();
        let bytes = b.finish();
        let archive = Archive::open(&bytes).unwrap();
        assert_eq!(archive.read("link").unwrap(), b"target/path");
    }

    #[test]
    fn empty_archive_is_valid() {
        let bytes = Builder::new().finish();
        let archive = Archive::open(&bytes).unwrap();
        assert!(archive.is_empty());
    }
}
