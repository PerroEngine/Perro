// Stat-sidecar cache for the `res/` -> `.perro/scripts/src` codegen pass.
//
// `sync_scripts` re-parses every `.scn` and `.panim` in the project and
// re-transpiles every script on each `perro dev`, even when nothing changed:
// ~400ms of a ~1.7s pre-launch budget on a mid-size project.
//
// # Why this cannot go stale on an engine change
//
// The generated files are a function of three things: the input bytes, the
// project config, and *the transpiler code itself*. Keying on inputs alone
// would serve stale output after an engine edit, so the cache context also
// carries a fingerprint of the running executable.
//
// `perro_compiler` is statically linked into that executable, so any engine
// change that could alter codegen necessarily produces a different binary ->
// different fingerprint -> full re-sync. The converse is what makes it safe
// rather than merely conservative: if the binary did *not* change, the code
// that produced the cached output is byte-identical to the code running now,
// so reusing it is correct by construction.
//
// Everything else fails closed: an unreadable sidecar, an unreadable exe, a
// version bump, a missing output, or any I/O error is a miss, not a hit.

use std::time::UNIX_EPOCH;

/// Bump to invalidate every sidecar in the wild.
const SYNC_CACHE_VERSION: u32 = 1;
const SYNC_CACHE_FILE: &str = ".sync_cache";

/// Stat key for one file: length plus mtime in nanos since the epoch.
type StatKey = (u64, u128);

pub(crate) struct SyncCache {
    path: PathBuf,
    context: String,
    inputs: BTreeMap<String, StatKey>,
    copied: Vec<String>,
}

impl SyncCache {
    /// Collects the current input stats and the cache context.
    ///
    /// Returns `None` when the context cannot be established (no readable
    /// executable), which disables caching rather than risking a stale hit.
    pub fn probe(project_root: &Path, demo: bool) -> Option<Self> {
        let context = cache_context(demo)?;
        let inputs = collect_input_stats(project_root);
        Some(Self {
            path: project_root
                .join(".perro")
                .join("scripts")
                .join(SYNC_CACHE_FILE),
            context,
            inputs,
            copied: Vec::new(),
        })
    }

    /// Returns the previously synced script list when nothing relevant changed.
    ///
    /// Requires an exact match on the context and on the whole input set --
    /// added and removed files both count -- and that every generated file the
    /// last run produced is still on disk.
    pub fn hit(&self, scripts_src: &Path) -> Option<Vec<String>> {
        let (context, inputs, copied) = read_sidecar(&self.path)?;
        if context != self.context || inputs != self.inputs {
            return None;
        }
        if !scripts_src.join("lib.rs").is_file() {
            return None;
        }
        for rel in &copied {
            if !scripts_src.join(generated_script_rel(rel)).is_file() {
                return None;
            }
        }
        Some(copied)
    }

    /// Records a completed sync. Best-effort: a write failure only costs the
    /// next run a rebuild.
    pub fn store(mut self, copied: &[String]) {
        self.copied = copied.to_vec();
        let _ = write_sidecar(&self.path, &self.context, &self.inputs, &self.copied);
    }
}

/// Version + demo flag + a fingerprint of the binary doing the codegen.
fn cache_context(demo: bool) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let (len, mtime) = stat_key(&exe)?;
    Some(format!(
        "v{SYNC_CACHE_VERSION}\tdemo={demo}\texe={len}:{mtime}"
    ))
}

fn stat_key(path: &Path) -> Option<StatKey> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), mtime))
}

/// Every file that can influence codegen.
///
/// Walks `res/` and `dlcs/` wholesale rather than filtering by extension: the
/// set of relevant extensions is a detail of the transpiler, and over-
/// invalidating costs a rebuild while under-invalidating ships stale code.
fn collect_input_stats(project_root: &Path) -> BTreeMap<String, StatKey> {
    let mut out = BTreeMap::new();
    for dir in ["res", "dlcs"] {
        let root = project_root.join(dir);
        collect_dir_stats(&root, &root, dir, &mut out);
    }
    for file in ["project.toml", "deps.toml"] {
        if let Some(key) = stat_key(&project_root.join(file)) {
            out.insert(file.to_string(), key);
        }
    }
    out
}

fn collect_dir_stats(
    root: &Path,
    dir: &Path,
    prefix: &str,
    out: &mut BTreeMap<String, StatKey>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_dir_stats(root, &path, prefix, out);
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let Some(key) = stat_key(&path) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        out.insert(format!("{prefix}/{rel}"), key);
    }
}

type Sidecar = (String, BTreeMap<String, StatKey>, Vec<String>);

fn read_sidecar(path: &Path) -> Option<Sidecar> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let context = lines.next()?.to_string();
    let mut inputs = BTreeMap::new();
    let mut copied = Vec::new();
    for line in lines {
        let mut parts = line.split('\t');
        match parts.next()? {
            "I" => {
                let rel = parts.next()?.to_string();
                let len = parts.next()?.parse().ok()?;
                let mtime = parts.next()?.parse().ok()?;
                inputs.insert(rel, (len, mtime));
            }
            "C" => copied.push(parts.next()?.to_string()),
            _ => return None,
        }
    }
    Some((context, inputs, copied))
}

fn write_sidecar(
    path: &Path,
    context: &str,
    inputs: &BTreeMap<String, StatKey>,
    copied: &[String],
) -> std::io::Result<()> {
    let mut out = String::with_capacity(inputs.len() * 48);
    out.push_str(context);
    out.push('\n');
    for (rel, (len, mtime)) in inputs {
        out.push_str(&format!("I\t{rel}\t{len}\t{mtime}\n"));
    }
    for rel in copied {
        out.push_str(&format!("C\t{rel}\n"));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, out)
}

#[cfg(test)]
mod sync_cache_tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("perro_sync_cache_{tag}_{stamp}"));
        fs::create_dir_all(dir.join("res")).expect("res dir");
        dir
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, body).expect("write");
    }

    /// Stand-in for a completed sync: the generated files the cache verifies.
    fn fake_outputs(root: &Path, copied: &[String]) -> PathBuf {
        let scripts_src = root.join(".perro").join("scripts").join("src");
        write(&scripts_src.join("lib.rs"), "// generated");
        for rel in copied {
            write(&scripts_src.join(generated_script_rel(rel)), "// generated");
        }
        scripts_src
    }

    #[test]
    fn unchanged_inputs_hit() {
        let root = temp_dir("hit");
        write(&root.join("res/scripts/a.rs"), "fn a() {}");
        let copied = vec!["scripts/a.rs".to_string()];
        let scripts_src = fake_outputs(&root, &copied);

        SyncCache::probe(&root, false).expect("probe").store(&copied);
        let hit = SyncCache::probe(&root, false).expect("probe").hit(&scripts_src);

        assert_eq!(hit, Some(copied));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn edited_added_and_removed_inputs_all_miss() {
        let root = temp_dir("miss");
        let script = root.join("res/scripts/a.rs");
        write(&script, "fn a() {}");
        let copied = vec!["scripts/a.rs".to_string()];
        let scripts_src = fake_outputs(&root, &copied);
        SyncCache::probe(&root, false).expect("probe").store(&copied);

        // Edited: same path, different length.
        write(&script, "fn a() { let _ = 1; }");
        assert_eq!(
            SyncCache::probe(&root, false).expect("probe").hit(&scripts_src),
            None
        );
        SyncCache::probe(&root, false).expect("probe").store(&copied);

        // Added: a new scene the index would have to read.
        let added = root.join("res/scenes/main.scn");
        write(&added, "[main]\n");
        assert_eq!(
            SyncCache::probe(&root, false).expect("probe").hit(&scripts_src),
            None
        );
        SyncCache::probe(&root, false).expect("probe").store(&copied);

        // Removed.
        fs::remove_file(&added).expect("remove");
        assert_eq!(
            SyncCache::probe(&root, false).expect("probe").hit(&scripts_src),
            None
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn demo_flag_and_tool_fingerprint_are_part_of_the_key() {
        let root = temp_dir("context");
        write(&root.join("res/scripts/a.rs"), "fn a() {}");
        let copied = vec!["scripts/a.rs".to_string()];
        let scripts_src = fake_outputs(&root, &copied);
        SyncCache::probe(&root, false).expect("probe").store(&copied);

        // Same inputs, different demo flag -> different context -> miss.
        assert_eq!(
            SyncCache::probe(&root, true).expect("probe").hit(&scripts_src),
            None
        );

        // A rebuilt engine binary changes the exe fingerprint; simulate by
        // rewriting the recorded context.
        let path = root.join(".perro/scripts").join(SYNC_CACHE_FILE);
        let text = fs::read_to_string(&path).expect("sidecar");
        let mut lines = text.lines();
        let stale = format!("{}-stale\n{}", lines.next().expect("context"), lines.collect::<Vec<_>>().join("\n"));
        fs::write(&path, stale).expect("rewrite");
        assert_eq!(
            SyncCache::probe(&root, false).expect("probe").hit(&scripts_src),
            None
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn missing_generated_output_misses() {
        let root = temp_dir("outputs");
        write(&root.join("res/scripts/a.rs"), "fn a() {}");
        let copied = vec!["scripts/a.rs".to_string()];
        let scripts_src = fake_outputs(&root, &copied);
        SyncCache::probe(&root, false).expect("probe").store(&copied);

        fs::remove_file(scripts_src.join(generated_script_rel("scripts/a.rs")))
            .expect("remove generated");
        assert_eq!(
            SyncCache::probe(&root, false).expect("probe").hit(&scripts_src),
            None
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
