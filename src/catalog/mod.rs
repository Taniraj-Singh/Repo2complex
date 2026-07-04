//! High-level queries over an opened archive.
//!
//! [`Catalog`] borrows an [`Archive`] and answers the
//! questions an archive browser or extraction tool asks: how big is everything,
//! how well did it compress, which entries match a predicate, what are the
//! largest files. It computes nothing at open time — each query walks the
//! already-parsed records.

use crate::format::EntryRecord;
use crate::reader::Archive;
use crate::types::EntryKind;

/// A read-only query interface over an archive's entries.
pub struct Catalog<'r, 'a> {
    archive: &'r Archive<'a>,
}

/// Aggregate statistics describing an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    /// Number of file entries.
    pub files: usize,
    /// Number of directory entries.
    pub directories: usize,
    /// Number of symlink entries.
    pub symlinks: usize,
    /// Total original (uncompressed) bytes across all entries.
    pub original_bytes: u64,
    /// Total stored (on-disk) bytes across all entries.
    pub stored_bytes: u64,
}

impl Summary {
    /// The overall compression ratio (stored / original), in `[0, 1]`. Returns
    /// `1.0` for an empty or all-directory archive.
    pub fn ratio(&self) -> f64 {
        if self.original_bytes == 0 {
            1.0
        } else {
            self.stored_bytes as f64 / self.original_bytes as f64
        }
    }

    /// The number of bytes saved by compression.
    pub fn saved_bytes(&self) -> u64 {
        self.original_bytes.saturating_sub(self.stored_bytes)
    }

    /// The total number of entries.
    pub fn entries(&self) -> usize {
        self.files + self.directories + self.symlinks
    }
}

impl<'r, 'a> Catalog<'r, 'a> {
    /// Creates a catalog over `archive`.
    pub fn new(archive: &'r Archive<'a>) -> Self {
        Catalog { archive }
    }

    /// Computes the aggregate [`Summary`].
    pub fn summary(&self) -> Summary {
        let mut s = Summary::default();
        for rec in self.archive.records() {
            match rec.kind {
                EntryKind::File => s.files += 1,
                EntryKind::Directory => s.directories += 1,
                EntryKind::Symlink => s.symlinks += 1,
            }
            s.original_bytes = s.original_bytes.saturating_add(rec.original_len);
            s.stored_bytes = s.stored_bytes.saturating_add(rec.stored_len);
        }
        s
    }

    /// Returns records for which `predicate` holds, in index order.
    pub fn filter<F: Fn(&EntryRecord) -> bool>(&self, predicate: F) -> Vec<&'r EntryRecord> {
        self.archive
            .records()
            .iter()
            .filter(|r| predicate(r))
            .collect()
    }

    /// Returns file entries whose name ends with `.{ext}` (case-sensitive).
    pub fn by_extension(&self, ext: &str) -> Vec<&'r EntryRecord> {
        let suffix = format!(".{ext}");
        self.filter(|r| r.kind == EntryKind::File && r.name.ends_with(&suffix))
    }

    /// Returns file entries under the directory prefix `dir` (e.g. `"src"`),
    /// including nested ones.
    pub fn under(&self, dir: &str) -> Vec<&'r EntryRecord> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        self.filter(|r| r.name.starts_with(&prefix))
    }

    /// Returns the `n` largest file entries by original size, descending.
    pub fn largest(&self, n: usize) -> Vec<&'r EntryRecord> {
        let mut files: Vec<&EntryRecord> = self
            .archive
            .records()
            .iter()
            .filter(|r| r.kind == EntryKind::File)
            .collect();
        files.sort_by_key(|r| std::cmp::Reverse(r.original_len));
        files.truncate(n);
        files
    }

    /// The total original size of all entries.
    pub fn total_original(&self) -> u64 {
        self.summary().original_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::{Builder, CompressionPolicy};

    fn sample() -> Vec<u8> {
        let mut b =
            Builder::new().with_policy(CompressionPolicy::Fixed(crate::types::Compression::Rle));
        b.add_file("src/main.rs", vec![b'x'; 900]).unwrap();
        b.add_file("src/lib.rs", vec![b'y'; 300]).unwrap();
        b.add_file("README.md", b"# hi".to_vec()).unwrap();
        b.add_directory("src").unwrap();
        b.finish()
    }

    #[test]
    fn summary_counts_and_sizes() {
        let bytes = sample();
        let archive = Archive::open(&bytes).unwrap();
        let cat = Catalog::new(&archive);
        let s = cat.summary();
        assert_eq!(s.files, 3);
        assert_eq!(s.directories, 1);
        assert_eq!(s.original_bytes, 900 + 300 + 4);
        // Highly repetitive data should compress well.
        assert!(s.stored_bytes < s.original_bytes);
        assert!(s.ratio() < 1.0);
    }

    #[test]
    fn queries_filter_and_sort() {
        let bytes = sample();
        let archive = Archive::open(&bytes).unwrap();
        let cat = Catalog::new(&archive);

        assert_eq!(cat.by_extension("rs").len(), 2);
        assert_eq!(cat.under("src").len(), 2);

        let largest = cat.largest(1);
        assert_eq!(largest.len(), 1);
        assert_eq!(largest[0].name, "src/main.rs");
    }
}
