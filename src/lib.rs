//! # Quiver
//!
//! Quiver is a small, dependency-free library for a **seekable binary archive
//! container format** — think of a minimal, well-specified `tar`/`zip` with its
//! own block compression. It can build archives from a set of named entries and
//! read them back with random access, streaming extraction, a virtual directory
//! view, catalog queries, and whole-archive integrity verification.
//!
//! ## Layout of the format
//!
//! ```text
//!   [FileHeader]        magic "QVR1", version, entry_count, index_offset
//!   [data blocks...]    each: BlockHeader ("QB") + stored bytes
//!   [index region]      entry_count EntryRecords (name, offsets, CRCs, mode)
//!   [footer]            index CRC + magic "QVRX"
//! ```
//!
//! See [`docs/FORMAT.md`] for the byte-level specification.
//!
//! ## Quick start
//!
//! ```
//! use quiver::pack::Pack;
//!
//! // Build an archive from named blobs.
//! let bytes = Pack::create([
//!     ("readme.txt", b"hello".to_vec()),
//!     ("data/nums.bin", (0u8..200).collect::<Vec<_>>()),
//! ]).unwrap();
//!
//! // Open it and read an entry back.
//! let archive = Pack::open(&bytes).unwrap();
//! assert_eq!(archive.read("readme.txt").unwrap(), b"hello");
//!
//! // Verify every entry's checksums.
//! let (_, report) = Pack::open_verified(&bytes).unwrap();
//! assert!(report.is_clean());
//! ```
//!
//! ## Safety on untrusted input
//!
//! Reading an archive is the crate's trust boundary. Every parse and decode
//! path is bounded by a [`util::Limits`] budget (entry count, name length,
//! decompressed size, path depth), validates all offsets and lengths before
//! use, and verifies CRCs. No archive — however malformed or hostile — can make
//! the reader panic, read out of bounds, loop forever, or allocate unboundedly;
//! it returns an [`errors::Error`] instead. The [`errors::Error::is_data_error`]
//! method distinguishes "reject this file" from "fix your code". These
//! guarantees are exercised continuously by the fuzz targets under `fuzz/`.
//!
//! [`docs/FORMAT.md`]: https://github.com/quiver-archive/quiver/blob/main/docs/FORMAT.md

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod catalog;
pub mod codec;
pub mod errors;
pub mod format;
pub mod integrity;
pub mod pack;
pub mod reader;
pub mod types;
pub mod util;
pub mod vfs;
pub mod writer;

pub use crate::errors::{Error, Result};
pub use crate::pack::Pack;
pub use crate::reader::Archive;
pub use crate::types::{Compression, EntryKind, EntryMeta};
pub use crate::writer::{Builder, CompressionPolicy};

/// The crate version, from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
