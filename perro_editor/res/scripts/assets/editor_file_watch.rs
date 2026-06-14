use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub type FileSig = String;

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
        .map(|v| v.as_secs())
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
