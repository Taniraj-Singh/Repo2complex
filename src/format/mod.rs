//! The on-disk archive format: fixed-layout headers, records, and footer.
//!
//! This module is the single source of truth for the byte layout, shared by the
//! [reader](crate::reader) and [writer](crate::writer). An archive is:
//!
//! ```text
//!   [FileHeader]            fixed 20 bytes
//!   [data blocks...]        each: BlockHeader (16 B) + stored bytes
//!   [index region]          entry_count EntryRecords, at header.index_offset
//!   [footer]                index_crc (4 B) + "QVRX"
//! ```
//!
//! See [`docs/FORMAT.md`](https://github.com/quiver-archive/quiver/blob/main/docs/FORMAT.md)
//! for the full specification.

mod block;
mod header;
mod index;
mod record;

pub use block::{BlockHeader, BLOCK_HEADER_LEN, BLOCK_MAGIC};
pub use header::{FileHeader, HEADER_LEN, MAGIC, VERSION};
pub use index::{decode_index, encode_index, read_footer, write_footer, FOOTER_LEN, FOOTER_MAGIC};
pub use record::{EntryRecord, RECORD_FIXED_LEN};
