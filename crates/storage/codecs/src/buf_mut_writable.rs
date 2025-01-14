use bytes::{buf::UninitSlice, BufMut, Bytes, BytesMut};
use std::ops::{Deref, DerefMut};

/// A trait that provides safe access to written portions of a buffer.
/// This trait is designed to be used in conjunction with `BufMut` to avoid
/// the footgun of mixing `BufMut` with `AsMut<[u8]>`.
pub trait BufMutWritable: BufMut {
    /// Returns a slice containing only the initialized/written portion of the buffer
    fn written_slice(&self) -> &[u8];

    /// Returns a mutable slice containing only the initialized/written portion of the buffer
    fn written_slice_mut(&mut self) -> &mut [u8];

    /// Returns the current length of written data
    fn written_len(&self) -> usize;

    /// Safely patch previously written bytes at a given offset
    ///
    /// # Panics
    ///
    /// Panics if offset + data.len() exceeds the written length
    fn patch_at(&mut self, offset: usize, data: &[u8]) {
        assert!(offset + data.len() <= self.written_len(), "attempt to patch unwritten bytes");
        self.written_slice_mut()[offset..offset + data.len()].copy_from_slice(data);
    }
}

#[derive(Debug)]
/// A wrapper type that implements `BufMutWritable` for `Vec<u8>`
pub struct WritableBuffer {
    inner: Vec<u8>,
    written: usize,
}

impl WritableBuffer {
    /// Create a new empty buffer
    pub fn new() -> Self {
        Self { inner: Vec::new(), written: 0 }
    }

    /// Create a new buffer with the given capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self { inner: Vec::with_capacity(capacity), written: 0 }
    }

    /// Convert into the underlying Vec<u8>
    pub fn into_inner(self) -> Vec<u8> {
        self.inner
    }

    /// Convert into Bytes
    pub fn into_bytes(self) -> Bytes {
        self.inner.into()
    }
}

impl Default for WritableBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for WritableBuffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for WritableBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// # Safety
///
/// This implementation upholds the following invariants required by `BufMut`:
/// - The buffer's capacity is always sufficient for the requested write operations
/// - The written length is always less than or equal to the buffer's length
/// - All bytes up to `written` are properly initialized
/// - The buffer is not shared or aliased while being written to
unsafe impl BufMut for WritableBuffer {
    fn remaining_mut(&self) -> usize {
        usize::MAX - self.inner.len()
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        // SAFETY: We're getting the spare capacity which is guaranteed to be valid for writing
        unsafe {
            UninitSlice::from_raw_parts_mut(
                self.inner.as_mut_ptr().add(self.inner.len()),
                self.inner.capacity() - self.inner.len(),
            )
        }
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        let new_len = self.inner.len() + cnt;
        // SAFETY: The caller must guarantee that the advanced bytes have been initialized
        self.inner.set_len(new_len);
        self.written += cnt;
    }

    fn put<T: bytes::Buf>(&mut self, mut src: T) {
        while src.has_remaining() {
            let chunk = src.chunk();
            self.put_slice(chunk);
            src.advance(chunk.len());
        }
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.inner.extend_from_slice(src);
        self.written += src.len();
    }
}

impl BufMutWritable for WritableBuffer {
    fn written_slice(&self) -> &[u8] {
        &self.inner[..self.written]
    }

    fn written_slice_mut(&mut self) -> &mut [u8] {
        &mut self.inner[..self.written]
    }

    fn written_len(&self) -> usize {
        self.written
    }
}

/// A wrapper type that implements `BufMutWritable` for `&mut [u8]`
pub struct WritableSlice<'a> {
    inner: &'a mut [u8],
    written: usize,
}

impl<'a> WritableSlice<'a> {
    /// Create a new writable slice
    pub fn new(slice: &'a mut [u8]) -> Self {
        Self { inner: slice, written: 0 }
    }

    /// Get the underlying slice
    pub fn into_inner(self) -> &'a mut [u8] {
        self.inner
    }
}

/// # Safety
///
/// This implementation upholds the following invariants required by `BufMut`:
/// - The buffer's capacity is fixed and checked on every write operation
/// - The written length is always less than or equal to the buffer's length
/// - All bytes up to `written` are properly initialized
/// - The buffer is not shared or aliased while being written to
unsafe impl<'a> BufMut for WritableSlice<'a> {
    fn remaining_mut(&self) -> usize {
        self.inner.len() - self.written
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        // SAFETY: We're getting the unwritten portion which is valid for writing
        unsafe {
            UninitSlice::from_raw_parts_mut(
                self.inner.as_mut_ptr().add(self.written),
                self.inner.len() - self.written,
            )
        }
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        let new_written = self.written + cnt;
        assert!(new_written <= self.inner.len(), "buffer overflow");
        self.written = new_written;
    }

    fn put<T: bytes::Buf>(&mut self, mut src: T) {
        while src.has_remaining() {
            let chunk = src.chunk();
            self.put_slice(chunk);
            src.advance(chunk.len());
        }
    }

    fn put_slice(&mut self, src: &[u8]) {
        let new_written = self.written + src.len();
        assert!(new_written <= self.inner.len(), "buffer overflow");
        self.inner[self.written..new_written].copy_from_slice(src);
        self.written = new_written;
    }
}

impl<'a> BufMutWritable for WritableSlice<'a> {
    fn written_slice(&self) -> &[u8] {
        &self.inner[..self.written]
    }

    fn written_slice_mut(&mut self) -> &mut [u8] {
        &mut self.inner[..self.written]
    }

    fn written_len(&self) -> usize {
        self.written
    }
}

impl BufMutWritable for Vec<u8> {
    fn written_slice(&self) -> &[u8] {
        self.as_slice()
    }

    fn written_slice_mut(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }

    fn written_len(&self) -> usize {
        self.len()
    }
}

impl BufMutWritable for BytesMut {
    fn written_slice(&self) -> &[u8] {
        self.as_ref()
    }

    fn written_slice_mut(&mut self) -> &mut [u8] {
        self.as_mut()
    }

    fn written_len(&self) -> usize {
        self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writable_buffer() {
        let mut buf = WritableBuffer::new();

        // Write some data
        buf.put_slice(b"hello");
        assert_eq!(buf.written_len(), 5);
        assert_eq!(buf.written_slice(), b"hello");

        // Patch some data
        buf.patch_at(0, b"H");
        assert_eq!(buf.written_slice(), b"Hello");

        // Write more data
        buf.put_slice(b" world");
        assert_eq!(buf.written_len(), 11);
        assert_eq!(buf.written_slice(), b"Hello world");
    }

    #[test]
    fn test_writable_slice() {
        let mut data = [0u8; 16];
        let mut buf = WritableSlice::new(&mut data);

        // Write some data
        buf.put_slice(b"hello");
        assert_eq!(buf.written_len(), 5);
        assert_eq!(buf.written_slice(), b"hello");

        // Patch some data
        buf.patch_at(0, b"H");
        assert_eq!(buf.written_slice(), b"Hello");

        // Write more data
        buf.put_slice(b" world");
        assert_eq!(buf.written_len(), 11);
        assert_eq!(buf.written_slice(), b"Hello world");
    }

    #[test]
    #[should_panic(expected = "buffer overflow")]
    fn test_slice_overflow() {
        let mut data = [0u8; 4];
        let mut buf = WritableSlice::new(&mut data);
        buf.put_slice(b"hello"); // Should panic - buffer too small
    }

    #[test]
    #[should_panic(expected = "attempt to patch unwritten bytes")]
    fn test_slice_patch_unwritten() {
        let mut data = [0u8; 16];
        let mut buf = WritableSlice::new(&mut data);
        buf.put_slice(b"hello");
        buf.patch_at(5, b"!"); // Should panic - trying to write beyond written length
    }
}
