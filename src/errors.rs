//! Error types for the whole crate.
//!
//! Quiver uses one top-level [`Error`] with a variant per failure domain. Each
//! variant is rich enough to diagnose the problem — a bad magic number reports
//! the bytes it saw, a truncated read reports how many bytes it wanted versus
//! had — while the single top-level type keeps `?` ergonomic for callers.

use core::fmt;

/// The crate-wide result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// A byte offset within an archive, used to locate errors.
pub type Offset = u64;

/// Everything that can go wrong reading, writing, or decoding an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The container header, an entry record, or the footer was malformed.
    Format(FormatError),
    /// A read ran past the end of the available bytes.
    Truncated {
        /// Where the read started.
        at: Offset,
        /// How many bytes were needed.
        needed: usize,
        /// How many bytes were actually available.
        available: usize,
    },
    /// A stored checksum did not match the data it protects.
    Integrity(IntegrityError),
    /// A block codec rejected its input, or would produce oversized output.
    Codec(CodecError),
    /// A declared size or count exceeded a configured safety limit.
    LimitExceeded {
        /// The name of the limit, e.g. `"max_entries"`.
        limit: &'static str,
        /// The configured ceiling.
        cap: u64,
        /// The value that breached it.
        got: u64,
    },
    /// A named entry or path was not found in the archive.
    NotFound(String),
    /// An entry path was not valid (absolute, contained `..`, or empty).
    InvalidPath(String),
    /// A caller used the API incorrectly (e.g. finished a builder twice). This
    /// signals a bug in caller code, not bad archive data.
    Usage(&'static str),
}

impl Error {
    /// A short, stable machine code for the error class.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Format(_) => "format",
            Error::Truncated { .. } => "truncated",
            Error::Integrity(_) => "integrity",
            Error::Codec(_) => "codec",
            Error::LimitExceeded { .. } => "limit",
            Error::NotFound(_) => "not_found",
            Error::InvalidPath(_) => "invalid_path",
            Error::Usage(_) => "usage",
        }
    }

    /// Whether the error was caused by malformed *archive data* rather than by
    /// API misuse. A reader of untrusted archives treats these as "reject this
    /// file", and everything else as a bug to fix.
    pub fn is_data_error(&self) -> bool {
        matches!(
            self,
            Error::Format(_)
                | Error::Truncated { .. }
                | Error::Integrity(_)
                | Error::Codec(_)
                | Error::LimitExceeded { .. }
                | Error::InvalidPath(_)
        )
    }

    /// Convenience constructor for [`Error::Truncated`].
    pub fn truncated(at: Offset, needed: usize, available: usize) -> Self {
        Error::Truncated {
            at,
            needed,
            available,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Format(e) => write!(f, "format error: {e}"),
            Error::Truncated {
                at,
                needed,
                available,
            } => write!(
                f,
                "truncated at offset {at}: needed {needed} bytes, had {available}"
            ),
            Error::Integrity(e) => write!(f, "integrity error: {e}"),
            Error::Codec(e) => write!(f, "codec error: {e}"),
            Error::LimitExceeded { limit, cap, got } => {
                write!(f, "limit `{limit}` exceeded: {got} > {cap}")
            }
            Error::NotFound(name) => write!(f, "entry `{name}` not found"),
            Error::InvalidPath(p) => write!(f, "invalid entry path `{p}`"),
            Error::Usage(msg) => write!(f, "misuse: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// Structural problems with the on-disk layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The file did not start with the archive magic.
    BadMagic {
        /// The first bytes actually found.
        found: [u8; 4],
    },
    /// The footer magic was wrong or absent.
    BadFooterMagic,
    /// The format version is newer than this reader understands.
    UnsupportedVersion(u8),
    /// A reserved header field held a non-zero/unexpected value.
    ReservedField,
    /// An entry record named an unknown entry kind byte.
    UnknownEntryKind(u8),
    /// An entry record named an unknown compression method byte.
    UnknownCompression(u8),
    /// The index offset pointed outside the file.
    IndexOutOfRange {
        /// The offset the header claimed.
        offset: Offset,
        /// The size of the archive.
        size: u64,
    },
    /// An entry's data range `[offset, offset+len)` fell outside the file.
    EntryRangeOutOfRange {
        /// The entry's data offset.
        offset: Offset,
        /// The entry's stored length.
        len: u64,
    },
    /// A string field was not valid UTF-8.
    InvalidUtf8,
    /// A block header was malformed.
    BadBlockHeader,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::BadMagic { found } => {
                write!(f, "bad archive magic: {found:02x?}")
            }
            FormatError::BadFooterMagic => write!(f, "bad footer magic"),
            FormatError::UnsupportedVersion(v) => write!(f, "unsupported version {v}"),
            FormatError::ReservedField => write!(f, "reserved field was not zero"),
            FormatError::UnknownEntryKind(k) => write!(f, "unknown entry kind 0x{k:02x}"),
            FormatError::UnknownCompression(c) => {
                write!(f, "unknown compression method 0x{c:02x}")
            }
            FormatError::IndexOutOfRange { offset, size } => {
                write!(f, "index offset {offset} outside archive of {size} bytes")
            }
            FormatError::EntryRangeOutOfRange { offset, len } => {
                write!(
                    f,
                    "entry data range [{offset}, {offset}+{len}) out of range"
                )
            }
            FormatError::InvalidUtf8 => write!(f, "string field is not valid UTF-8"),
            FormatError::BadBlockHeader => write!(f, "malformed block header"),
        }
    }
}

/// Checksum mismatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityError {
    /// A data block's CRC did not match.
    BlockChecksum {
        /// The CRC stored in the block header.
        expected: u32,
        /// The CRC computed over the data.
        actual: u32,
    },
    /// The index's CRC (in the footer) did not match.
    IndexChecksum {
        /// The stored CRC.
        expected: u32,
        /// The computed CRC.
        actual: u32,
    },
    /// A decoded entry's length did not match the record's declared length.
    LengthMismatch {
        /// The declared original length.
        expected: u64,
        /// The length actually produced by decoding.
        actual: u64,
    },
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntegrityError::BlockChecksum { expected, actual } => write!(
                f,
                "block checksum mismatch: expected {expected:#010x}, got {actual:#010x}"
            ),
            IntegrityError::IndexChecksum { expected, actual } => write!(
                f,
                "index checksum mismatch: expected {expected:#010x}, got {actual:#010x}"
            ),
            IntegrityError::LengthMismatch { expected, actual } => {
                write!(f, "length mismatch: expected {expected}, decoded {actual}")
            }
        }
    }
}

/// Failures decoding a compressed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// A back-reference pointed before the start of the output.
    BackrefUnderflow {
        /// The distance the token requested.
        distance: usize,
        /// How many bytes had been produced so far.
        produced: usize,
    },
    /// The compressed stream ended in the middle of a token.
    UnexpectedEnd,
    /// Decoding would produce more than the allowed number of bytes (a
    /// decompression bomb).
    OutputTooLarge {
        /// The configured output ceiling.
        cap: usize,
    },
    /// A control/token byte was not recognised.
    BadToken(u8),
    /// A run-length or copy length of zero, which no valid encoder emits.
    ZeroLength,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::BackrefUnderflow { distance, produced } => write!(
                f,
                "back-reference distance {distance} exceeds {produced} produced bytes"
            ),
            CodecError::UnexpectedEnd => write!(f, "compressed stream ended unexpectedly"),
            CodecError::OutputTooLarge { cap } => {
                write!(f, "decoded output would exceed {cap} bytes")
            }
            CodecError::BadToken(b) => write!(f, "invalid token byte 0x{b:02x}"),
            CodecError::ZeroLength => write!(f, "zero-length run or copy"),
        }
    }
}

macro_rules! from_domain {
    ($($variant:ident => $ty:ty),+ $(,)?) => {
        $(impl From<$ty> for Error {
            fn from(e: $ty) -> Self { Error::$variant(e) }
        })+
    };
}

from_domain! {
    Format => FormatError,
    Integrity => IntegrityError,
    Codec => CodecError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(Error::from(FormatError::BadFooterMagic).code(), "format");
        assert_eq!(Error::truncated(0, 4, 1).code(), "truncated");
        assert_eq!(Error::Usage("x").code(), "usage");
    }

    #[test]
    fn data_errors_classified() {
        assert!(Error::from(FormatError::InvalidUtf8).is_data_error());
        assert!(Error::truncated(0, 4, 0).is_data_error());
        assert!(!Error::Usage("boom").is_data_error());
        assert!(!Error::NotFound("x".into()).is_data_error());
    }

    #[test]
    fn display_mentions_details() {
        let e = Error::from(IntegrityError::BlockChecksum {
            expected: 0xdead_beef,
            actual: 0,
        });
        assert!(format!("{e}").contains("deadbeef"));
    }
}
