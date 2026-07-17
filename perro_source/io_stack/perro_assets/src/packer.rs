use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Cursor, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use super::common::{
    FLAG_COMPRESSED, PERRO_ASSETS_COMPRESSED_MAGIC, PERRO_ASSETS_MAGIC, PerroAssetsEntryMeta,
    PerroAssetsHeader, read_header, read_index_entry, write_header, write_index_entry,
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
    // (len, mtime nanos) recorded in the stat sidecar; None skips recording
    // (unreadable stat, or the file changed between stat and read).
    stat: Option<(u64, u128)>,
}

/// Stat sidecar version; bump alongside any change to what a cached entry
/// means (compression codec, entry layout) so stale sidecars self-invalidate.
const ASSETS_STAT_VERSION: u32 = 1;

fn stat_manifest_header() -> String {
    format!(
        "perro-assets-stat v{ASSETS_STAT_VERSION} archive-v{}",
        archive::VERSION
    )
}

/// `<output>.stat` sidecar recording each packed source's (len, mtime), which
/// lets the next build reuse already-compressed bytes out of the previous
/// archive instead of re-reading + re-deflating the whole res tree.
fn stat_sidecar_path(output: &Path) -> PathBuf {
    let mut name = output.file_name().unwrap_or_default().to_os_string();
    name.push(".stat");
    output.with_file_name(name)
}

fn file_stat(path: &Path) -> Option<(u64, u128)> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), mtime))
}

/// Previous archive + its stat sidecar, loaded for compressed-byte reuse.
struct ReusedArchive {
    stats: HashMap<String, (u64, u128)>,
    index: HashMap<String, PerroAssetsEntryMeta>,
    bytes: Vec<u8>,
}

impl ReusedArchive {
    fn entry_slice(&self, meta: &PerroAssetsEntryMeta) -> Option<&[u8]> {
        let start = usize::try_from(meta.offset).ok()?;
        let size = usize::try_from(meta.size).ok()?;
        self.bytes.get(start..start.checked_add(size)?)
    }
}

fn load_reuse_archive(output: &Path, stat_path: &Path) -> Option<ReusedArchive> {
    let text = fs::read_to_string(stat_path).ok()?;
    let mut lines = text.lines();
    if lines.next()? != stat_manifest_header() {
        return None;
    }
    let mut stats = HashMap::new();
    for line in lines {
        let mut parts = line.splitn(3, '\t');
        let len = parts.next()?.parse().ok()?;
        let mtime = parts.next()?.parse().ok()?;
        stats.insert(parts.next()?.to_string(), (len, mtime));
    }
    let bytes = fs::read(output).ok()?;
    let mut cursor = Cursor::new(bytes.as_slice());
    let header = read_header(&mut cursor).ok()?;
    cursor.seek(SeekFrom::Start(header.index_offset)).ok()?;
    let mut index = HashMap::with_capacity(header.file_count as usize);
    for _ in 0..header.file_count {
        let (path, meta) = read_index_entry(&mut cursor).ok()?;
        index.insert(path, meta);
    }
    Some(ReusedArchive {
        stats,
        index,
        bytes,
    })
}

fn write_stat_manifest(path: &Path, files: &[ProcessedFile]) -> io::Result<()> {
    let mut out = stat_manifest_header();
    out.push('\n');
    for file in files {
        if let Some((len, mtime)) = file.stat {
            out.push_str(&format!("{len}\t{mtime}\t{}\n", file.rel_path));
        }
    }
    write_output_if_changed(path, out.as_bytes())
}

/// Write only when bytes differ; unchanged archives keep their mtime so
/// `include_bytes!` consumers are not re-fingerprinted by cargo.
fn write_output_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Ok(meta) = fs::metadata(path)
        && meta.len() == bytes.len() as u64
        && let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return Ok(());
    }
    fs::write(path, bytes)
}

/// Build a `.perro` archive.
///
/// Incremental: a `<output>.stat` sidecar records each source's (len, mtime);
/// sources whose stat is unchanged reuse their compressed bytes from the
/// previous archive instead of being re-read and re-deflated.
pub fn build_perro_assets_archive(
    output: &Path,
    res_dir: &Path,
    _project_root: &Path,
    extra_skip_rel_paths: &[String],
) -> io::Result<()> {
    let extra_skip_set: HashSet<&str> = extra_skip_rel_paths.iter().map(String::as_str).collect();

    // Collect file paths and process file bytes/compression in parallel.
    let mut rel_paths = collect_file_paths(res_dir, res_dir)?
        .into_iter()
        .map(|rel| rel.replace('\\', "/"))
        .filter(|rel| !should_skip(rel, &extra_skip_set))
        .collect::<Vec<_>>();
    rel_paths.sort();

    let stat_path = stat_sidecar_path(output);
    let reuse = load_reuse_archive(output, &stat_path);

    let processed_files = rel_paths
        .into_par_iter()
        .map(|rel_path| -> io::Result<ProcessedFile> {
            let full_path = res_dir.join(&rel_path);
            let stat = file_stat(&full_path);
            // Unchanged stat: lift the already-compressed bytes straight out
            // of the previous archive.
            if let (Some(reuse), Some(stat)) = (reuse.as_ref(), stat)
                && reuse.stats.get(&rel_path) == Some(&stat)
                && let Some(meta) = reuse.index.get(&format!("res/{rel_path}"))
                && let Some(data) = reuse.entry_slice(meta)
            {
                return Ok(ProcessedFile {
                    rel_path,
                    data: data.to_vec(),
                    flags: meta.flags,
                    original_size: meta.original_size,
                    stat: Some(stat),
                });
            }
            let mut data = fs::read(&full_path)?;
            // A length mismatch means the file changed between stat and read;
            // drop the stat so the next build re-encodes instead of reusing.
            let stat = stat.filter(|(len, _)| *len == data.len() as u64);
            let original_size = data.len() as u64;
            let mut flags = 0;
            if original_size > 0 {
                let compressed = compress_zlib_best(&data)?;
                // Only use compressed data if it's actually smaller
                if compressed.len() < data.len() {
                    data = compressed;
                    flags |= FLAG_COMPRESSED;
                }
            }
            Ok(ProcessedFile {
                rel_path,
                data,
                flags,
                original_size,
                stat,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let mut archive = Cursor::new(Vec::<u8>::new());
    let header = PerroAssetsHeader {
        magic: PERRO_ASSETS_MAGIC,
        version: archive::VERSION,
        file_count: 0,
        index_offset: 0,
    };
    write_header(&mut archive, &header)?;

    let mut entries = Vec::new();
    for processed in &processed_files {
        let offset = archive.stream_position()?;
        archive.write_all(&processed.data)?;

        entries.push(PerroAssetsEntry {
            path: format!("res/{}", processed.rel_path),
            meta: PerroAssetsEntryMeta {
                offset,
                size: processed.data.len() as u64,
                original_size: processed.original_size,
                flags: processed.flags,
            },
        });
    }

    // Write index
    let index_offset = archive.stream_position()?;
    for e in &entries {
        write_index_entry(&mut archive, &e.path, &e.meta)?;
    }

    // Rewrite header with correct counts
    archive.seek(SeekFrom::Start(0))?;
    let header = PerroAssetsHeader {
        magic: PERRO_ASSETS_MAGIC,
        version: archive::VERSION,
        file_count: entries.len() as u32,
        index_offset,
    };
    write_header(&mut archive, &header)?;

    write_output_if_changed(output, &archive.into_inner())?;
    write_stat_manifest(&stat_path, &processed_files)
}

/// Build a generic `.perro` archive from explicit `(virtual_path, source_file)` entries.
pub fn build_perro_archive_from_entries(
    output: &Path,
    entries: &[(String, std::path::PathBuf)],
) -> io::Result<()> {
    let read = read_archive_entries(entries)?;
    let mut archive = Cursor::new(Vec::<u8>::new());
    write_perro_archive_from_bytes(&mut archive, &read, true)?;
    write_output_if_changed(output, &archive.into_inner())
}

/// Build a generic `.perro` archive, then wrap the full archive in zlib when smaller.
///
/// Sources are read and compressed once; the two candidates are a per-entry
/// compressed archive and a whole-archive zlib wrap (which can win on
/// cross-file redundancy). A fully raw archive can never beat the per-entry
/// candidate — entries only stay compressed when smaller — so it is not built.
pub fn build_compressed_perro_archive_from_entries(
    output: &Path,
    entries: &[(String, std::path::PathBuf)],
) -> io::Result<()> {
    let read = read_archive_entries(entries)?;

    let mut raw = Cursor::new(Vec::<u8>::new());
    write_perro_archive_from_bytes(&mut raw, &read, false)?;
    let raw_wrapped = wrap_compressed_archive(&raw.into_inner())?;

    let mut entry_compressed = Cursor::new(Vec::<u8>::new());
    write_perro_archive_from_bytes(&mut entry_compressed, &read, true)?;
    let entry_compressed = entry_compressed.into_inner();

    let best = if raw_wrapped.len() < entry_compressed.len() {
        raw_wrapped
    } else {
        entry_compressed
    };
    write_output_if_changed(output, &best)
}

struct ReadArchiveEntry {
    virtual_path: String,
    raw: Vec<u8>,
    // Present only when zlib actually shrank the payload.
    compressed: Option<Vec<u8>>,
}

/// Read and per-entry-compress every source exactly once, sorted by path.
fn read_archive_entries(
    entries: &[(String, std::path::PathBuf)],
) -> io::Result<Vec<ReadArchiveEntry>> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    sorted
        .into_par_iter()
        .map(|(virtual_path, source_path)| -> io::Result<ReadArchiveEntry> {
            let raw = fs::read(&source_path)?;
            let compressed = compress_zlib_best(&raw)?;
            let compressed = (compressed.len() < raw.len()).then_some(compressed);
            Ok(ReadArchiveEntry {
                virtual_path,
                raw,
                compressed,
            })
        })
        .collect()
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

fn write_perro_archive_from_bytes<W: Write + Seek>(
    writer: &mut W,
    entries: &[ReadArchiveEntry],
    compress_entries: bool,
) -> io::Result<()> {
    let header = PerroAssetsHeader {
        magic: PERRO_ASSETS_MAGIC,
        version: archive::VERSION,
        file_count: 0,
        index_offset: 0,
    };
    write_header(writer, &header)?;

    let mut index_entries = Vec::<PerroAssetsEntry>::with_capacity(entries.len());
    for entry in entries {
        let (data, flags) = match (&entry.compressed, compress_entries) {
            (Some(compressed), true) => (compressed.as_slice(), FLAG_COMPRESSED),
            _ => (entry.raw.as_slice(), 0),
        };
        let offset = writer.stream_position()?;
        writer.write_all(data)?;
        index_entries.push(PerroAssetsEntry {
            path: entry.virtual_path.clone(),
            meta: PerroAssetsEntryMeta {
                offset,
                size: data.len() as u64,
                original_size: entry.raw.len() as u64,
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
