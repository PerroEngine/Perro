use rayon::prelude::*;
use std::{
    fs,
    fs::File,
    io::{self, Seek, SeekFrom, Write},
    path::Path,
};

use super::common::{
    BRK_MAGIC, BrkEntryMeta, BrkHeader, FLAG_COMPRESSED, write_header, write_index_entry,
};
use crate::compress_deflate_best;

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
const SKIP_RESOURCES: &[&str] = &["pmat", "pmesh"];

fn should_skip(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("");
    SKIP_SCRIPT_EXT.contains(&ext)
        || SKIP_SCENE_FUR_EXT.contains(&ext)
        || SKIP_IMAGES.contains(&ext)
        || SKIP_MODELS.contains(&ext)
        || SKIP_RESOURCES.contains(&ext)
}

#[derive(Debug, Clone)]
struct BrkEntry {
    path: String,
    meta: BrkEntryMeta,
}

struct ProcessedFile {
    rel_path: String,
    data: Vec<u8>,
    flags: u32,
    original_size: u64,
}

/// Build a `.brk` archive
pub fn build_brk(output: &Path, res_dir: &Path, _project_root: &Path) -> io::Result<()> {
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
    let mut rel_paths = crate::collect_file_paths(res_dir, res_dir)?
        .into_iter()
        .map(|rel| rel.replace('\\', "/"))
        .filter(|rel| !should_skip(rel))
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

        entries.push(BrkEntry {
            path: format!("res/{rel_path}"),
            meta: BrkEntryMeta {
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
    let header = BrkHeader {
        magic: BRK_MAGIC,
        version: 1,
        file_count: entries.len() as u32,
        index_offset,
    };
    write_header(&mut file, &header)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_skip;

    #[test]
    fn pmat_is_skipped_as_compiled_resource() {
        assert!(should_skip("materials/mat.pmat"));
        assert!(should_skip("scene/main.scn"));
        assert!(should_skip("mesh/robot.glb"));
        assert!(!should_skip("audio/music.ogg"));
    }
}
