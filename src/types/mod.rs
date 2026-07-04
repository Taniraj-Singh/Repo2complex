//! Core value types describing archive entries: [`EntryKind`], [`Compression`],
//! and [`EntryMeta`]. These are the domain vocabulary the format, reader,
//! writer, and higher layers all speak.

mod entry;

pub use entry::{Compression, EntryKind, EntryMeta};
