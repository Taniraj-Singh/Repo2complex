//! Whole-archive integrity verification.
//!
//! Opening an [`Archive`] already validates the header,
//! index CRC, and every entry's data range. [`verify`] goes further: it decodes
//! *every* entry, exercising each block's CRC, decoded-length, and content-CRC
//! checks, and collects a [`Report`] of anything that fails — the archive
//! equivalent of `gzip -t`.

use crate::errors::Error;
use crate::reader::Archive;

/// The outcome of verifying an archive.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// The number of entries that decoded and verified cleanly.
    pub ok: usize,
    /// The entries that failed, paired with the error each hit.
    pub failures: Vec<(String, Error)>,
}

impl Report {
    /// Whether every entry verified successfully.
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }

    /// The total number of entries examined.
    pub fn checked(&self) -> usize {
        self.ok + self.failures.len()
    }
}

/// Verifies every entry in `archive`, returning a [`Report`]. Never fails as a
/// whole; per-entry failures are collected so a caller sees all corruption at
/// once rather than stopping at the first bad entry.
pub fn verify(archive: &Archive<'_>) -> Report {
    let mut report = Report::default();
    for (i, rec) in archive.records().iter().enumerate() {
        match archive.read_index(i) {
            Ok(_) => report.ok += 1,
            Err(e) => report.failures.push((rec.name.clone(), e)),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writer::Builder;

    #[test]
    fn clean_archive_verifies() {
        let mut b = Builder::new();
        b.add_file("a.txt", b"hello".to_vec()).unwrap();
        b.add_file("b.bin", vec![9u8; 2000]).unwrap();
        let bytes = b.finish();
        let archive = Archive::open(&bytes).unwrap();
        let report = verify(&archive);
        assert!(report.is_clean());
        assert_eq!(report.checked(), 2);
    }

    #[test]
    fn corrupted_data_is_detected() {
        let mut b = Builder::new();
        b.add_file("a.txt", vec![b'A'; 1000]).unwrap();
        let mut bytes = b.finish();

        // Flip a byte inside the data-block region (just past the 20-byte header
        // and 16-byte block header) to corrupt the stored payload.
        bytes[40] ^= 0xff;

        // Opening still succeeds (the index is intact); verification catches it.
        let archive = Archive::open(&bytes).unwrap();
        let report = verify(&archive);
        assert!(!report.is_clean());
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].0, "a.txt");
    }
}
