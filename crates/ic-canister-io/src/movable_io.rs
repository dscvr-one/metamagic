//! Common IO logic

use std::io::{Read, Seek, Write};

/// Helper reader that wraps around an existing reader that can be moved.
pub struct MovableReader<'a, R: Read + Seek> {
    /// The underlying reader
    reader: &'a mut R,
}

impl<'a, R: Read + Seek> MovableReader<'a, R> {
    /// Create a new consumable reader from a reader reference
    #[inline]
    pub fn new(reader: &'a mut R) -> Self {
        Self { reader }
    }
}

impl<'a, R: Read + Seek> Read for MovableReader<'a, R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Read::read(&mut self.reader, buf)
    }
}

impl<'a, R: Read + Seek> Seek for MovableReader<'a, R> {
    #[inline]
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Seek::seek(&mut self.reader, pos)
    }
}

/// Helper writer that saves the offset when dropped. This is useful to track the number
/// of bytes read for crates such as bincode which consume the reader during serialization.
pub struct MovableWriter<'a, W: Write + Seek> {
    /// The underlying writer
    writer: &'a mut W,
}

impl<'a, W: Write + Seek> MovableWriter<'a, W> {
    /// Create a new consumable writer from a writer reference
    pub fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }
}

impl<'a, W: Write + Seek> Write for MovableWriter<'a, W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Write::write(&mut self.writer, buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<'a, W: Write + Seek> Seek for MovableWriter<'a, W> {
    #[inline]
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Seek::seek(&mut self.writer, pos)
    }
}
