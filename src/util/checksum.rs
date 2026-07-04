//! Checksums used to detect corruption.
//!
//! Data blocks and the index are protected by CRC-32 (IEEE polynomial). A
//! faster [`adler32`] is also provided for callers that want a cheap rolling
//! integrity check on large streams. Neither is a security primitive — they
//! guard against accidental corruption, not tampering.

/// A streaming CRC-32 (IEEE 802.3, reflected).
#[derive(Debug, Clone)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Crc32::new()
    }
}

impl Crc32 {
    /// A fresh accumulator.
    pub fn new() -> Self {
        Crc32 { state: !0 }
    }

    /// Feeds bytes into the accumulator.
    pub fn update(&mut self, data: &[u8]) {
        let table = table();
        let mut crc = self.state;
        for &b in data {
            crc = (crc >> 8) ^ table[((crc ^ b as u32) & 0xff) as usize];
        }
        self.state = crc;
    }

    /// Finalises and returns the checksum.
    pub fn finish(self) -> u32 {
        !self.state
    }

    /// One-shot CRC-32 of a single buffer.
    pub fn hash(data: &[u8]) -> u32 {
        let mut c = Crc32::new();
        c.update(data);
        c.finish()
    }
}

fn table() -> &'static [u32; 256] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            t[i] = c;
            i += 1;
        }
        t
    })
}

/// The Adler-32 modulus (largest prime below 65536).
const ADLER_MOD: u32 = 65521;

/// Computes the Adler-32 checksum of `data`.
pub fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    // Process in chunks so the running sums cannot overflow before the modulo.
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= ADLER_MOD;
        b %= ADLER_MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_known_vector() {
        // "123456789" -> 0xCBF43926 is the canonical CRC-32 check value.
        assert_eq!(Crc32::hash(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc_streaming_matches_oneshot() {
        let mut c = Crc32::new();
        c.update(b"1234");
        c.update(b"56789");
        assert_eq!(c.finish(), Crc32::hash(b"123456789"));
    }

    #[test]
    fn adler_known_vector() {
        // Adler-32 of "Wikipedia" is 0x11E60398.
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn distinct_inputs_differ() {
        assert_ne!(Crc32::hash(b"a"), Crc32::hash(b"b"));
        assert_ne!(adler32(b"a"), adler32(b"b"));
    }
}
