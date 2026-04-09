use rayon::prelude::*;
use std::{
    collections::HashSet,
    fs,
    fs::File,
    io::{self, Seek, SeekFrom, Write},
    path::Path,
};

use super::common::{
    FLAG_COMPRESSED, PERRO_ASSETS_MAGIC, PerroAssetsEntryMeta, PerroAssetsHeader, write_header,
    write_index_entry,
};
use crate::compression::compress_deflate_best;
use crate::walkdir::collect_file_paths;

// Scripts (compiled into binary)
const SKIP_SCRIPT_EXT: &[&str] = &["rs"];

// Scene and UI data (compiled into scenes.rs and fur.rs)
const SKIP_SCENE_FUR_EXT: &[&str] = &["scn", "fur"];

// Images are pre-decoded + compressed into .ptex static assets
const SKIP_IMAGES: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "gif", "ico", "tga", "webp", "rgba",
];

// Models are converted to pmesh (static assets) in release
const SKIP_MODELS: &[&str] = &["glb", "gltf"];

// Resources compiled into static runtime tables
const SKIP_RESOURCES: &[&str] = &["pmat", "ppart", "pmesh", "panim", "ptchunk", "pterr"];
// Shaders are compiled into static shader tables
const SKIP_SHADERS: &[&str] = &["wgsl"];
const SKIP_AUDIO: &[&str] = &["mp3", "wav", "ogg", "flac", "aac", "m4a"];

fn should_skip(path: &str, extra_skip_rel_paths: &HashSet<&str>) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    extra_skip_rel_paths.contains(path)
        || ext
            .as_deref()
            .is_some_and(|ext| SKIP_SCRIPT_EXT.contains(&ext))
        || ext
            .as_deref()
            .is_some_and(|ext| SKIP_SCENE_FUR_EXT.contains(&ext))
        || ext.as_deref().is_some_and(|ext| SKIP_IMAGES.contains(&ext))
        || ext.as_deref().is_some_and(|ext| SKIP_MODELS.contains(&ext))
        || ext
            .as_deref()
            .is_some_and(|ext| SKIP_RESOURCES.contains(&ext))
        || ext.as_deref().is_some_and(|ext| SKIP_SHADERS.contains(&ext))
        || ext.as_deref().is_some_and(|ext| SKIP_AUDIO.contains(&ext))
}

#[derive(Debug, Clone)]
struct PerroAssetsEntry {
    path: String,
    meta: PerroAssetsEntryMeta,
}

struct ProcessedFile {
    rel_path: String,
    data: Vec<u8>,
    flags: u32,
    original_size: u64,
}

/// Build a `.perro` archive
pub fn build_perro_assets_archive(
    output: &Path,
    res_dir: &Path,
    _project_root: &Path,
    extra_skip_rel_paths: &[String],
) -> io::Result<()> {
    let mut file = File::create(output)?;
    let extra_skip_set: HashSet<&str> = extra_skip_rel_paths.iter().map(String::as_str).collect();

    // Write placeholder header
    let header = PerroAssetsHeader {
        magic: PERRO_ASSETS_MAGIC,
        version: 1,
        file_count: 0,
        index_offset: 0,
    };
    write_header(&mut file, &header)?;

    let mut entries = Vec::new();

    // Helper to process data (compress if beneficial)
    let process_data =
        |mut data: Vec<u8>, should_compress: bool| -> io::Result<(Vec<u8>, u32, u64)> {
            let original_data_len = data.len() as u64;
            let mut flags = 0;

            if should_compress && original_data_len > 0 {
                let compressed = compress_deflate_best(&data)?;

                // Only use compressed data if it's actually smaller
                if compressed.len() < data.len() {
                    data = compressed;
                    flags |= FLAG_COMPRESSED;
                }
            }

            Ok((data, flags, original_data_len))
        };

    // Collect file paths and process file bytes/compression in parallel.
    let mut rel_paths = collect_file_paths(res_dir, res_dir)?
        .into_iter()
        .map(|rel| rel.replace('\\', "/"))
        .filter(|rel| !should_skip(rel, &extra_skip_set))
        .collect::<Vec<_>>();
    rel_paths.sort();

    let processed_files = rel_paths
        .into_par_iter()
        .map(|rel_path| -> io::Result<ProcessedFile> {
            let full_path = res_dir.join(&rel_path);
            let data = fs::read(&full_path)?;
            let (processed_data, flags, original_size) = process_data(data, true)?;
            Ok(ProcessedFile {
                rel_path,
                data: processed_data,
                flags,
                original_size,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    for processed in processed_files {
        let ProcessedFile {
            rel_path,
            data,
            flags,
            original_size,
        } = processed;

        let offset = file.stream_position()?;
        file.write_all(&data)?;
        let size = data.len() as u64;

        entries.push(PerroAssetsEntry {
            path: format!("res/{rel_path}"),
            meta: PerroAssetsEntryMeta {
                offset,
                size,
                original_size,
                flags,
            },
        });
    }

    // Write index
    let index_offset = file.stream_position()?;
    for e in &entries {
        write_index_entry(&mut file, &e.path, &e.meta)?;
    }

    // Rewrite header with correct counts
    file.seek(SeekFrom::Start(0))?;
    let header = PerroAssetsHeader {
        magic: PERRO_ASSETS_MAGIC,
        version: 1,
        file_count: entries.len() as u32,
        index_offset,
    };
    write_header(&mut file, &header)?;

    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/packer_tests.rs"]
mod tests;
