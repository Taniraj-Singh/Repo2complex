//! Reading archives.
//!
//! [`Archive`] parses and validates an archive from a borrowed byte buffer and
//! offers random access by path; [`Extractor`] walks every entry sequentially,
//! decoding one at a time. Both are total over untrusted input — malformed
//! bytes always yield an [`Error`](crate::errors::Error), never a panic.
//!
//! ```
//! # use quiver::writer::Builder;
//! # use quiver::reader::Archive;
//! # let bytes = { let mut b = Builder::new(); b.add_file("hello.txt", b"hi").unwrap(); b.finish() };
//! let archive = Archive::open(&bytes).unwrap();
//! assert_eq!(archive.read("hello.txt").unwrap(), b"hi");
//! ```

mod archive;
mod stream;

pub use archive::Archive;
pub use stream::Extractor;
