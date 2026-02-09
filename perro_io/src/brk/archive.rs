use std::collections::HashMap;
use std::fs;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use memmap2::Mmap;

use zstd::stream::decode_all;

use super::common::{BrkEntryMeta, FLAG_COMPRESSED, FLAG_ENCRYPTED, read_header, read_index_entry};

/// Entry metadata
pub type BrkEntry = BrkEntryMeta;

/// Archive data source
#[derive(Clone)]
pub enum BrkData { 
    Bytes(Arc<[u8]>), // for include_bytes!
    Mmap(Arc<Mmap>),  // for disk files
}

/// Archive handle
pub struct BrkArchive {
    data: BrkData,
    index: HashMap<String, BrkEntry>,
}

impl BrkArchive {
    /// Open a .brk archive from embedded bytes (include_bytes!)
    pub fn open_from_bytes(data: &'static [u8]) -> io::Result<Self> {
        let arc: Arc<[u8]> = Arc::from(data);
        let mut cursor = Cursor::new(&*arc);
        let index = Self::parse_index(&mut cursor)?;
        Ok(Self {
            data: BrkData::Bytes(arc),
            index,
        })
    }

    /// Open a .brk archive from disk (mmap for streaming, e.g. DLCs)
    pub fn open_from_file(path: &Path) -> io::Result<Self> {
        let file = fs::File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let arc = Arc::new(mmap);
        let mut cursor = Cursor::new(&*arc);
        let index = Self::parse_index(&mut cursor)?;
        Ok(Self {
            data: BrkData::Mmap(arc),
            index,
        })
    }

    /// Parse header + index (generic over &[u8] or Mmap)
    fn parse_index<T: AsRef<[u8]>>(
        cursor: &mut Cursor<T>,
    ) -> io::Result<HashMap<String, BrkEntry>> {
        let header = read_header(cursor)?;
        cursor.seek(SeekFrom::Start(header.index_offset))?;
        let mut index = HashMap::new();

        for _ in 0..header.file_count {
            let (path, meta) = read_index_entry(cursor)?;
            index.insert(path, meta);
        }

        Ok(index)
    }

    /// Read a file fully into memory (for small/encrypted/compressed files)
    pub fn read_file(&self, path: &str, key: Option<&[u8; 32]>) -> io::Result<Vec<u8>> {
        let entry = self
            .index
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        let mut data_buf = match &self.data {
            // Make buf mutable, rename to data_buf for clarity
            BrkData::Bytes(bytes) => {
                let start = entry.offset as usize;
                let end = start + entry.size as usize;
                bytes[start..end].to_vec()
            }
            BrkData::Mmap(mmap) => {
                let start = entry.offset as usize;
                let end = start + entry.size as usize;
                mmap[start..end].to_vec()
            }
        };

        // --- DECRYPTION STEP --- (Decrypt first if encrypted)
        if entry.flags & FLAG_ENCRYPTED != 0 {
            let key =
                key.ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Missing decryption key"))?;
            let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
            let nonce = Nonce::from_slice(&entry.nonce);

            let mut combined_encrypted_data_with_tag = data_buf.clone();
            combined_encrypted_data_with_tag.extend_from_slice(&entry.tag);

            let decrypted = cipher
                .decrypt(nonce, combined_encrypted_data_with_tag.as_ref())
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Decryption failed"))?;
            data_buf = decrypted; // Update data_buf with decrypted data
        }

        // --- DECOMPRESSION STEP --- (Decompress second if compressed)
        if entry.flags & FLAG_COMPRESSED != 0 {
            let decompressed = decode_all(&*data_buf).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("ZSTD decompression failed: {}", e),
                )
            })?;

            // Verify decompressed size matches original_size for integrity
            if decompressed.len() as u64 != entry.original_size {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Decompressed size mismatch for '{}'. Expected {} bytes, got {} bytes.",
                        path,
                        entry.original_size,
                        decompressed.len()
                    ),
                ));
            }
            data_buf = decompressed; // Update data_buf with decompressed data
        }

        Ok(data_buf)
    }

    /// Open a file for streaming. Only works for unencrypted AND uncompressed files.
    pub fn stream_file(&self, path: &str) -> io::Result<BrkFile> {
        let entry = self
            .index
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File not found"))?;

        // Streaming is not supported for encrypted or compressed files as BrkFile
        // currently reads raw chunks and does not handle on-the-fly decryption/decompression.
        if entry.flags & FLAG_ENCRYPTED != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Streaming encrypted files not supported (use read_file)",
            ));
        }
        if entry.flags & FLAG_COMPRESSED != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Streaming compressed files not supported (use read_file)",
            ));
        }

        Ok(BrkFile {
            data: match &self.data {
                BrkData::Bytes(b) => BrkData::Bytes(b.clone()),
                BrkData::Mmap(m) => BrkData::Mmap(m.clone()),
            },
            entry: entry.clone(),
            pos: 0,
        })
    }

    /// List all files in the archive
    pub fn list_files(&self) -> Vec<String> {
        self.index.keys().cloned().collect()
    }
}

/// Streaming file handle
pub struct BrkFile {
    data: BrkData,
    entry: BrkEntry,
    pos: u64,
}

impl Read for BrkFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let data: &[u8] = match &self.data {
            BrkData::Bytes(b) => {
                &b[self.entry.offset as usize..(self.entry.offset + self.entry.size) as usize]
            }
            BrkData::Mmap(m) => {
                &m[self.entry.offset as usize..(self.entry.offset + self.entry.size) as usize]
            }
        };

        let mut remaining = &data[self.pos as usize..];
        let amt = remaining.read(buf)?;
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

        // Seeking beyond the actual content length is an error
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
