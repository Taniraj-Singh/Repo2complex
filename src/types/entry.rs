//! Entry kinds, compression methods, and per-entry metadata.

use crate::errors::{FormatError, Result};

/// What an archive entry represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EntryKind {
    /// A regular file with data.
    File = 0,
    /// A directory (no data; groups other entries by path prefix).
    Directory = 1,
    /// A symbolic link; its data is the (path) target.
    Symlink = 2,
}

impl EntryKind {
    /// Decodes a kind byte.
    pub fn from_byte(b: u8) -> Result<EntryKind> {
        Ok(match b {
            0 => EntryKind::File,
            1 => EntryKind::Directory,
            2 => EntryKind::Symlink,
            other => return Err(FormatError::UnknownEntryKind(other).into()),
        })
    }

    /// The kind's byte value.
    pub fn as_byte(self) -> u8 {
        self as u8
    }

    /// Whether an entry of this kind carries stored data.
    pub fn has_data(self) -> bool {
        matches!(self, EntryKind::File | EntryKind::Symlink)
    }

    /// A short human-facing label.
    pub fn label(self) -> &'static str {
        match self {
            EntryKind::File => "file",
            EntryKind::Directory => "dir",
            EntryKind::Symlink => "symlink",
        }
    }
}

/// How an entry's data block is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Compression {
    /// Raw, uncompressed bytes.
    Store = 0,
    /// Run-length encoding (good for sparse or repetitive data).
    Rle = 1,
    /// A small LZ77-style compressor with a bounded window.
    Lz = 2,
}

impl Compression {
    /// Decodes a compression byte.
    pub fn from_byte(b: u8) -> Result<Compression> {
        Ok(match b {
            0 => Compression::Store,
            1 => Compression::Rle,
            2 => Compression::Lz,
            other => return Err(FormatError::UnknownCompression(other).into()),
        })
    }

    /// The method's byte value.
    pub fn as_byte(self) -> u8 {
        self as u8
    }

    /// A short human-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Compression::Store => "store",
            Compression::Rle => "rle",
            Compression::Lz => "lz",
        }
    }
}

/// Metadata describing a single archive entry, independent of where its data
/// lives on disk. The on-disk [`EntryRecord`](crate::format::EntryRecord) pairs
/// this with the data offset and lengths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMeta {
    /// The entry's normalized, slash-separated path.
    pub path: String,
    /// What the entry is.
    pub kind: EntryKind,
    /// How its data is stored.
    pub compression: Compression,
    /// The original (uncompressed) data length.
    pub original_len: u64,
    /// Unix-style permission bits (only the low 12 bits are meaningful).
    pub mode: u32,
    /// Modification time as seconds since the Unix epoch.
    pub mtime: i64,
}

impl EntryMeta {
    /// Builds metadata for a stored file with default permissions.
    pub fn file(path: impl Into<String>) -> Self {
        EntryMeta {
            path: path.into(),
            kind: EntryKind::File,
            compression: Compression::Store,
            original_len: 0,
            mode: 0o644,
            mtime: 0,
        }
    }

    /// Builds metadata for a directory entry.
    pub fn directory(path: impl Into<String>) -> Self {
        EntryMeta {
            path: path.into(),
            kind: EntryKind::Directory,
            compression: Compression::Store,
            original_len: 0,
            mode: 0o755,
            mtime: 0,
        }
    }

    /// Returns a copy with the compression method set.
    pub fn with_compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }

    /// Returns a copy with the mode set.
    pub fn with_mode(mut self, mode: u32) -> Self {
        self.mode = mode;
        self
    }

    /// Returns a copy with the modification time set.
    pub fn with_mtime(mut self, mtime: i64) -> Self {
        self.mtime = mtime;
        self
    }

    /// The final path component (file name).
    pub fn name(&self) -> &str {
        crate::util::path::file_name(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_roundtrip() {
        for k in [EntryKind::File, EntryKind::Directory, EntryKind::Symlink] {
            assert_eq!(EntryKind::from_byte(k.as_byte()).unwrap(), k);
        }
        assert!(EntryKind::from_byte(9).is_err());
    }

    #[test]
    fn compression_roundtrip() {
        for c in [Compression::Store, Compression::Rle, Compression::Lz] {
            assert_eq!(Compression::from_byte(c.as_byte()).unwrap(), c);
        }
        assert!(Compression::from_byte(200).is_err());
    }

    #[test]
    fn builders_and_name() {
        let m = EntryMeta::file("a/b/c.txt")
            .with_compression(Compression::Lz)
            .with_mode(0o600)
            .with_mtime(42);
        assert_eq!(m.name(), "c.txt");
        assert_eq!(m.compression, Compression::Lz);
        assert_eq!(m.mode, 0o600);
        assert_eq!(m.mtime, 42);
        assert!(m.kind.has_data());
    }

    #[test]
    fn directory_has_no_data() {
        assert!(!EntryKind::Directory.has_data());
    }
}
