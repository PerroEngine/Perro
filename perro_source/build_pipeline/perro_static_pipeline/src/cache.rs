//! Incremental helpers for the static pipeline.
//!
//! Two problems make naive generation expensive:
//! 1. Rewriting identical generated files touches their mtimes, which
//!    invalidates cargo's fingerprints and recompiles + relinks the whole
//!    generated project crate even when nothing changed.
//! 2. Re-decoding and re-compressing every asset on every build burns CPU
//!    proportional to the whole project instead of the change.
//!
//! [`write_if_changed`] fixes (1); [`SourceCache`] fixes (2) with a per-kind
//! manifest keyed on source length + mtime, storing whatever per-source
//! metadata rows the generator needs to emit codegen without re-processing.

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Bump when any encoder output changes shape (compression codec, container
/// layout, payload packing) so stale caches self-invalidate.
pub(crate) const PIPELINE_CACHE_VERSION: u32 = 1;

const MANIFEST_FILE: &str = ".perro_manifest";

/// Write `bytes` to `path` only when the on-disk content differs, preserving
/// the mtime of unchanged outputs so downstream cargo fingerprints hold.
pub(crate) fn write_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Ok(meta) = fs::metadata(path)
        && meta.len() == bytes.len() as u64
        && let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return Ok(());
    }
    fs::write(path, bytes)
}

/// Stat key for a source file: (len, mtime nanos). `None` when the file is
/// unreadable, which callers treat as a cache miss.
pub(crate) fn source_stat(path: &Path) -> Option<(u64, u128)> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), mtime))
}

/// Per-source cache record: free-form metadata rows (generator-defined) plus
/// the dir-relative blob files this source produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CachedSource {
    pub rows: Vec<Vec<String>>,
    pub files: Vec<String>,
}

struct ManifestEntry {
    len: u64,
    mtime: u128,
    source: CachedSource,
}

/// Manifest-backed cache for one generator's embedded output dir.
///
/// Owns stale-file pruning for the dir: every file not produced or reused in
/// the current run is deleted by [`SourceCache::finish`], which replaces the
/// old whole-dir `remove_dir_all` reset.
pub(crate) struct SourceCache {
    dir: PathBuf,
    context: String,
    old: HashMap<String, ManifestEntry>,
    fresh: Vec<(String, ManifestEntry)>,
    current_files: HashSet<String>,
}

impl SourceCache {
    /// Load the dir's manifest. `context` folds in anything besides source
    /// bytes that affects encoding (cache version, flags like meshlet baking);
    /// a mismatch discards the old manifest wholesale.
    pub fn open(dir: &Path, context: &str) -> Self {
        let context = format!("v{PIPELINE_CACHE_VERSION} {context}");
        let old = read_manifest(&dir.join(MANIFEST_FILE), &context).unwrap_or_default();
        Self {
            dir: dir.to_path_buf(),
            context,
            old,
            fresh: Vec::new(),
            current_files: HashSet::new(),
        }
    }

    /// Cache hit iff the stat key matches and every recorded output file still
    /// exists. On hit the entry carries over to the new manifest and its files
    /// count as current for pruning.
    pub fn lookup(&mut self, rel: &str, len: u64, mtime: u128) -> Option<CachedSource> {
        let entry = self.old.get(rel)?;
        if entry.len != len || entry.mtime != mtime {
            return None;
        }
        if !entry
            .source
            .files
            .iter()
            .all(|file| self.dir.join(file).is_file())
        {
            return None;
        }
        let entry = self.old.remove(rel)?;
        let source = entry.source.clone();
        self.current_files.extend(source.files.iter().cloned());
        self.fresh.push((rel.to_string(), entry));
        Some(source)
    }

    /// Record a freshly processed source. The caller has already written the
    /// blob files (via [`write_if_changed`]).
    pub fn store(&mut self, rel: &str, len: u64, mtime: u128, source: CachedSource) {
        self.current_files.extend(source.files.iter().cloned());
        self.fresh
            .push((rel.to_string(), ManifestEntry { len, mtime, source }));
    }

    /// Delete files in the dir that no current source produced, then persist
    /// the manifest (only when its bytes changed).
    pub fn finish(mut self) -> io::Result<()> {
        prune_dir(&self.dir, &self.dir, &self.current_files)?;
        self.fresh.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out = String::new();
        out.push_str(&self.context);
        out.push('\n');
        for (rel, entry) in &self.fresh {
            out.push_str(&format!(
                "S\t{rel}\t{}\t{}\t{}\t{}\n",
                entry.len,
                entry.mtime,
                entry.source.rows.len(),
                entry.source.files.len()
            ));
            for row in &entry.source.rows {
                out.push('R');
                for field in row {
                    out.push('\t');
                    out.push_str(field);
                }
                out.push('\n');
            }
            for file in &entry.source.files {
                out.push_str(&format!("F\t{file}\n"));
            }
        }
        write_if_changed(&self.dir.join(MANIFEST_FILE), out.as_bytes())
    }
}

/// Stale-file cleanup for generators cheap enough to skip the manifest cache:
/// removes everything in `dir` except `keep` (dir-relative, `/`-separated).
pub(crate) fn prune_embedded_dir(dir: &Path, keep: &HashSet<String>) -> io::Result<()> {
    prune_dir(dir, dir, keep)
}

fn read_manifest(path: &Path, context: &str) -> Option<HashMap<String, ManifestEntry>> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    if lines.next()? != context {
        return None;
    }
    let mut entries = HashMap::new();
    let mut lines = lines.peekable();
    while let Some(line) = lines.next() {
        let mut parts = line.split('\t');
        if parts.next()? != "S" {
            return None;
        }
        let rel = parts.next()?.to_string();
        let len = parts.next()?.parse().ok()?;
        let mtime = parts.next()?.parse().ok()?;
        let n_rows: usize = parts.next()?.parse().ok()?;
        let n_files: usize = parts.next()?.parse().ok()?;
        let mut source = CachedSource::default();
        for _ in 0..n_rows {
            let row = lines.next()?;
            let mut fields = row.split('\t');
            if fields.next()? != "R" {
                return None;
            }
            source.rows.push(fields.map(str::to_string).collect());
        }
        for _ in 0..n_files {
            let file = lines.next()?;
            source.files.push(file.strip_prefix("F\t")?.to_string());
        }
        entries.insert(rel, ManifestEntry { len, mtime, source });
    }
    Some(entries)
}

fn prune_dir(root: &Path, dir: &Path, keep: &HashSet<String>) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    let mut remaining = false;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            prune_dir(root, &path, keep)?;
            if fs::read_dir(&path)?.next().is_none() {
                fs::remove_dir(&path)?;
            } else {
                remaining = true;
            }
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if rel == MANIFEST_FILE || keep.contains(&rel) {
            remaining = true;
            continue;
        }
        fs::remove_file(&path)?;
    }
    let _ = remaining;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_if_changed_preserves_mtime_for_identical_bytes() {
        let dir = std::env::temp_dir().join(format!("perro_wic_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.rs");
        write_if_changed(&path, b"hello").unwrap();
        let first = fs::metadata(&path).unwrap().modified().unwrap();
        write_if_changed(&path, b"hello").unwrap();
        assert_eq!(first, fs::metadata(&path).unwrap().modified().unwrap());
        write_if_changed(&path, b"hello2").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hello2");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn source_cache_round_trips_and_prunes_stale_files() {
        let dir = std::env::temp_dir().join(format!("perro_cache_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("kept.bin"), b"kept").unwrap();
        fs::write(dir.join("sub/stale.bin"), b"stale").unwrap();

        let mut cache = SourceCache::open(&dir, "test");
        assert!(cache.lookup("a.png", 10, 20).is_none());
        cache.store(
            "a.png",
            10,
            20,
            CachedSource {
                rows: vec![vec!["res://a.png".into(), "kept.bin".into()]],
                files: vec!["kept.bin".into()],
            },
        );
        cache.finish().unwrap();

        assert!(dir.join("kept.bin").is_file());
        assert!(!dir.join("sub").exists(), "stale subdir should be pruned");

        let mut cache = SourceCache::open(&dir, "test");
        let hit = cache.lookup("a.png", 10, 20).expect("stat-matched hit");
        assert_eq!(hit.rows[0][0], "res://a.png");
        assert!(cache.lookup("a.png", 10, 20).is_none(), "consumed");

        let mut cache = SourceCache::open(&dir, "test");
        assert!(cache.lookup("a.png", 11, 20).is_none(), "len mismatch");

        let mut cache = SourceCache::open(&dir, "other-context");
        assert!(
            cache.lookup("a.png", 10, 20).is_none(),
            "context change invalidates"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
