//! Run-length encoding.
//!
//! A compact byte-oriented RLE with a control-byte scheme:
//!
//! * A control byte `< 0x80` starts a **literal run**: its value + 1 is the
//!   number of raw bytes that follow (1..=128).
//! * A control byte `>= 0x80` is a **repeat run**: the low 7 bits + 1 is the run
//!   length (1..=128), and the next byte is the value to repeat.
//!
//! Decoding is bounded by a caller-supplied output ceiling so that a hostile
//! stream cannot expand into unbounded memory.

use crate::errors::{CodecError, Result};

/// The largest run a single control byte can express.
const MAX_RUN: usize = 128;

/// Encodes `input` with RLE.
pub fn encode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() / 2 + 1);
    let mut i = 0;
    while i < input.len() {
        // Measure the run of equal bytes starting at `i`.
        let byte = input[i];
        let mut run = 1;
        while i + run < input.len() && input[i + run] == byte && run < MAX_RUN {
            run += 1;
        }
        if run >= 3 {
            // Worth a repeat run.
            out.push(0x80 | (run as u8 - 1));
            out.push(byte);
            i += run;
        } else {
            // Accumulate a literal run up to the next promising repeat.
            let start = i;
            let mut lit = 0;
            while i < input.len() && lit < MAX_RUN {
                // Stop the literal run if a length-3 repeat begins here.
                if i + 2 < input.len() && input[i] == input[i + 1] && input[i + 1] == input[i + 2] {
                    break;
                }
                i += 1;
                lit += 1;
            }
            out.push(lit as u8 - 1);
            out.extend_from_slice(&input[start..start + lit]);
        }
    }
    out
}

/// Decodes an RLE stream, refusing to produce more than `max_output` bytes.
pub fn decode(input: &[u8], max_output: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let control = input[i];
        i += 1;
        if control & 0x80 == 0 {
            // Literal run of (control + 1) bytes.
            let count = control as usize + 1;
            let end = i
                .checked_add(count)
                .filter(|&e| e <= input.len())
                .ok_or(CodecError::UnexpectedEnd)?;
            guard(&out, count, max_output)?;
            out.extend_from_slice(&input[i..end]);
            i = end;
        } else {
            // Repeat run of (low7 + 1) copies of the next byte.
            let count = (control & 0x7f) as usize + 1;
            let value = *input.get(i).ok_or(CodecError::UnexpectedEnd)?;
            i += 1;
            guard(&out, count, max_output)?;
            out.resize(out.len() + count, value);
        }
    }
    Ok(out)
}

fn guard(out: &[u8], adding: usize, max_output: usize) -> Result<()> {
    if out.len() + adding > max_output {
        return Err(CodecError::OutputTooLarge { cap: max_output }.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(data: &[u8]) {
        let encoded = encode(data);
        let decoded = decode(&encoded, usize::MAX).unwrap();
        assert_eq!(decoded, data, "roundtrip failed for {data:?}");
    }

    #[test]
    fn roundtrips_various() {
        roundtrip(b"");
        roundtrip(b"a");
        roundtrip(b"aaaaaaa");
        roundtrip(b"abcdefg");
        roundtrip(b"aaabbbcccddd");
        roundtrip(&[0u8; 500]);
        roundtrip(&(0u8..=255).collect::<Vec<_>>());
    }

    #[test]
    fn long_run_spans_multiple_controls() {
        let data = vec![7u8; 1000];
        let decoded = decode(&encode(&data), usize::MAX).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn decode_respects_output_cap() {
        // A repeat run of 128 bytes, but a cap of 10.
        let encoded = encode(&[0xAAu8; 128]);
        assert!(matches!(
            decode(&encoded, 10).unwrap_err(),
            crate::errors::Error::Codec(CodecError::OutputTooLarge { .. })
        ));
    }

    #[test]
    fn truncated_streams_error_not_panic() {
        // Literal control claiming 5 bytes but none follow.
        assert!(decode(&[0x04], usize::MAX).is_err());
        // Repeat control with no value byte.
        assert!(decode(&[0x80], usize::MAX).is_err());
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        for a in 0u8..=255 {
            let _ = decode(&[a], 4096);
            let _ = decode(&[a, 0x00, 0x01], 4096);
        }
    }
}
