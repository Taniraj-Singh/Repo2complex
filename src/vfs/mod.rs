//! A virtual filesystem view over an archive.
//!
//! Archive entries are stored as a flat list of paths; [`Tree`] reconstructs the
//! directory hierarchy they describe so callers can list directories, resolve
//! paths, and walk the archive like a filesystem.

mod tree;

pub use tree::{Node, Tree};
