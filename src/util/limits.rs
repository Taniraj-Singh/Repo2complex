//! Resource limits applied when reading untrusted archives.
//!
//! Every quantity a hostile archive could inflate — entry count, name length,
//! declared decompressed size, nesting of the virtual path tree — is bounded by
//! a shared [`Limits`]. Centralising them makes the crate's safety envelope
//! auditable in one place and lets a fuzz harness tighten every knob at once.

/// Configurable ceilings for archive parsing and decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum number of entries an archive index may declare.
    pub max_entries: u64,
    /// Maximum length, in bytes, of an entry name.
    pub max_name_len: u32,
    /// Maximum decompressed size of a single block. This is the primary
    /// decompression-bomb guard.
    pub max_decoded_block: usize,
    /// Maximum total decompressed size across a whole extraction.
    pub max_total_decoded: u64,
    /// Maximum depth of the virtual directory tree built from entry paths.
    pub max_path_depth: usize,
}

impl Limits {
    /// Generous production defaults.
    pub const DEFAULT: Limits = Limits {
        max_entries: 1 << 20,
        max_name_len: 4096,
        max_decoded_block: 64 << 20,
        max_total_decoded: 4 << 30,
        max_path_depth: 64,
    };

    /// Tight limits for fuzzing and for tests that want to trip a bound cheaply.
    pub const STRICT: Limits = Limits {
        max_entries: 4096,
        max_name_len: 512,
        max_decoded_block: 1 << 20,
        max_total_decoded: 16 << 20,
        max_path_depth: 32,
    };

    /// Builder-style override for the decoded-block ceiling.
    pub fn with_max_decoded_block(mut self, cap: usize) -> Self {
        self.max_decoded_block = cap;
        self
    }

    /// Builder-style override for the entry-count ceiling.
    pub fn with_max_entries(mut self, cap: u64) -> Self {
        self.max_entries = cap;
        self
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn strict_is_tighter() {
        assert!(Limits::STRICT.max_entries < Limits::DEFAULT.max_entries);
        assert!(Limits::STRICT.max_decoded_block < Limits::DEFAULT.max_decoded_block);
    }

    #[test]
    fn builders_override() {
        let l = Limits::DEFAULT
            .with_max_entries(10)
            .with_max_decoded_block(99);
        assert_eq!(l.max_entries, 10);
        assert_eq!(l.max_decoded_block, 99);
    }
}
