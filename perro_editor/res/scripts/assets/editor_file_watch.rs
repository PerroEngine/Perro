use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

pub type FileSig = String;

#[derive(Default)]
struct ProjectScanJob {
    running: bool,
    ready: Option<ProjectScan>,
}

pub struct ProjectScan {
    pub before: Vec<FileSig>,
    pub next: Vec<FileSig>,
    pub changed: Vec<String>,
    pub res_paths: Vec<String>,
}

static PROJECT_SCAN_JOBS: OnceLock<Mutex<BTreeMap<String, ProjectScanJob>>> = OnceLock::new();

pub fn request_project_scan(root: PathBuf, before: Vec<FileSig>) {
    let key = root.to_string_lossy().to_string();
    let jobs = PROJECT_SCAN_JOBS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut guard) = jobs.lock() else {
        return;
    };
    let job = guard.entry(key.clone()).or_default();
    if job.running || job.ready.is_some() {
        return;
    }
    job.running = true;
    drop(guard);

    std::thread::spawn(move || {
        let next = scan_project(&root);
        let changed = changed_paths(&before, &next);
        let res_paths = res_paths_from_sigs(&next);
        let scan = ProjectScan {
            before,
            next,
            changed,
            res_paths,
        };
        let jobs = PROJECT_SCAN_JOBS.get_or_init(|| Mutex::new(BTreeMap::new()));
        if let Ok(mut guard) = jobs.lock() {
            let job = guard.entry(key).or_default();
            job.running = false;
            job.ready = Some(scan);
        }
    });
}

pub fn take_project_scan(root: &Path) -> Option<ProjectScan> {
    let key = root.to_string_lossy();
    let jobs = PROJECT_SCAN_JOBS.get_or_init(|| Mutex::new(BTreeMap::new()));
    jobs.lock().ok()?.get_mut(key.as_ref())?.ready.take()
}

pub fn scan_project(root: &Path) -> Vec<FileSig> {
    let mut out = Vec::new();
    scan_watch_path(root, &root.join("res"), &mut out);
    for file in watched_root_files() {
        scan_watch_path(root, &root.join(file), &mut out);
    }
    out.sort();
    out
}

pub fn changed_paths(before: &[FileSig], after: &[FileSig]) -> Vec<String> {
    let before = before
        .iter()
        .filter_map(|sig| sig_path(sig).map(|path| (path.to_string(), sig.clone())))
        .collect::<BTreeMap<_, _>>();
    let after = after
        .iter()
        .filter_map(|sig| sig_path(sig).map(|path| (path.to_string(), sig.clone())))
        .collect::<BTreeMap<_, _>>();

    let mut out = Vec::new();
    for (path, sig) in after.iter() {
        if before.get(path) != Some(sig) {
            out.push(path.clone());
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            out.push(path.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn is_under_res(root: &Path, abs_or_rel: &str) -> bool {
    let rel = normalize_rel(root, abs_or_rel);
    rel == "res" || rel.starts_with("res/")
}

pub fn abs_scene_to_res(root: &Path, abs_or_rel: &str) -> Option<String> {
    let rel = normalize_rel(root, abs_or_rel);
    let rel = rel.strip_prefix("res/")?;
    rel.ends_with(".scn").then(|| format!("res://{rel}"))
}

pub fn res_paths_from_sigs(sigs: &[FileSig]) -> Vec<String> {
    sigs.iter()
        .filter_map(|sig| {
            let rel = sig_path(sig)?;
            if rel == "res" {
                return Some("res://".to_string());
            }
            let path = rel.strip_prefix("res/")?;
            let mut path = format!("res://{path}");
            if sig_is_dir(sig) {
                path.push('/');
            }
            Some(path)
        })
        .collect()
}

fn scan_inner(root: &Path, path: &Path, out: &mut Vec<FileSig>) {
    let Ok(read_dir) = fs::read_dir(path) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
        if name == ".git" || name == "target" {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let rel = rel_path(root, &path);
        let is_dir = meta.is_dir();
        out.push(sig_for_meta(&rel, &meta, is_dir));
        if is_dir {
            scan_inner(root, &path, out);
        }
    }
}

fn scan_watch_path(root: &Path, path: &Path, out: &mut Vec<FileSig>) {
    let rel = rel_path(root, path);
    let Ok(meta) = fs::metadata(path) else {
        out.push(format!("{rel}|missing|0|0"));
        return;
    };
    let is_dir = meta.is_dir();
    out.push(sig_for_meta(&rel, &meta, is_dir));
    if is_dir {
        scan_inner(root, path, out);
    }
}

fn sig_for_meta(rel: &str, meta: &fs::Metadata, is_dir: bool) -> String {
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|v| v.as_nanos())
        .unwrap_or(0);
    format!(
        "{rel}|{}|{modified}|{}",
        meta.len(),
        if is_dir { 1 } else { 0 }
    )
}

fn watched_root_files() -> &'static [&'static str] {
    &[
        "project.toml",
        "input_map.toml",
        "locale.csv",
        "localization.csv",
        "translation.csv",
        "translations.csv",
    ]
}

fn sig_path(sig: &str) -> Option<&str> {
    sig.split('|').next()
}

fn sig_is_dir(sig: &str) -> bool {
    sig.rsplit('|').next() == Some("1")
}

fn normalize_rel(root: &Path, abs_or_rel: &str) -> String {
    let path = PathBuf::from(abs_or_rel);
    if path.is_absolute() {
        rel_path(root, &path)
    } else {
        abs_or_rel.replace('\\', "/")
    }
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_paths_detects_subsecond_same_size_write() {
        let before = vec!["res/live.rs|12|1000000001|0".to_string()];
        let after = vec!["res/live.rs|12|1000000002|0".to_string()];
        assert_eq!(changed_paths(&before, &after), vec!["res/live.rs"]);
    }

    #[test]
    fn res_paths_reuse_scan_output() {
        let sigs = vec![
            "project.toml|12|1|0".to_string(),
            "res|0|1|1".to_string(),
            "res/scenes|0|1|1".to_string(),
            "res/scenes/main.scn|12|1|0".to_string(),
        ];
        assert_eq!(
            res_paths_from_sigs(&sigs),
            vec!["res://", "res://scenes/", "res://scenes/main.scn"]
        );
    }
}
