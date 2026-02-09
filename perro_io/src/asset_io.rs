use std::{
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::PathBuf,
    sync::RwLock,
};

use crate::brk::archive::{BrkArchive, BrkFile};

/// Trait alias for Read + Seek
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone)]
pub enum ProjectRoot {
    Disk { root: PathBuf, name: String },
    Brk { data: &'static [u8], name: String },
}

static PROJECT_ROOT: RwLock<Option<ProjectRoot>> = RwLock::new(None);

static PROJECT_KEY: RwLock<Option<[u8; 32]>> = RwLock::new(None);

static BRK_ARCHIVE: RwLock<Option<BrkArchive>> = RwLock::new(None);

pub fn get_project_root() -> ProjectRoot {
    PROJECT_ROOT
        .read()
        .unwrap()
        .clone()
        .expect("Project root not set")
}

pub fn set_project_root(root: ProjectRoot) {
    *PROJECT_ROOT.write().unwrap() = Some(root.clone());

    if let ProjectRoot::Brk { data, .. } = root {
        let archive = BrkArchive::open_from_bytes(data).expect("Failed to open BRK archive");
        *BRK_ARCHIVE.write().unwrap() = Some(archive);
    }
}

pub fn set_key(key: [u8; 32]) {
    *PROJECT_KEY.write().unwrap() = Some(key);
}

#[derive(Debug, Clone)]
pub enum ResolvedPath {
    Disk(PathBuf),
    Brk(String),
}

/// Take a virtual path (res://foo/bar.png or user://save.dat) and resolve it to either a disk path or a BRK virtual path, depending on the project root configuration.
pub fn resolve_path(path: &str) -> ResolvedPath {
    let project_root_opt = PROJECT_ROOT.read().unwrap().clone();

    // 1. Handle user:// paths first, as they always map to disk.
    if let Some(stripped) = path.strip_prefix("user://") {
        let app_name = project_root_opt
            .as_ref()
            .map(|root| match root {
                ProjectRoot::Disk { name, .. } => name.as_str(),
                ProjectRoot::Brk { name, .. } => name.as_str(),
            })
            .expect("Project root not set");
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join(app_name);
        return ResolvedPath::Disk(base.join(stripped));
    }

    // 2. Handle explicit absolute filesystem paths (like C:\Users\...)
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        return ResolvedPath::Disk(path_buf);
    }

    match project_root_opt {
        Some(ProjectRoot::Disk { root, name: _ }) => {
            // When the project is disk-based (e.g., game in dev mode)
            if let Some(stripped) = path.strip_prefix("res://") {
                let mut pb = root.clone();
                pb.push("res");
                pb.push(stripped);
                return ResolvedPath::Disk(pb);
            } else {
                let mut pb = root.clone();
                pb.push(path);
                return ResolvedPath::Disk(pb);
            }
        }
        Some(ProjectRoot::Brk { data: _, name: _ }) => {
            // When the project is BRK-based (e.g., game in production)
            if let Some(stripped) = path.strip_prefix("res://") {
                ResolvedPath::Brk(format!("res/{}", stripped));
            } else {
                // This branch now correctly means "this path is relative and should be in the BRK"
                ResolvedPath::Brk(path.to_string());
            }
        }
        None => {
            // No project root set, treat as disk path
            ResolvedPath::Disk(PathBuf::from(path));
        }
    }

    ResolvedPath::Brk(path.to_string())
}

/// Load an asset fully into memory
pub fn load_asset(path: &str) -> io::Result<Vec<u8>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => fs::read(pb),
        ResolvedPath::Brk(virtual_path) => {
            let key = PROJECT_KEY.read().unwrap();
            if let Some(archive) = BRK_ARCHIVE.write().unwrap().as_mut() {
                archive.read_file(&virtual_path, key.as_ref())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "BRK archive not loaded",
                ))
            }
        }
    }
}

/// Get a reader for an asset, which can be used to stream large files without loading them fully into memory.
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
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "BRK archive not loaded",
                ))
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
