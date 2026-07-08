use std::collections::HashMap;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use super::common::{
    FLAG_COMPRESSED, PERRO_ASSETS_COMPRESSED_MAGIC, PERRO_ASSETS_MAGIC, PerroAssetsEntryMeta,
    read_header, read_index_entry,
};
use super::compression::decompress_zlib_limited;

pub type PerroAssetsEntry = PerroAssetsEntryMeta;

const MAX_DECOMPRESSED_ARCHIVE_BYTES: usize = 1024 * 1024 * 1024;

pub struct PerroAssetsArchive {
    data: Arc<[u8]>,
    index: HashMap<String, PerroAssetsEntry>,
}

impl PerroAssetsArchive {
    /// Open a .perro archive from embedded bytes (include_bytes!)
    pub fn open_from_bytes(data: &'static [u8]) -> io::Result<Self> {
        let arc: Arc<[u8]> = Arc::from(data);
        Self::open_from_arc(arc)
    }

    /// Open a .perro archive from owned bytes.
    pub fn open_from_owned_bytes(data: Vec<u8>) -> io::Result<Self> {
        let data = decode_archive_container(data)?;
        let arc: Arc<[u8]> = Arc::from(data.into_boxed_slice());
        Self::open_from_arc(arc)
    }

    /// Open a .perro archive from file path.
    pub fn open_from_file(path: &Path) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::open_from_owned_bytes(bytes)
    }

    fn open_from_arc(arc: Arc<[u8]>) -> io::Result<Self> {
        let mut cursor = Cursor::new(&*arc);
        let index = Self::parse_index(&mut cursor)?;
        Ok(Self { data: arc, index })
    }

    fn parse_index(cursor: &mut Cursor<&[u8]>) -> io::Result<HashMap<String, PerroAssetsEntry>> {
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

        let range = checked_entry_range(self.data.len(), entry)?;
        let mut data_buf = self.data[range].to_vec();

        // Decompress if needed
        if entry.flags & FLAG_COMPRESSED != 0 {
            let expected_size = checked_decompressed_size(entry.original_size)?;
            let decompressed = decompress_zlib_limited(&data_buf, expected_size)?;

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
            return Err(io::Error::other(
                "Cannot get slice of compressed file (use read_file)",
            ));
        }

        let range = checked_entry_range(self.data.len(), entry)?;
        Ok(&self.data[range])
    }

    /// Stream a file (only works for uncompressed files)
    pub fn stream_file(&self, path: &str) -> io::Result<PerroAssetsFile> {
        let entry = self
            .index
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        if entry.flags & FLAG_COMPRESSED != 0 {
            return Err(io::Error::other(
                "Streaming compressed files not supported (use read_file)",
            ));
        }

        checked_entry_range(self.data.len(), entry)?;

        Ok(PerroAssetsFile {
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

fn decode_archive_container(data: Vec<u8>) -> io::Result<Vec<u8>> {
    if data.len() < 4 || data[..4] == PERRO_ASSETS_MAGIC {
        return Ok(data);
    }

    if data[..4] != PERRO_ASSETS_COMPRESSED_MAGIC {
        return Ok(data);
    }

    if data.len() < 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Compressed PerroAssets header too short",
        ));
    }

    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    if version != perro_asset_formats::archive::VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported compressed PerroAssets version {version}"),
        ));
    }

    let original_size =
        checked_decompressed_size(u64::from_le_bytes(data[8..16].try_into().unwrap()))?;
    let mut out = decompress_zlib_limited(&data[16..], original_size)?;
    if out.len() != original_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Compressed PerroAssets size mismatch: expected {}, got {}",
                original_size,
                out.len()
            ),
        ));
    }
    Ok(std::mem::take(&mut out))
}

fn checked_decompressed_size(size: u64) -> io::Result<usize> {
    let size = usize::try_from(size)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "archive entry too large"))?;
    if size > MAX_DECOMPRESSED_ARCHIVE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive entry exceeds decompression limit",
        ));
    }
    Ok(size)
}

fn checked_entry_range(data_len: usize, entry: &PerroAssetsEntry) -> io::Result<Range<usize>> {
    let start = usize::try_from(entry.offset)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "archive offset too large"))?;
    let size = usize::try_from(entry.size)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "archive entry too large"))?;
    let end = start
        .checked_add(size)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "archive range overflow"))?;
    if end > data_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive entry outside data bounds",
        ));
    }
    Ok(start..end)
}

/// Streaming file handle (for uncompressed files only)
pub struct PerroAssetsFile {
    data: Arc<[u8]>,
    entry: PerroAssetsEntry,
    pos: u64,
}

impl Read for PerroAssetsFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let range = checked_entry_range(self.data.len(), &self.entry)?;
        let data = &self.data[range];
        let pos = usize::try_from(self.pos)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "read offset too large"))?;
        let remaining = data.get(pos..).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "read offset out of bounds")
        })?;
        let amt = remaining.len().min(buf.len());
        buf[..amt].copy_from_slice(&remaining[..amt]);
        self.pos += amt as u64;
        Ok(amt)
    }
}

impl Seek for PerroAssetsFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::End(n) => checked_seek(self.entry.size, n)?,
            SeekFrom::Current(n) => checked_seek(self.pos, n)?,
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

fn checked_seek(base: u64, offset: i64) -> io::Result<u64> {
    if offset >= 0 {
        base.checked_add(offset as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek offset overflow"))
    } else {
        base.checked_sub(offset.unsigned_abs())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before start"))
    }
}

#[cfg(test)]
mod tests {
    use super::checked_seek;

    #[test]
    fn checked_seek_rejects_wraparound() {
        assert_eq!(checked_seek(10, 5).unwrap(), 15);
        assert_eq!(checked_seek(10, -5).unwrap(), 5);
        assert!(checked_seek(10, -11).is_err());
        assert!(checked_seek(u64::MAX, 1).is_err());
    }
}
