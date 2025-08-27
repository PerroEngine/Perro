//! 🐾 BaRK Format (Binary Resource pacK)
//!
//! This module builds `.brk` archives for Perro.
//!
//! A `.brk` file is a **Binary Resource pacK** — a simple, efficient, and secure
//! container format for bundling game assets.
//!
//! ## Layout
//!
//! ```text
//! +-------------------+
//! | Header            |  (magic, version, file count, index offset)
//! +-------------------+
//! | File Data Blobs   |  (raw file contents | project.toml, scenes, and fur files are encrypted)
//! +-------------------+
//! | File Index        |  (path, offset, size, flags, nonce, tag)
//! +-------------------+
//! ```
//!
//! ## Header
//! - Magic: `BRK1`
//! - Version: `u32` (currently 1)
//! - File Count: `u32`
//! - Index Offset: `u64`
//!
//! ## Index Entries
//! - Path length (`u16`) + UTF‑8 path string
//! - Offset (`u64`)
//! - Size (`u64`)
//! - Flags (`u32`) — e.g. `2 = encrypted`
//! - Nonce (`[u8; 12]`) — AES‑GCM nonce if encrypted
//! - Tag (`[u8; 16]`) — AES‑GCM authentication tag if encrypted
//!
//! ## Encryption
//! - Uses **AES‑256‑GCM** for integrity + confidentiality.
//! - By default, only critical files are encrypted:
//!   - `project.toml` (always)
//!   - `.scn` (scenes)
//!   - `.fur` (UI layouts)
//!   - `.toml` (configs)
//! - Other assets (textures, audio, video) are left unencrypted for performance/streaming.
//!
//! ## Why "BaRK"?
//! Because Perro is a dog 🐕, and dogs bark — so your assets are packed into a **BaRK**.

use std::fs::File;
use std::io::{self, Write, Seek, SeekFrom};
use std::path::Path;
use walkdir::WalkDir;

use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use rand::RngCore;

/// File types to skip entirely
const SKIP_EXTENSIONS: &[&str] = &["pup", "rs", "cs", "ts"];
/// File types to encrypt
const ENCRYPT_EXTENSIONS: &[&str] = &["scn", "fur", "toml"];

/// Archive header
pub struct BrkHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub file_count: u32,
    pub index_offset: u64,
}

/// Entry metadata written into the index
pub struct BrkEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
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

/// Build a `.brk` archive from a resource directory + project root
pub fn build_brk(output: &Path, res_dir: &Path, project_root: &Path, key: &[u8; 32]) -> io::Result<()> {
    let mut file = File::create(output)?;

    // Write placeholder header
    let header = BrkHeader {
        magic: *b"BRK1",
        version: 1,
        file_count: 0,
        index_offset: 0,
    };
    write_header(&mut file, &header)?;

    let mut entries = Vec::new();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    // 1. Explicitly add project.toml (always encrypted)
    let project_toml = project_root.join("project.toml");
    if project_toml.exists() {
        let mut data = std::fs::read(&project_toml)?;
        let mut nonce = [0u8; 12];
        let mut tag = [0u8; 16];
        let mut flags = 0;

        rand::thread_rng().fill_bytes(&mut nonce);
        let nonce_obj = Nonce::from_slice(&nonce);
        let encrypted = cipher.encrypt(nonce_obj, data.as_ref())
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Encryption failed"))?;
        let (ciphertext, gcm_tag) = encrypted.split_at(encrypted.len() - 16);
        data = ciphertext.to_vec();
        tag.copy_from_slice(gcm_tag);
        flags |= 2; // mark as encrypted

        let offset = file.seek(SeekFrom::Current(0))?;
        file.write_all(&data)?;
        let size = data.len() as u64;

        entries.push(BrkEntry {
            path: "project.toml".to_string(),
            offset,
            size,
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

            // Skip unwanted extensions
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SKIP_EXTENSIONS.contains(&ext) {
                    continue;
                }
            }

            let rel = path.strip_prefix(res_dir).unwrap().to_string_lossy().to_string();
            let mut data = std::fs::read(path)?;
            let mut flags = 0;
            let mut nonce = [0u8; 12];
            let mut tag = [0u8; 16];

            // Encrypt if extension matches
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ENCRYPT_EXTENSIONS.contains(&ext) {
                    rand::thread_rng().fill_bytes(&mut nonce);
                    let nonce_obj = Nonce::from_slice(&nonce);
                    let encrypted = cipher.encrypt(nonce_obj, data.as_ref())
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Encryption failed"))?;
                    let (ciphertext, gcm_tag) = encrypted.split_at(encrypted.len() - 16);
                    data = ciphertext.to_vec();
                    tag.copy_from_slice(gcm_tag);
                    flags |= 2;
                }
            }

            let offset = file.seek(SeekFrom::Current(0))?;
            file.write_all(&data)?;
            let size = data.len() as u64;

            entries.push(BrkEntry {
                path: format!("res/{}", rel.replace("\\", "/")),
                offset,
                size,
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