use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::RwLock;
use once_cell::sync::Lazy;
use zip::ZipArchive;

/// Where the project root lives
#[derive(Clone)]
pub enum ProjectRoot {
    Disk { root: PathBuf, name: String },
    Pak { data: &'static [u8], name: String },
}

/// Global project root
static PROJECT_ROOT: Lazy<RwLock<Option<ProjectRoot>>> =
    Lazy::new(|| RwLock::new(None));

pub fn set_project_root(root: ProjectRoot) {
    *PROJECT_ROOT.write().unwrap() = Some(root);
}

pub fn get_project_root() -> ProjectRoot {
    PROJECT_ROOT
        .read()
        .unwrap()
        .clone()
        .expect("Project root not set")
}

/// A resolved path can either be a real filesystem path or a virtual path inside a pak
#[derive(Debug, Clone)]
pub enum ResolvedPath {
    Disk(PathBuf),
    Pak(String),
}

/// Resolve a `res://` or `user://` path into either a disk path or a pak-relative path
pub fn resolve_path(path: &str) -> ResolvedPath {
    match get_project_root() {
        ProjectRoot::Disk { root, name } => {
            if let Some(stripped) = path.strip_prefix("user://") {
                let base = dirs::data_local_dir()
                    .unwrap_or_else(|| std::env::temp_dir())
                    .join(&name);
                ResolvedPath::Disk(base.join(stripped))
            } else if let Some(stripped) = path.strip_prefix("res://") {
                let mut pb = root.clone();
                pb.push("res");
                pb.push(stripped);
                ResolvedPath::Disk(pb)
            } else {
                // ✅ Default: resolve relative to project root
                let mut pb = root.clone();
                pb.push(path);
                ResolvedPath::Disk(pb)
            }
        }
        ProjectRoot::Pak { data: _, name } => {
            if let Some(stripped) = path.strip_prefix("user://") {
                let base = dirs::data_local_dir()
                    .unwrap_or_else(|| std::env::temp_dir())
                    .join(&name);
                ResolvedPath::Disk(base.join(stripped))
            } else if let Some(stripped) = path.strip_prefix("res://") {
                ResolvedPath::Pak(format!("res/{}", stripped))
            } else {
                // ✅ Default: treat as root-level file in pak
                ResolvedPath::Pak(path.to_string())
            }
        }
    }
}

/// Load an asset into memory
pub fn load_asset(path: &str) -> io::Result<Vec<u8>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => fs::read(pb),
        ResolvedPath::Pak(virtual_path) => {
            if let ProjectRoot::Pak { data, .. } = get_project_root() {
                let cursor = std::io::Cursor::new(data);
                let mut archive = ZipArchive::new(cursor)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                for i in 0..archive.len() {
    let file = archive.by_index(i).unwrap();
    eprintln!("[pak] contains: {}", file.name());
}
                let mut file = archive
                    .by_name(&virtual_path)
                    .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;

                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                Ok(buf)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Tried to load from pak, but project root is not pak",
                ))
            }
        }
    }
}

/// Save an asset (only works for disk + user://)
pub fn save_asset(path: &str, data: &[u8]) -> io::Result<()> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            if let Some(parent) = pb.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(pb)?;
            file.write_all(data)
        }
        ResolvedPath::Pak(_) => Err(io::Error::new(
            io::ErrorKind::Other,
            "Cannot save into a pak archive (read-only)",
        )),
    }
}