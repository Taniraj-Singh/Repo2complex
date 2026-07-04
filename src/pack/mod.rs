//! The high-level façade.
//!
//! [`Pack`] ties the reader, writer, VFS, catalog, and verifier together behind
//! a small convenience API for the common cases: build an archive from a set of
//! files, open one and browse it, or open-and-verify in a single call. Advanced
//! users can still reach for [`Builder`] and
//! [`Archive`] directly.

use crate::catalog::{Catalog, Summary};
use crate::errors::Result;
use crate::integrity::{self, Report};
use crate::reader::Archive;
use crate::types::EntryMeta;
use crate::util::Limits;
use crate::vfs::Tree;
use crate::writer::{Builder, CompressionPolicy};

/// A convenience builder for creating archives from `(path, data)` pairs.
pub struct Pack;

impl Pack {
    /// Builds an archive from an iterator of `(path, data)` pairs using the
    /// automatic compression policy.
    pub fn create<I, P, D>(entries: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = (P, D)>,
        P: AsRef<str>,
        D: Into<Vec<u8>>,
    {
        let mut builder = Builder::new();
        for (path, data) in entries {
            builder.add_file(path.as_ref(), data.into())?;
        }
        Ok(builder.finish())
    }

    /// Builds an archive from `(path, data)` pairs with an explicit compression
    /// policy.
    pub fn create_with<I, P, D>(policy: CompressionPolicy, entries: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = (P, D)>,
        P: AsRef<str>,
        D: Into<Vec<u8>>,
    {
        let mut builder = Builder::new().with_policy(policy);
        for (path, data) in entries {
            builder.add_file(path.as_ref(), data.into())?;
        }
        Ok(builder.finish())
    }

    /// Opens an archive for browsing with the default limits.
    pub fn open(bytes: &[u8]) -> Result<Archive<'_>> {
        Archive::open(bytes)
    }

    /// Opens an archive with explicit limits (for untrusted input).
    pub fn open_with(bytes: &[u8], limits: Limits) -> Result<Archive<'_>> {
        Archive::open_with(bytes, limits)
    }

    /// Opens an archive and verifies every entry, returning both the archive and
    /// the verification [`Report`].
    pub fn open_verified(bytes: &[u8]) -> Result<(Archive<'_>, Report)> {
        let archive = Archive::open(bytes)?;
        let report = integrity::verify(&archive);
        Ok((archive, report))
    }

    /// Opens an archive and returns its aggregate [`Summary`].
    pub fn summary(bytes: &[u8]) -> Result<Summary> {
        let archive = Archive::open(bytes)?;
        Ok(Catalog::new(&archive).summary())
    }

    /// Opens an archive and returns its virtual directory [`Tree`].
    pub fn tree(bytes: &[u8]) -> Result<Tree> {
        let archive = Archive::open(bytes)?;
        Tree::build(archive.records(), &archive.limits())
    }

    /// Opens an archive and extracts every entry into `(metadata, data)` pairs.
    /// Intended for small archives; for large ones prefer
    /// [`Archive::extract_all`] to keep memory bounded.
    pub fn extract_all(bytes: &[u8]) -> Result<Vec<(EntryMeta, Vec<u8>)>> {
        let archive = Archive::open(bytes)?;
        archive.extract_all().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_open_extract() {
        let bytes = Pack::create([
            ("a.txt", b"alpha".to_vec()),
            ("dir/b.txt", b"beta".to_vec()),
        ])
        .unwrap();

        let extracted = Pack::extract_all(&bytes).unwrap();
        assert_eq!(extracted.len(), 2);
        assert_eq!(extracted[0].0.path, "a.txt");
        assert_eq!(extracted[0].1, b"alpha");
    }

    #[test]
    fn open_verified_reports_clean() {
        let bytes = Pack::create([("x", vec![1u8; 500])]).unwrap();
        let (_archive, report) = Pack::open_verified(&bytes).unwrap();
        assert!(report.is_clean());
    }

    #[test]
    fn summary_and_tree() {
        let bytes = Pack::create([
            ("src/main.rs", vec![b'a'; 100]),
            ("src/lib.rs", vec![b'b'; 100]),
        ])
        .unwrap();
        assert_eq!(Pack::summary(&bytes).unwrap().files, 2);
        let tree = Pack::tree(&bytes).unwrap();
        assert!(tree.get("src").unwrap().is_dir);
    }
}
