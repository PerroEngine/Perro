use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use rand::RngCore;
use std::{
    fs::{self, File},
    io::{self, Seek, SeekFrom, Write},
    path::Path,
};
use walkdir::WalkDir;
use zstd::encode_all;

use super::common::{
    BRK_MAGIC, BrkEntryMeta, BrkHeader, FLAG_COMPRESSED, FLAG_ENCRYPTED, write_header,
    write_index_entry,
};

// Scripts (compiled into binary)
const SKIP_SCRIPT_EXT: &[&str] = &["pup"];

// Scene and UI data (compiled into scenes.rs and fur.rs)
const SKIP_SCENE_FUR_EXT: &[&str] = &["scn", "fur"];

// Images are pre-decoded + Zstd-compressed into .ptex static assets
const SKIP_IMAGES: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "gif", "ico", "tga", "webp", "rgba",
];

// Models are converted to pmesh (static assets) in release
const SKIP_MODELS: &[&str] = &["glb", "gltf"];

fn should_skip(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    SKIP_SCRIPT_EXT.contains(&ext)
        || SKIP_SCENE_FUR_EXT.contains(&ext)
        || SKIP_IMAGES.contains(&ext)
        || SKIP_MODELS.contains(&ext)
}

#[derive(Debug, Clone)]
struct BrkEntry {
    path: String,
    meta: BrkEntryMeta,
}

/// Build a `.brk` archive
pub fn build_brk(
    output: &Path,
    res_dir: &Path,
    _project_root: &Path,
    key: &[u8; 32],
) -> io::Result<()> {
    let mut file = File::create(output)?;

    // Write placeholder header
    let header = BrkHeader {
        magic: BRK_MAGIC,
        version: 1,
        file_count: 0,
        index_offset: 0,
    };
    write_header(&mut file, &header)?;

    let mut entries = Vec::new();
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    // ZSTD compression level (1-22)
    const COMPRESSION_LEVEL: i32 = 15;

    // Helper closure to process data (compress, then encrypt if needed)
    let process_data = |mut data: Vec<u8>,
                        should_compress: bool|
     -> io::Result<(Vec<u8>, u32, [u8; 12], [u8; 16], u64)> {
        let original_data_len = data.len() as u64;
        let mut flags = 0;
        let mut nonce = [0u8; 12];
        let mut tag = [0u8; 16];

        // --- COMPRESSION STEP ---
        if should_compress && original_data_len > 0 {
            // Only compress if flagged and not empty
            let compressed_data = encode_all(&*data, COMPRESSION_LEVEL).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("ZSTD compression failed: {}", e),
                )
            })?;

            rand::rng().fill_bytes(&mut nonce);
            let nonce_obj = Nonce::from_slice(&nonce);
            let encrypted = cipher
                .encrypt(nonce_obj, &*data)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Encryption failed"))?;
            let (ciphertext, gcm_tag) = encrypted.split_at(encrypted.len() - 16);
            data = ciphertext.to_vec();
            tag.copy_from_slice(gcm_tag);
            flags |= FLAG_ENCRYPTED;

            // Only use compressed data if it's actually smaller.
            // Zstd has a header, so very tiny files might get slightly larger.
            if compressed_data.len() < data.len() {
                data = compressed_data;
                flags |= FLAG_COMPRESSED;
            }
        }

        Ok((data, flags, nonce, tag, original_data_len))
    };

    for entry in WalkDir::new(res_dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path();

            // Skip extensions that are statically compiled
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if should_skip(ext) {
                    continue;
                }
            }

            let rel = path
                .strip_prefix(res_dir)
                .unwrap()
                .to_string_lossy()
                .to_string();

            // Read file data (no more minification since .scn and .fur are skipped)
            let data = fs::read(path)?;

            // Always try Zstd; we only keep compressed data when it's smaller (see process_data).
            // Things that are preprocessed (textures → rgb8, meshes → pmesh) are static assets and skipped above.
            let should_compress = true;

            let (processed_data, flags, nonce, tag, original_size) =
                process_data(data, should_compress)?;

            let offset = file.seek(SeekFrom::Current(0))?;
            file.write_all(&processed_data)?;
            let size = processed_data.len() as u64;

            entries.push(BrkEntry {
                path: format!("res/{}", rel.replace("\\", "/")),
                meta: BrkEntryMeta {
                    offset,
                    size,
                    original_size,
                    flags,
                    nonce,
                    tag,
                },
            });
        }
    }

    // 3. Write index
    let index_offset = file.seek(SeekFrom::Current(0))?;
    for e in &entries {
        write_index_entry(&mut file, &e.path, &e.meta)?;
    }

    // 4. Rewrite header with correct counts
    file.seek(SeekFrom::Start(0))?;
    let header = BrkHeader {
        magic: BRK_MAGIC,
        version: 1,
        file_count: entries.len() as u32,
        index_offset,
    };
    write_header(&mut file, &header)?;

    Ok(())
}
