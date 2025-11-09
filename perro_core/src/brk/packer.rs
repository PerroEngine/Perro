//! 🐾 BaRK Format (Binary Resource pacK)

use std::fs::{self, File};
use std::io::{self, Write, Seek, SeekFrom};
use std::path::Path;
use walkdir::WalkDir;

use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use rand::RngCore;
use serde_json::Value;

use zstd::stream::{encode_all}; // Added for Zstandard compression

/// File types to skip entirely from BRK packaging (e.g., source code, scripts that will be compiled)
const SKIP_EXTENSIONS: &[&str] = &["pup", "rs", "cs", "ts", "go"];
/// File types to encrypt (e.g., sensitive game data, scenes)
const ENCRYPT_EXTENSIONS: &[&str] = &["scn", "fur", "toml"];
/// File types that are already efficiently compressed (e.g., JPG, PNG, OGG)
/// These might not benefit much from Zstd and could even get larger or take longer.
const ALREADY_COMPRESSED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "ogg", "mp3", "webp"];

// Flags for BrkEntry
const FLAG_COMPRESSED: u32 = 1; // Bit 0: Data is ZSTD compressed
const FLAG_ENCRYPTED: u32 = 2;  // Bit 1: Data is AES-GCM encrypted

/// Archive header
pub struct BrkHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub file_count: u32,
    pub index_offset: u64,
}

/// Entry metadata written into the index
#[derive(Debug, Clone)] // Derive Debug and Clone for convenience
pub struct BrkEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,          // This will be the *actual* size in the archive (compressed if FLAG_COMPRESSED)
    pub original_size: u64, // The original size before any compression
    pub flags: u32,
    pub nonce: [u8; 12],
    pub tag: [u8; 16],
}

/// Write header manually (little-endian)
fn write_header(file: &mut File, header: &BrkHeader) -> io::Result<()> {
    file.write_all(&header.magic)?;
    file.write_all(&header.version.to_le_bytes())?;
    file.write_all(&header.file_count.to_le_bytes())?;
    file.write_all(&header.index_offset.to_le_bytes())?;
    Ok(())
}

/// Minify JSON (.scn) file in memory
fn minify_json(path: &Path) -> io::Result<Vec<u8>> {
    let data = fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let minified = serde_json::to_vec(&json)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(minified)
}

/// Minify FUR file in memory
fn minify_fur(path: &Path) -> io::Result<Vec<u8>> {
    let data = fs::read_to_string(path)?;
    let mut output = String::with_capacity(data.len());

    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        output.push_str(line);
        output.push(' '); // single space to separate tokens
    }
    Ok(output.trim().as_bytes().to_vec())
}

/// Build a `.brk` archive
pub fn build_brk(output: &Path, res_dir: &Path, project_root: &Path, key: &[u8; 32]) -> io::Result<()> {
    let mut file = File::create(output)?;

    // Write placeholder header
    let header = BrkHeader {
        magic: *b"BRK1",
        version: 1, // Archive version. Increment if metadata format changes!
        file_count: 0,
        index_offset: 0,
    };
    write_header(&mut file, &header)?;

    let mut entries = Vec::new();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    // Define a ZSTD compression level (1-22, higher is more compression, slower)
    // 0 is default, negative numbers are fast. Higher numbers improve ratio.
    const COMPRESSION_LEVEL: i32 = 10; 

    // Helper closure to process data (compress, then encrypt if needed)
    let mut process_data = |mut data: Vec<u8>, should_encrypt: bool, should_compress: bool| -> io::Result<(Vec<u8>, u32, [u8; 12], [u8; 16], u64)> {
        let original_data_len = data.len() as u64;
        let mut flags = 0;
        let mut nonce = [0u8; 12];
        let mut tag = [0u8; 16];

        // --- COMPRESSION STEP ---
        if should_compress && original_data_len > 0 { // Only compress if flagged and not empty
            let compressed_data = encode_all(&*data, COMPRESSION_LEVEL)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("ZSTD compression failed: {}", e)))?;
            
            // Only use compressed data if it's actually smaller.
            // Zstd has a header, so very tiny files might get slightly larger.
            if compressed_data.len() < data.len() {
                data = compressed_data;
                flags |= FLAG_COMPRESSED;
            }
        }

        // --- ENCRYPTION STEP ---
        if should_encrypt {
            rand::thread_rng().fill_bytes(&mut nonce);
            let nonce_obj = Nonce::from_slice(&nonce);
            let encrypted = cipher.encrypt(nonce_obj, &*data)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Encryption failed"))?;
            let (ciphertext, gcm_tag) = encrypted.split_at(encrypted.len() - 16);
            data = ciphertext.to_vec();
            tag.copy_from_slice(gcm_tag);
            flags |= FLAG_ENCRYPTED;
        }

        Ok((data, flags, nonce, tag, original_data_len))
    };

    // 1. Explicitly add project.toml (always encrypted, and compressed)
    let project_toml = project_root.join("project.toml");
    if project_toml.exists() {
        let raw_data = fs::read(&project_toml)?;
        let (processed_data, flags, nonce, tag, original_size) = process_data(raw_data, true, true)?; // Encrypt and Compress project.toml

        let offset = file.seek(SeekFrom::Current(0))?;
        file.write_all(&processed_data)?;
        let size = processed_data.len() as u64;

        entries.push(BrkEntry {
            path: "project.toml".to_string(),
            offset,
            size,
            original_size,
            flags,
            nonce,
            tag,
        });
    }

    // 2. Walk res/ folder
    for entry in WalkDir::new(res_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path();

            // Skip unwanted extensions (scripts, source files)
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SKIP_EXTENSIONS.contains(&ext) {
                    continue;
                }
            }

            let rel = path.strip_prefix(res_dir).unwrap().to_string_lossy().to_string();

            // Minify .scn and .fur in memory
            let mut data = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                match ext {
                    "scn" => minify_json(path)?,
                    "fur" => minify_fur(path)?,
                    _ => fs::read(path)?,
                }
            } else {
                fs::read(path)?
            };

            // Determine if this file should be encrypted
            let should_encrypt = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                ENCRYPT_EXTENSIONS.contains(&ext)
            } else {
                false
            };
            
            // Determine if this file should be compressed
            let should_compress = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                !ALREADY_COMPRESSED_EXTENSIONS.contains(&ext) // Don't re-compress if format is already compressed
            } else {
                true // Default to compressing unknown extensions
            };


            let (processed_data, flags, nonce, tag, original_size) = process_data(data, should_encrypt, should_compress)?;

            let offset = file.seek(SeekFrom::Current(0))?;
            file.write_all(&processed_data)?;
            let size = processed_data.len() as u64;

            entries.push(BrkEntry {
                path: format!("res/{}", rel.replace("\\", "/")),
                offset,
                size,
                original_size,
                flags,
                nonce,
                tag,
            });
        }
    }

    // 3. Write index
    let index_offset = file.seek(SeekFrom::Current(0))?;
    for e in &entries {
        let path_bytes = e.path.as_bytes();
        let path_len = path_bytes.len() as u16;
        file.write_all(&path_len.to_le_bytes())?;
        file.write_all(path_bytes)?;
        file.write_all(&e.offset.to_le_bytes())?;
        file.write_all(&e.size.to_le_bytes())?;
        file.write_all(&e.original_size.to_le_bytes())?; // NEW: Write original_size
        file.write_all(&e.flags.to_le_bytes())?;
        file.write_all(&e.nonce)?;
        file.write_all(&e.tag)?;
    }

    // 4. Rewrite header with correct counts
    file.seek(SeekFrom::Start(0))?;
    let header = BrkHeader {
        magic: *b"BRK1",
        version: 1,
        file_count: entries.len() as u32,
        index_offset,
    };
    write_header(&mut file, &header)?;

    Ok(())
}