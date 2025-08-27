use std::fs::{self, File};
use std::io::{self, Write, Read, Seek};
use std::path::PathBuf;
use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::brk::archive::BrkFile;
use crate::brk::{BrkArchive};

/// Trait alias for Read + Seek
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone)]
pub enum ProjectRoot {
    Disk { root: PathBuf, name: String },
    Brk { data: &'static [u8], name: String },
}

static PROJECT_ROOT: Lazy<RwLock<Option<ProjectRoot>>> =
    Lazy::new(|| RwLock::new(None));

static PROJECT_KEY: Lazy<RwLock<Option<[u8; 32]>>> =
    Lazy::new(|| RwLock::new(None));

/// Cached BRK archive (parsed once at startup)
static BRK_ARCHIVE: Lazy<RwLock<Option<BrkArchive>>> =
    Lazy::new(|| RwLock::new(None));

/// Set the project root
pub fn set_project_root(root: ProjectRoot) {
    *PROJECT_ROOT.write().unwrap() = Some(root.clone());

    if let ProjectRoot::Brk { data, .. } = root {
        let archive = BrkArchive::open_from_bytes(data)
            .expect("Failed to open BRK archive");
        *BRK_ARCHIVE.write().unwrap() = Some(archive);
    }
}

/// Set the decryption key
pub fn set_key(key: [u8; 32]) {
    *PROJECT_KEY.write().unwrap() = Some(key);
}

pub fn get_project_root() -> ProjectRoot {
    PROJECT_ROOT
        .read()
        .unwrap()
        .clone()
        .expect("Project root not set")
}

#[derive(Debug, Clone)]
pub enum ResolvedPath {
    Disk(PathBuf),
    Brk(String),
}

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
                let mut pb = root.clone();
                pb.push(path);
                ResolvedPath::Disk(pb)
            }
        }
        ProjectRoot::Brk { data: _, name } => {
            if let Some(stripped) = path.strip_prefix("user://") {
                let base = dirs::data_local_dir()
                    .unwrap_or_else(|| std::env::temp_dir())
                    .join(&name);
                ResolvedPath::Disk(base.join(stripped))
            } else if let Some(stripped) = path.strip_prefix("res://") {
                ResolvedPath::Brk(format!("res/{}", stripped))
            } else {
                ResolvedPath::Brk(path.to_string())
            }
        }
    }
}

/// Load an asset fully into memory (for small files)
pub fn load_asset(path: &str) -> io::Result<Vec<u8>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => fs::read(pb),
        ResolvedPath::Brk(virtual_path) => {
            let key = PROJECT_KEY.read().unwrap();
            if let Some(archive) = BRK_ARCHIVE.write().unwrap().as_mut() {
                archive.read_file(&virtual_path, key.as_ref())
            } else {
                Err(io::Error::new(io::ErrorKind::Other, "BRK archive not loaded"))
            }
        }
    }
}

/// Open an asset for streaming (for large files like audio/video)
pub fn stream_asset(path: &str) -> io::Result<Box<dyn ReadSeek>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            let file = File::open(pb)?;
            Ok(Box::new(file))
        }
        ResolvedPath::Brk(virtual_path) => {
            if let Some(archive) = BRK_ARCHIVE.read().unwrap().as_ref() {
                let file: BrkFile = archive.stream_file(&virtual_path)?;
                Ok(Box::new(file))
            } else {
                Err(io::Error::new(io::ErrorKind::Other, "BRK archive not loaded"))
            }
        }
    }
}

/// Save an asset (only works on disk, not BRK)
pub fn save_asset(path: &str, data: &[u8]) -> io::Result<()> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            if let Some(parent) = pb.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(pb)?;
            file.write_all(data)
        }
        ResolvedPath::Brk(_) => Err(io::Error::new(
            io::ErrorKind::Other,
            "Cannot save into a brk archive (read-only)",
        )),
    }
}