use rayon::prelude::*;
use std::{
    collections::HashSet,
    fs,
    fs::File,
    io::{self, Cursor, Seek, SeekFrom, Write},
    path::Path,
};

use super::common::{
    FLAG_COMPRESSED, PERRO_ASSETS_COMPRESSED_MAGIC, PERRO_ASSETS_MAGIC, PerroAssetsEntryMeta,
    PerroAssetsHeader, write_header, write_index_entry,
};
use crate::compression::compress_zlib_best;
use crate::walkdir::collect_file_paths;
use perro_asset_formats::{archive, source_ext};

fn should_skip(path: &str, extra_skip_rel_paths: &HashSet<&str>) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    extra_skip_rel_paths.contains(path)
        || ext.as_deref().is_some_and(|ext| {
            ext.eq_ignore_ascii_case(source_ext::RUST_SCRIPT)
                || source_ext::contains(source_ext::SCENE_FUR, ext)
                || source_ext::contains(source_ext::IMAGE, ext)
                || source_ext::contains(source_ext::MODEL, ext)
                || source_ext::contains(source_ext::STATIC_RESOURCE, ext)
                || source_ext::contains(source_ext::SHADER, ext)
                || source_ext::contains(source_ext::AUDIO, ext)
                || source_ext::contains(source_ext::MIDI, ext)
                || source_ext::contains(source_ext::SOUNDFONT, ext)
                || source_ext::contains(source_ext::FONT, ext)
        })
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
        version: archive::VERSION,
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
                let compressed = compress_zlib_best(&data)?;

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
        version: archive::VERSION,
        file_count: entries.len() as u32,
        index_offset,
    };
    write_header(&mut file, &header)?;

    Ok(())
}

/// Build a generic `.perro` archive from explicit `(virtual_path, source_file)` entries.
pub fn build_perro_archive_from_entries(
    output: &Path,
    entries: &[(String, std::path::PathBuf)],
) -> io::Result<()> {
    let mut file = File::create(output)?;
    write_perro_archive_from_entries(&mut file, entries, true)
}

/// Build a generic `.perro` archive, then wrap the full archive in zlib when smaller.
pub fn build_compressed_perro_archive_from_entries(
    output: &Path,
    entries: &[(String, std::path::PathBuf)],
) -> io::Result<()> {
    let mut raw = Cursor::new(Vec::<u8>::new());
    write_perro_archive_from_entries(&mut raw, entries, false)?;
    let raw = raw.into_inner();
    let raw_wrapped = wrap_compressed_archive(&raw)?;

    let mut entry_compressed = Cursor::new(Vec::<u8>::new());
    write_perro_archive_from_entries(&mut entry_compressed, entries, true)?;
    let entry_compressed = entry_compressed.into_inner();

    let best = [raw, raw_wrapped, entry_compressed]
        .into_iter()
        .min_by_key(Vec::len)
        .unwrap();

    fs::write(output, best)?;
    Ok(())
}

fn wrap_compressed_archive(raw: &[u8]) -> io::Result<Vec<u8>> {
    let compressed = compress_zlib_best(raw)?;
    let mut out = Vec::with_capacity(16 + compressed.len());
    out.extend_from_slice(&PERRO_ASSETS_COMPRESSED_MAGIC);
    out.extend_from_slice(&archive::VERSION.to_le_bytes());
    out.extend_from_slice(&(raw.len() as u64).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

fn write_perro_archive_from_entries<W: Write + Seek>(
    writer: &mut W,
    entries: &[(String, std::path::PathBuf)],
    compress_entries: bool,
) -> io::Result<()> {
    let header = PerroAssetsHeader {
        magic: PERRO_ASSETS_MAGIC,
        version: archive::VERSION,
        file_count: 0,
        index_offset: 0,
    };
    write_header(writer, &header)?;

    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let processed = sorted
        .into_par_iter()
        .map(
            |(virtual_path, source_path)| -> io::Result<(String, Vec<u8>, u32, u64)> {
                let data = fs::read(&source_path)?;
                let original_size = data.len() as u64;
                let compressed = if compress_entries {
                    compress_zlib_best(&data)?
                } else {
                    Vec::new()
                };
                if compress_entries && compressed.len() < data.len() {
                    Ok((virtual_path, compressed, FLAG_COMPRESSED, original_size))
                } else {
                    Ok((virtual_path, data, 0, original_size))
                }
            },
        )
        .collect::<io::Result<Vec<_>>>()?;

    let mut index_entries = Vec::<PerroAssetsEntry>::with_capacity(processed.len());
    for (virtual_path, data, flags, original_size) in processed {
        let offset = writer.stream_position()?;
        writer.write_all(&data)?;
        index_entries.push(PerroAssetsEntry {
            path: virtual_path,
            meta: PerroAssetsEntryMeta {
                offset,
                size: data.len() as u64,
                original_size,
                flags,
            },
        });
    }

    let index_offset = writer.stream_position()?;
    for entry in &index_entries {
        write_index_entry(writer, &entry.path, &entry.meta)?;
    }

    writer.seek(SeekFrom::Start(0))?;
    let header = PerroAssetsHeader {
        magic: PERRO_ASSETS_MAGIC,
        version: archive::VERSION,
        file_count: index_entries.len() as u32,
        index_offset,
    };
    write_header(writer, &header)?;
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/packer_tests.rs"]
mod tests;
