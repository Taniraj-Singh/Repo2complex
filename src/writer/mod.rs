//! Writing archives.
//!
//! [`Builder`] stages entries and serialises them with [`Builder::finish`]. The
//! [`CompressionPolicy`] controls how each entry's data block is stored.
//!
//! ```
//! use quiver::writer::{Builder, CompressionPolicy};
//! use quiver::types::Compression;
//!
//! let bytes = {
//!     let mut b = Builder::new().with_policy(CompressionPolicy::Fixed(Compression::Lz));
//!     b.add_file("readme.md", b"# hello".to_vec()).unwrap();
//!     b.finish()
//! };
//! assert!(bytes.starts_with(b"QVR1"));
//! ```

mod builder;

pub use builder::{Builder, CompressionPolicy};
