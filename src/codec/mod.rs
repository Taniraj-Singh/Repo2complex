//! Block codecs: the compression methods a data block can use.
//!
//! Each [`Compression`] method has an `encode`/`decode` pair. Encoding is
//! infallible; decoding takes untrusted bytes and a hard output ceiling, and is
//! the crate's main decompression attack surface. The submodules
//! ([`rle`], [`lz`]) implement the two non-trivial methods; `Store` is a raw
//! copy handled here.
//!
//! ```
//! use quiver::codec::{encode, decode};
//! use quiver::types::Compression;
//!
//! let data = b"aaaaaabbbbbbcccccc".repeat(4);
//! let packed = encode(Compression::Rle, &data);
//! let back = decode(Compression::Rle, &packed, 1 << 20).unwrap();
//! assert_eq!(back, data);
//! ```

pub mod lz;
pub mod rle;

use crate::errors::Result;
use crate::types::Compression;

/// Compresses `data` with the given method. This never fails.
pub fn encode(method: Compression, data: &[u8]) -> Vec<u8> {
    match method {
        Compression::Store => data.to_vec(),
        Compression::Rle => rle::encode(data),
        Compression::Lz => lz::encode(data),
    }
}

/// Decompresses `data` with the given method, producing at most `max_output`
/// bytes (a decompression-bomb guard). Returns an error for malformed input or
/// oversized output; never panics.
pub fn decode(method: Compression, data: &[u8], max_output: usize) -> Result<Vec<u8>> {
    match method {
        Compression::Store => {
            if data.len() > max_output {
                return Err(crate::errors::CodecError::OutputTooLarge { cap: max_output }.into());
            }
            Ok(data.to_vec())
        }
        Compression::Rle => rle::decode(data, max_output),
        Compression::Lz => lz::decode(data, max_output),
    }
}

/// Picks the method that yields the smallest encoding for `data`, returning the
/// chosen method and the encoded bytes. `Store` wins ties so that
/// already-incompressible data is not needlessly transformed.
pub fn best(data: &[u8]) -> (Compression, Vec<u8>) {
    let store = data.to_vec();
    let mut best_method = Compression::Store;
    let mut best_bytes = store;

    for method in [Compression::Rle, Compression::Lz] {
        let encoded = encode(method, data);
        if encoded.len() < best_bytes.len() {
            best_method = method;
            best_bytes = encoded;
        }
    }
    (best_method, best_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_method_roundtrips() {
        let data = b"hello hello hello world world world".repeat(10);
        for method in [Compression::Store, Compression::Rle, Compression::Lz] {
            let encoded = encode(method, &data);
            let decoded = decode(method, &encoded, 1 << 20).unwrap();
            assert_eq!(decoded, data, "method {:?}", method);
        }
    }

    #[test]
    fn store_respects_cap() {
        assert!(decode(Compression::Store, &[0u8; 100], 10).is_err());
    }

    #[test]
    fn best_never_expands() {
        let data = vec![42u8; 1000];
        let (method, bytes) = best(&data);
        assert!(bytes.len() <= data.len());
        assert_eq!(decode(method, &bytes, 1 << 20).unwrap(), data);
    }

    #[test]
    fn best_chooses_store_for_random() {
        // Incompressible-ish data: best should not expand it.
        let data: Vec<u8> = (0..256u32).map(|i| (i * 37 % 251) as u8).collect();
        let (_, bytes) = best(&data);
        assert!(bytes.len() <= data.len());
    }
}
