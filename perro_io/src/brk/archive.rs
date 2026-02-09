use flate2::read::DeflateDecoder;
use std::collections::HashMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::sync::Arc;

use super::common::{BrkEntryMeta, FLAG_COMPRESSED, read_header, read_index_entry};

pub type BrkEntry = BrkEntryMeta;

pub struct BrkArchive {
    data: Arc<[u8]>,
    index: HashMap<String, BrkEntry>,
}

impl BrkArchive {
    /// Open a .brk archive from embedded bytes (include_bytes!)
    pub fn open_from_bytes(data: &'static [u8]) -> io::Result<Self> {
        let arc: Arc<[u8]> = Arc::from(data);
        let mut cursor = Cursor::new(&*arc);
        let index = Self::parse_index(&mut cursor)?;
        Ok(Self { data: arc, index })
    }

    fn parse_index(cursor: &mut Cursor<&[u8]>) -> io::Result<HashMap<String, BrkEntry>> {
        let header = read_header(cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;
        let mut index = HashMap::new();

        for _ in 0..header.file_count {
            let (path, meta) = read_index_entry(cursor)?;
            index.insert(path, meta);
        }

        Ok(index)
    }

    /// Read a file fully into memory
    pub fn read_file(&self, path: &str) -> io::Result<Vec<u8>> {
        let entry = self
            .index
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        let mut data_buf = self.data[start..end].to_vec();

        // Decompress if needed
        if entry.flags & FLAG_COMPRESSED != 0 {
            let mut decoder = DeflateDecoder::new(&data_buf[..]);
            let mut decompressed = Vec::with_capacity(entry.original_size as usize);
            decoder.read_to_end(&mut decompressed)?;

            if decompressed.len() as u64 != entry.original_size {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Size mismatch: expected {}, got {}",
                        entry.original_size,
                        decompressed.len()
                    ),
                ));
            }
            data_buf = decompressed;
        }

        Ok(data_buf)
    }

    /// Get a direct slice (only works for uncompressed files)
    pub fn get_file_slice(&self, path: &str) -> io::Result<&[u8]> {
        let entry = self
            .index
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        if entry.flags & FLAG_COMPRESSED != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot get slice of compressed file (use read_file)",
            ));
        }

        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        Ok(&self.data[start..end])
    }

    /// Stream a file (only works for uncompressed files)
    pub fn stream_file(&self, path: &str) -> io::Result<BrkFile> {
        let entry = self
            .index
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        if entry.flags & FLAG_COMPRESSED != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Streaming compressed files not supported (use read_file)",
            ));
        }

        Ok(BrkFile {
            data: self.data.clone(),
            entry: entry.clone(),
            pos: 0,
        })
    }

    /// List all files in the archive
    pub fn list_files(&self) -> Vec<String> {
        self.index.keys().cloned().collect()
    }
}

/// Streaming file handle (for uncompressed files only)
pub struct BrkFile {
    data: Arc<[u8]>,
    entry: BrkEntry,
    pos: u64,
}

impl Read for BrkFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let start = self.entry.offset as usize;
        let end = (self.entry.offset + self.entry.size) as usize;
        let data = &self.data[start..end];

        let remaining = &data[self.pos as usize..];
        let amt = remaining.len().min(buf.len());
        buf[..amt].copy_from_slice(&remaining[..amt]);
        self.pos += amt as u64;
        Ok(amt)
    }
}

impl Seek for BrkFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => (self.entry.size as i64 + n) as u64,
            SeekFrom::Current(n) => (self.pos as i64 + n) as u64,
        };

        if new_pos > self.entry.size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek out of bounds",
            ));
        }

        self.pos = new_pos;
        Ok(self.pos)
    }
}
