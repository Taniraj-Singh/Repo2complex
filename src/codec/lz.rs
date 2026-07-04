//! A small LZ77 / LZSS-style compressor with a bounded window.
//!
//! The stream is a sequence of groups. Each group begins with a **flag byte**
//! whose 8 bits (MSB first) describe up to 8 tokens:
//!
//! * flag bit `1` → a **literal**: the next input byte is copied verbatim.
//! * flag bit `0` → a **match**: the next two bytes pack a back-reference as a
//!   big-endian `u16`, `(distance-1) << 4 | (length - MIN_MATCH)`. So distances
//!   run 1..=4096 and lengths 3..=18.
//!
//! The encoder uses a hash-chain matcher for reasonable speed; the decoder is
//! the security-critical half and is written defensively: it validates every
//! back-reference against the bytes produced so far (rejecting underflow), never
//! reads out of bounds, and refuses to exceed a caller-supplied output ceiling
//! (defeating decompression bombs).

use crate::errors::{CodecError, Result};

/// The maximum back-reference distance (the sliding-window size).
const WINDOW: usize = 4096;
/// The shortest match worth encoding.
const MIN_MATCH: usize = 3;
/// The longest encodable match (`MIN_MATCH + 15`, length fits in 4 bits).
const MAX_MATCH: usize = MIN_MATCH + 15;
/// Size of the hash head table (power of two).
const HASH_BITS: u32 = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;
/// Maximum chain walk per position, trading ratio for speed.
const MAX_CHAIN: usize = 128;

fn hash3(a: u8, b: u8, c: u8) -> usize {
    let h = ((a as usize) << 10) ^ ((b as usize) << 5) ^ (c as usize);
    h & (HASH_SIZE - 1)
}

/// Encodes `input` with the LZ77 scheme.
pub fn encode(input: &[u8]) -> Vec<u8> {
    let n = input.len();
    let mut out = Vec::with_capacity(n / 2 + 1);
    if n == 0 {
        return out;
    }

    let mut head = vec![-1i32; HASH_SIZE];
    let mut prev = vec![-1i32; n];

    // Tokens are emitted in groups of 8 behind a flag byte. Reserve the first.
    out.push(0);
    let mut flag_pos = out.len() - 1;
    let mut flag = 0u8;
    let mut token_in_group = 0u8;

    let emit = |out: &mut Vec<u8>,
                flag: &mut u8,
                token_in_group: &mut u8,
                flag_pos: &mut usize,
                is_literal: bool,
                lit: u8,
                dist: usize,
                len: usize| {
        if *token_in_group == 8 {
            out[*flag_pos] = *flag;
            *flag = 0;
            *token_in_group = 0;
            out.push(0);
            *flag_pos = out.len() - 1;
        }
        if is_literal {
            *flag |= 1 << (7 - *token_in_group);
            out.push(lit);
        } else {
            let packed = (((dist - 1) as u16) << 4) | ((len - MIN_MATCH) as u16);
            out.push((packed >> 8) as u8);
            out.push((packed & 0xff) as u8);
        }
        *token_in_group += 1;
    };

    let mut pos = 0usize;
    while pos < n {
        let (mut best_len, mut best_dist) = (0usize, 0usize);
        if pos + MIN_MATCH <= n {
            let h = hash3(input[pos], input[pos + 1], input[pos + 2]);
            let mut cand = head[h];
            let mut depth = 0;
            while cand >= 0 && depth < MAX_CHAIN {
                let c = cand as usize;
                let dist = pos - c;
                if dist > WINDOW {
                    break;
                }
                let mlen = match_len(input, c, pos, n);
                if mlen > best_len {
                    best_len = mlen;
                    best_dist = dist;
                    if mlen >= MAX_MATCH {
                        break;
                    }
                }
                cand = prev[c];
                depth += 1;
            }
        }

        if best_len >= MIN_MATCH {
            emit(
                &mut out,
                &mut flag,
                &mut token_in_group,
                &mut flag_pos,
                false,
                0,
                best_dist,
                best_len,
            );
            // Insert all covered positions into the hash chains.
            let end = pos + best_len;
            while pos < end {
                if pos + MIN_MATCH <= n {
                    let h = hash3(input[pos], input[pos + 1], input[pos + 2]);
                    prev[pos] = head[h];
                    head[h] = pos as i32;
                }
                pos += 1;
            }
        } else {
            emit(
                &mut out,
                &mut flag,
                &mut token_in_group,
                &mut flag_pos,
                true,
                input[pos],
                0,
                0,
            );
            if pos + MIN_MATCH <= n {
                let h = hash3(input[pos], input[pos + 1], input[pos + 2]);
                prev[pos] = head[h];
                head[h] = pos as i32;
            }
            pos += 1;
        }
    }
    // Flush the final (possibly partial) flag byte.
    out[flag_pos] = flag;
    out
}

/// The length of the common prefix of `input[a..]` and `input[b..]`, capped at
/// `MAX_MATCH` and by the end of the buffer.
fn match_len(input: &[u8], a: usize, b: usize, n: usize) -> usize {
    let mut len = 0;
    while len < MAX_MATCH && b + len < n && input[a + len] == input[b + len] {
        len += 1;
    }
    len
}

/// Decodes an LZ77 stream, refusing to produce more than `max_output` bytes.
pub fn decode(input: &[u8], max_output: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < input.len() {
        let flag = input[i];
        i += 1;
        for bit in 0..8u8 {
            if i >= input.len() {
                // A clean end at a token boundary: remaining flag bits are unused.
                return Ok(out);
            }
            let is_literal = flag & (1 << (7 - bit)) != 0;
            if is_literal {
                if out.len() + 1 > max_output {
                    return Err(CodecError::OutputTooLarge { cap: max_output }.into());
                }
                out.push(input[i]);
                i += 1;
            } else {
                // A match needs two bytes.
                if input.len() - i < 2 {
                    return Err(CodecError::UnexpectedEnd.into());
                }
                let packed = ((input[i] as u16) << 8) | (input[i + 1] as u16);
                i += 2;
                let dist = ((packed >> 4) as usize) + 1;
                let len = ((packed & 0x0f) as usize) + MIN_MATCH;
                if dist > out.len() {
                    return Err(CodecError::BackrefUnderflow {
                        distance: dist,
                        produced: out.len(),
                    }
                    .into());
                }
                if out.len() + len > max_output {
                    return Err(CodecError::OutputTooLarge { cap: max_output }.into());
                }
                let start = out.len() - dist;
                for k in 0..len {
                    // Overlapping copies are valid and important (dist < len is
                    // how runs are expressed); each source byte is already
                    // present by the time it is read.
                    let byte = out[start + k];
                    out.push(byte);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(data: &[u8]) {
        let encoded = encode(data);
        let decoded = decode(&encoded, usize::MAX).unwrap();
        assert_eq!(decoded, data, "LZ roundtrip failed (len {})", data.len());
    }

    #[test]
    fn roundtrips_various() {
        roundtrip(b"");
        roundtrip(b"a");
        roundtrip(b"abcabcabcabcabc");
        roundtrip(b"the quick brown fox the quick brown fox");
        roundtrip(&[0u8; 5000]);
        roundtrip(&(0u8..=255).cycle().take(4000).collect::<Vec<_>>());
    }

    #[test]
    fn compresses_repetitive_data() {
        let data = b"ABCABCABCABCABCABCABCABCABCABCABCABC".repeat(20);
        let encoded = encode(&data);
        assert!(encoded.len() < data.len(), "expected compression");
        assert_eq!(decode(&encoded, usize::MAX).unwrap(), data);
    }

    #[test]
    fn overlapping_match_is_run() {
        // "aaaa..." must survive; internally encoded via dist=1 overlap copies.
        let data = vec![b'z'; 300];
        roundtrip(&data);
    }

    #[test]
    fn decode_rejects_backref_underflow() {
        // Flag says "match" (bit 0), then a match with distance 1 at output
        // position 0 — nothing has been produced yet.
        let stream = [0x00u8, 0x00, 0x00];
        assert!(matches!(
            decode(&stream, usize::MAX).unwrap_err(),
            crate::errors::Error::Codec(CodecError::BackrefUnderflow { .. })
        ));
    }

    #[test]
    fn decode_respects_output_cap() {
        let data = vec![7u8; 10_000];
        let encoded = encode(&data);
        assert!(matches!(
            decode(&encoded, 100).unwrap_err(),
            crate::errors::Error::Codec(CodecError::OutputTooLarge { .. })
        ));
    }

    #[test]
    fn truncated_match_errors() {
        // Flag says "match" but only one of the two match bytes is present.
        assert!(decode(&[0x00u8, 0x12], usize::MAX).is_err());
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        // A byte sweep: every decode must return, never panic.
        for a in 0u8..=255 {
            let _ = decode(&[a], 1 << 16);
            let _ = decode(&[a, a, a, a, a], 1 << 16);
        }
    }
}
