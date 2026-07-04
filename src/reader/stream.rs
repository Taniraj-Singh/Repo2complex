//! Sequential extraction over an opened [`Archive`].
//!
//! While [`Archive`] offers random access by path, extraction usually wants to
//! walk every entry in order. [`Extractor`] is a lazy iterator that yields each
//! entry's metadata paired with its decoded data (or the error decoding it hit),
//! decoding one entry at a time so peak memory stays bounded by the largest
//! single entry rather than the whole archive.

use crate::errors::Result;
use crate::reader::Archive;
use crate::types::EntryMeta;

/// A lazy iterator over `(metadata, decoded data)` for every entry.
pub struct Extractor<'a, 'arc> {
    archive: &'arc Archive<'a>,
    index: usize,
}

impl<'a, 'arc> Extractor<'a, 'arc> {
    /// Creates an extractor over `archive`.
    pub fn new(archive: &'arc Archive<'a>) -> Self {
        Extractor { archive, index: 0 }
    }
}

impl Iterator for Extractor<'_, '_> {
    type Item = Result<(EntryMeta, Vec<u8>)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.archive.len() {
            return None;
        }
        let i = self.index;
        self.index += 1;
        let meta = self.archive.records()[i].to_meta();
        Some(self.archive.read_index(i).map(|data| (meta, data)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.archive.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a> Archive<'a> {
    /// Returns a lazy extractor over all entries, in index order.
    pub fn extract_all(&self) -> Extractor<'a, '_> {
        Extractor::new(self)
    }
}
