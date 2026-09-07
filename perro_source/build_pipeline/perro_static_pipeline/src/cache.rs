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
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    fs,
    hash::Hasher,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::UNIX_EPOCH,
};

/// Bump when any encoder output changes shape (compression codec, container
/// layout, payload packing) so stale caches self-invalidate.
/// v2: flush bakes produced before SVG_RASTER_SCALE entered the textures
/// cache context (4x-era rasters never invalidated on the 4x -> 2x change).
pub(crate) const PIPELINE_CACHE_VERSION: u32 = 2;

const MANIFEST_FILE: &str = ".perro_manifest";

/// Cache a whole text-codegen output by dependency paths, stats and contents.
/// The running executable fingerprint invalidates this cache when codegen
/// changes. Missing optional files are recorded too (for example .pretarget).
/// An edited/deleted generated output or any unreadable dependency is a miss.
pub(crate) struct CodegenCache {
    output: PathBuf,
    sidecar: PathBuf,
    inputs: Option<String>,
}

impl CodegenCache {
    pub fn new(output: &Path, inputs: impl IntoIterator<Item = PathBuf>, context: &str) -> Self {
        let mut paths = inputs.into_iter().collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        let fingerprint = || -> Option<String> {
            // Small codegen costs less than executable hashing and sidecar IO.
            // Check eligibility before either, including on the first call.
            if paths.len() < 8 {
                let bytes = paths
                    .iter()
                    .filter_map(|path| fs::metadata(path).ok())
                    .map(|meta| meta.len())
                    .sum::<u64>();
                if bytes < 16 * 1024 {
                    return None;
                }
            }
            let exe = codegen_executable_fingerprint()?;
            let mut key = format!("codegen-v2 exe={exe} context={context:?}\n");
            for path in paths {
                match content_fingerprint(&path) {
                    Ok((len, modified, hash)) => {
                        key.push_str(&format!("{path:?}\t{len}\t{modified}\t{hash:016x}\n"));
                    }
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {
                        key.push_str(&format!("{path:?}\tmissing\n"));
                    }
                    _ => return None,
                }
            }
            Some(key)
        };
        Self {
            output: output.to_path_buf(),
            sidecar: output.with_file_name(format!(
                ".{}.codegen",
                output.file_name().unwrap_or_default().to_string_lossy()
            )),
            inputs: fingerprint(),
        }
    }

    fn signature(&self) -> Option<String> {
        let inputs = self.inputs.as_ref()?;
        let (len, mtime, hash) = content_fingerprint(&self.output).ok()?;
        Some(format!("{inputs}output={len}:{mtime}:{hash:016x}\n"))
    }

    pub fn hit(&self) -> bool {
        if self.inputs.is_none() {
            return false;
        }
        fs::read_to_string(&self.sidecar)
            .is_ok_and(|old| self.signature().is_some_and(|signature| old == signature))
    }

    /// Record bytes after a successful write, without reopening the new output.
    /// Reopening it can force a synchronous scan by filesystem filters. A hit
    /// still reads and hashes the on-disk output, catching even same-stat edits.
    pub fn store(self, output: &[u8]) {
        let Some(inputs) = self.inputs else { return };
        let Ok(meta) = fs::metadata(&self.output) else {
            return;
        };
        if !meta.is_file() || meta.len() != output.len() as u64 {
            return;
        }
        let Some(mtime) = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        else {
            return;
        };
        let mut hasher = DefaultHasher::new();
        for chunk in output.chunks(64 * 1024) {
            hasher.write(chunk);
        }
        let signature = format!(
            "{inputs}output={}:{}:{:016x}\n",
            output.len(),
            mtime.as_nanos(),
            hasher.finish()
        );
        // Cache persistence is optional; output generation already succeeded.
        let _ = write_if_changed(&self.sidecar, signature.as_bytes());
    }
}

// The executable cannot change its loaded code during this process. Hash it
// once, including its path and stats, instead of rereading it for each kind.
// A failed identity read disables codegen caching for this process.
fn codegen_executable_fingerprint() -> Option<&'static str> {
    static IDENTITY: OnceLock<Option<String>> = OnceLock::new();
    IDENTITY
        .get_or_init(|| {
            let path = std::env::current_exe().ok()?;
            let (len, modified, hash) = content_fingerprint(&path).ok()?;
            Some(format!("{path:?}:{len}:{modified}:{hash:016x}"))
        })
        .as_deref()
}

fn content_fingerprint(path: &Path) -> io::Result<(u64, u128, u64)> {
    let mut file = fs::File::open(path)?;
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(io::Error::other("codegen dependency is not a file"));
    }
    let modified = before.modified()?;
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut len = 0u64;
    loop {
        let read = match file.read(&mut buffer) {
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if read == 0 {
            break;
        }
        hasher.write(&buffer[..read]);
        len += read as u64;
    }
    let after = file.metadata()?;
    if len != before.len() || after.len() != len || after.modified()? != modified {
        return Err(io::Error::other(
            "codegen dependency changed during fingerprint",
        ));
    }
    let modified = modified
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    Ok((len, modified, hasher.finish()))
}

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
    if path.exists() {
        fs::remove_file(path)?;
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
    fn codegen_cache_skips_small_inputs_and_stores_known_output_bytes() {
        let unique = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "perro_codegen_cache_bytes_{}_{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("fixture");
        let source = root.join("source.csv");
        let output = root.join("csvs.rs");
        let sidecar = root.join(".csvs.rs.codegen");
        let probe = || CodegenCache::new(&output, [source.clone()], "csv");
        fs::write(&source, b"small").expect("small source");
        fs::write(&sidecar, b"old cache").expect("stale cache");
        let small = probe();
        assert!(small.inputs.is_none(), "tiny inputs bypass fingerprinting");
        assert!(!small.hit());
        small.store(b"no output");
        assert_eq!(fs::read(&sidecar).expect("sidecar"), b"old cache");
        fs::remove_file(&sidecar).expect("remove stale cache");

        fs::write(&source, vec![b'x'; 16 * 1024]).expect("large source");
        assert!(probe().inputs.is_some());
        probe().store(b"missing output");
        assert!(!sidecar.exists(), "missing output cannot be cached");
        fs::create_dir(&output).expect("output directory");
        probe().store(b"");
        assert!(!sidecar.exists(), "directory cannot be cached");
        fs::remove_dir(&output).expect("remove output directory");
        let bytes = vec![b'y'; 128 * 1024 + 7];
        fs::write(&output, &bytes).expect("multi-chunk output");
        probe().store(b"wrong length");
        assert!(!sidecar.exists(), "length mismatch cannot be cached");
        probe().store(&bytes);
        assert!(probe().hit(), "known bytes match full on-disk fingerprint");
        let time = fs::metadata(&output)
            .expect("output metadata")
            .modified()
            .expect("output time");
        fs::write(&output, vec![b'z'; bytes.len()]).expect("external edit");
        fs::File::options()
            .write(true)
            .open(&output)
            .expect("output handle")
            .set_times(fs::FileTimes::new().set_modified(time))
            .expect("restore output time");
        // Even an edit between output write and cache persistence must miss.
        probe().store(&bytes);
        assert!(!probe().hit(), "same-stat external edit cannot hit");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn codegen_cache_tracks_optional_inputs_context_and_output_edits() {
        let unique = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "perro_codegen_cache_{}_{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("fixture");
        let source = root.join("clip.panim");
        let optional = root.join("clip.pretarget");
        let output = root.join("animations.rs");
        fs::write(&source, "source".repeat(4096)).expect("source");
        fs::write(&output, "generated").expect("output");
        let probe =
            |context: &str| CodegenCache::new(&output, [source.clone(), optional.clone()], context);
        assert!(!probe("ctx").hit());
        probe("ctx").store(b"generated");
        assert!(probe("ctx").hit());
        assert!(!probe("new ctx").hit());
        let source_time = fs::metadata(&source)
            .expect("source metadata")
            .modified()
            .expect("source time");
        fs::write(&source, "edited".repeat(4096)).expect("same-size source edit");
        fs::File::options()
            .write(true)
            .open(&source)
            .expect("source handle")
            .set_times(fs::FileTimes::new().set_modified(source_time))
            .expect("restore source time");
        assert!(
            !probe("ctx").hit(),
            "source contents invalidate unchanged stats"
        );
        probe("ctx").store(b"generated");
        assert!(probe("ctx").hit());
        fs::write(&optional, "new retarget profile").expect("optional input");
        assert!(!probe("ctx").hit());
        probe("ctx").store(b"generated");
        fs::remove_file(&optional).expect("remove optional");
        assert!(!probe("ctx").hit());
        probe("ctx").store(b"generated");
        let output_time = fs::metadata(&output)
            .expect("output metadata")
            .modified()
            .expect("output time");
        fs::write(&output, "GENERATED").expect("same-size output edit");
        fs::File::options()
            .write(true)
            .open(&output)
            .expect("output handle")
            .set_times(fs::FileTimes::new().set_modified(output_time))
            .expect("restore output time");
        assert!(
            !probe("ctx").hit(),
            "output contents invalidate unchanged stats"
        );
        probe("ctx").store(b"GENERATED");
        assert!(probe("ctx").hit());
        fs::write(&output, "edited generated output").expect("edited output");
        assert!(!probe("ctx").hit());
        probe("ctx").store(b"edited generated output");
        fs::remove_file(&output).expect("remove output");
        assert!(!probe("ctx").hit());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn write_if_changed_preserves_mtime_for_identical_bytes() {
        let dir = std::env::temp_dir().join(format!("perro_wic_{}", std::process::id()));
        fs::create_dir_all(&dir).expect("required value must be present");
        let path = dir.join("out.rs");
        write_if_changed(&path, b"hello").expect("required value must be present");
        let first = fs::metadata(&path)
            .expect("required value must be present")
            .modified()
            .expect("required value must be present");
        write_if_changed(&path, b"hello").expect("required value must be present");
        assert_eq!(
            first,
            fs::metadata(&path)
                .expect("required value must be present")
                .modified()
                .expect("required value must be present")
        );
        write_if_changed(&path, b"hello2").expect("required value must be present");
        assert_eq!(
            fs::read(&path).expect("required value must be present"),
            b"hello2"
        );
        write_if_changed(&path, b"HELLO2").expect("required value must be present");
        assert_eq!(
            fs::read(&path).expect("required value must be present"),
            b"HELLO2"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn source_cache_round_trips_and_prunes_stale_files() {
        let dir = std::env::temp_dir().join(format!("perro_cache_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).expect("required value must be present");
        fs::write(dir.join("kept.bin"), b"kept").expect("required value must be present");
        fs::write(dir.join("sub/stale.bin"), b"stale").expect("required value must be present");

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
        cache.finish().expect("required value must be present");

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
