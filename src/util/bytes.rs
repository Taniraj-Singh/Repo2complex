//! Bounds-checked big-endian byte reading and writing.
//!
//! The whole crate uses fixed-width big-endian integers on disk (unlike a
//! varint format), so these helpers are the single place that touches raw
//! bytes. [`ByteReader`] never indexes without a length check, which is the
//! property the archive-parsing fuzz targets rely on: a truncated or hostile
//! buffer yields an [`Error::Truncated`], never a panic.

use crate::errors::{Error, Result};

/// A forward, bounds-checked cursor over a byte slice.
pub struct ByteReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    /// Wraps a byte slice.
    pub fn new(buf: &'a [u8]) -> Self {
        ByteReader { buf, pos: 0 }
    }

    /// Creates a reader positioned at `offset`.
    pub fn at(buf: &'a [u8], offset: usize) -> Result<Self> {
        if offset > buf.len() {
            return Err(Error::truncated(offset as u64, 0, buf.len()));
        }
        Ok(ByteReader { buf, pos: offset })
    }

    /// The current read position.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// The number of unread bytes.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Whether all bytes have been consumed.
    pub fn is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    /// Moves the cursor to an absolute offset.
    pub fn seek(&mut self, offset: usize) -> Result<()> {
        if offset > self.buf.len() {
            return Err(Error::truncated(offset as u64, 0, self.buf.len()));
        }
        self.pos = offset;
        Ok(())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.buf.len())
            .ok_or_else(|| Error::truncated(self.pos as u64, n, self.remaining()))?;
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Reads `n` raw bytes.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.take(n)
    }

    /// Reads a single byte.
    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// Reads a big-endian `u16`.
    pub fn read_u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// Reads a big-endian `u32`.
    pub fn read_u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a big-endian `u64`.
    pub fn read_u64(&mut self) -> Result<u64> {
        let b = self.take(8)?;
        Ok(u64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a big-endian `i64`.
    pub fn read_i64(&mut self) -> Result<i64> {
        Ok(self.read_u64()? as i64)
    }

    /// Reads a fixed-size array.
    pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }
}

/// An append-only big-endian byte writer over a `Vec<u8>`.
#[derive(Debug, Default)]
pub struct ByteWriter {
    buf: Vec<u8>,
}

impl ByteWriter {
    /// A new empty writer.
    pub fn new() -> Self {
        ByteWriter { buf: Vec::new() }
    }

    /// A new writer with reserved capacity.
    pub fn with_capacity(cap: usize) -> Self {
        ByteWriter {
            buf: Vec::with_capacity(cap),
        }
    }

    /// The number of bytes written so far, i.e. the current offset.
    pub fn offset(&self) -> u64 {
        self.buf.len() as u64
    }

    /// The number of bytes written.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether nothing has been written.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Appends raw bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Appends a single byte.
    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Appends a big-endian `u16`.
    pub fn write_u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Appends a big-endian `u32`.
    pub fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Appends a big-endian `u64`.
    pub fn write_u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Appends a big-endian `i64`.
    pub fn write_i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Overwrites 8 bytes at `offset` with a big-endian `u64`. Used to back-patch
    /// the index offset into the header once it is known.
    ///
    /// # Panics
    /// Panics if `offset + 8` exceeds the buffer; callers only patch positions
    /// they previously reserved.
    pub fn patch_u64(&mut self, offset: usize, v: u64) {
        self.buf[offset..offset + 8].copy_from_slice(&v.to_be_bytes());
    }

    /// Consumes the writer, returning the buffer.
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// Borrows the written bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_integers() {
        let mut w = ByteWriter::new();
        w.write_u8(0x12);
        w.write_u16(0x3456);
        w.write_u32(0x789a_bcde);
        w.write_u64(0x0102_0304_0506_0708);
        w.write_i64(-2);
        let bytes = w.into_vec();

        let mut r = ByteReader::new(&bytes);
        assert_eq!(r.read_u8().unwrap(), 0x12);
        assert_eq!(r.read_u16().unwrap(), 0x3456);
        assert_eq!(r.read_u32().unwrap(), 0x789a_bcde);
        assert_eq!(r.read_u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(r.read_i64().unwrap(), -2);
        assert!(r.is_empty());
    }

    #[test]
    fn reads_are_bounds_checked() {
        let mut r = ByteReader::new(&[0x00, 0x01]);
        assert!(r.read_u32().is_err());
        let mut r2 = ByteReader::new(&[]);
        assert!(r2.read_u8().is_err());
    }

    #[test]
    fn seek_and_at_validate() {
        let buf = [0u8; 4];
        assert!(ByteReader::at(&buf, 5).is_err());
        let mut r = ByteReader::at(&buf, 2).unwrap();
        assert_eq!(r.remaining(), 2);
        assert!(r.seek(10).is_err());
    }

    #[test]
    fn patch_backfills() {
        let mut w = ByteWriter::new();
        let slot = w.len();
        w.write_u64(0); // placeholder
        w.write_u8(0xff);
        w.patch_u64(slot, 0xAABB);
        let bytes = w.into_vec();
        let mut r = ByteReader::new(&bytes);
        assert_eq!(r.read_u64().unwrap(), 0xAABB);
    }
}
