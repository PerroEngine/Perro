use std::{
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::PathBuf,
    sync::RwLock,
};

use crate::{brk::archive::{BrkArchive, BrkFile}, data_local_dir};

/// Trait alias for Read + Seek
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone)]
pub enum ProjectRoot {
    Disk { root: PathBuf, name: String },
    Brk { data: &'static [u8], name: String },
}

static PROJECT_ROOT: RwLock<Option<ProjectRoot>> = RwLock::new(None);
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

#[derive(Debug, Clone)]
pub enum ResolvedPath {
    Disk(PathBuf),
    Brk(String),
}

/// Resolve virtual path (res://foo/bar.png or user://save.dat) to actual location
pub fn resolve_path(path: &str) -> ResolvedPath {
    let project_root_opt = PROJECT_ROOT.read().unwrap().clone();

    // Handle user:// paths (always disk)
    if let Some(stripped) = path.strip_prefix("user://") {
        let app_name = project_root_opt
            .as_ref()
            .map(|root| match root {
                ProjectRoot::Disk { name, .. } => name.as_str(),
                ProjectRoot::Brk { name, .. } => name.as_str(),
            })
            .expect("Project root not set");
        
        let base = data_local_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join(app_name);
        return ResolvedPath::Disk(base.join(stripped));
    }

    // Handle absolute filesystem paths
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        return ResolvedPath::Disk(path_buf);
    }

    match project_root_opt {
        Some(ProjectRoot::Disk { root, .. }) => {
            if let Some(stripped) = path.strip_prefix("res://") {
                ResolvedPath::Disk(root.join("res").join(stripped))
            } else {
                ResolvedPath::Disk(root.join(path))
            }
        }
        Some(ProjectRoot::Brk { .. }) => {
            if let Some(stripped) = path.strip_prefix("res://") {
                ResolvedPath::Brk(format!("res/{}", stripped))
            } else {
                ResolvedPath::Brk(path.to_string())
            }
        }
        None => ResolvedPath::Disk(PathBuf::from(path)),
    }
}

/// Load an asset fully into memory
pub fn load_asset(path: &str) -> io::Result<Vec<u8>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => fs::read(pb),
        ResolvedPath::Brk(virtual_path) => {
            if let Some(archive) = BRK_ARCHIVE.read().unwrap().as_ref() {
                archive.read_file(&virtual_path)
            } else {
                Err(io::Error::new(io::ErrorKind::Other, "BRK archive not loaded"))
            }
        }
    }
}

/// Stream an asset (for large files)
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

/// Save an asset (disk only)
pub fn save_asset(path: &str, data: &[u8]) -> io::Result<()> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => {
            if let Some(parent) = pb.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = File::create(pb)?;
            file.write_all(data)
        }
        ResolvedPath::Brk(_) => {
            Err(io::Error::new(io::ErrorKind::Other, "Cannot save to BRK archive"))
        }
    }
}