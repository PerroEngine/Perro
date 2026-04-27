use std::{
    cell::RefCell,
    collections::HashMap,
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, RwLock},
};

use crate::data_local_dir;
use perro_assets::archive::{PerroAssetsArchive, PerroAssetsFile};

/// Trait alias for Read + Seek
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone)]
pub enum ProjectRoot {
    Disk { root: PathBuf, name: String },
    PerroAssets { data: &'static [u8], name: String },
}

static PROJECT_ROOT: RwLock<Option<ProjectRoot>> = RwLock::new(None);
static PERRO_ASSETS_ARCHIVE: RwLock<Option<PerroAssetsArchive>> = RwLock::new(None);
static DLC_MOUNTS: LazyLock<RwLock<HashMap<String, DlcMount>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static DLC_ARCHIVES: LazyLock<RwLock<HashMap<String, Arc<PerroAssetsArchive>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

thread_local! {
    static DLC_SELF_CONTEXT: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[derive(Clone, Debug)]
pub enum DlcMountSource {
    Disk(PathBuf),
    Archive(PathBuf),
}

#[derive(Clone, Debug)]
pub struct DlcMount {
    pub name: String,
    pub source: DlcMountSource,
}

pub fn is_reserved_dlc_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("self")
}

pub fn get_project_root() -> ProjectRoot {
    PROJECT_ROOT
        .read()
        .unwrap()
        .clone()
        .expect("Project root not set")
}

pub fn set_project_root(root: ProjectRoot) {
    *PROJECT_ROOT.write().unwrap() = Some(root.clone());

    if let ProjectRoot::PerroAssets { data, .. } = root {
        let archive =
            PerroAssetsArchive::open_from_bytes(data).expect("Failed to open PerroAssets archive");
        *PERRO_ASSETS_ARCHIVE.write().unwrap() = Some(archive);
    }
}

pub fn clear_dlc_mounts() {
    DLC_MOUNTS.write().unwrap().clear();
    DLC_ARCHIVES.write().unwrap().clear();
}

pub fn mounted_dlc_names() -> Vec<String> {
    let mut out = DLC_MOUNTS
        .read()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    out.sort();
    out
}

pub fn read_mounted_dlc_file(name: &str, virtual_path: &str) -> io::Result<Vec<u8>> {
    let key = name.to_ascii_lowercase();
    if let Some(archive) = DLC_ARCHIVES.read().unwrap().get(&key) {
        archive.read_file(virtual_path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc archive mount not found: {name}"),
        ))
    }
}

pub fn mount_dlc_disk(name: &str, root: impl AsRef<Path>) -> io::Result<()> {
    if is_reserved_dlc_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "dlc name `self` is reserved",
        ));
    }
    let root = root.as_ref().to_path_buf();
    if !root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dlc disk root not found: {}", root.display()),
        ));
    }
    DLC_MOUNTS.write().unwrap().insert(
        name.to_ascii_lowercase(),
        DlcMount {
            name: name.to_string(),
            source: DlcMountSource::Disk(root),
        },
    );
    Ok(())
}

pub fn mount_dlc_archive(name: &str, archive_path: impl AsRef<Path>) -> io::Result<()> {
    if is_reserved_dlc_name(name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "dlc name `self` is reserved",
        ));
    }
    let archive_path = archive_path.as_ref().to_path_buf();
    let archive = Arc::new(PerroAssetsArchive::open_from_file(&archive_path)?);
    let key = name.to_ascii_lowercase();
    DLC_ARCHIVES.write().unwrap().insert(key.clone(), archive);
    DLC_MOUNTS.write().unwrap().insert(
        key,
        DlcMount {
            name: name.to_string(),
            source: DlcMountSource::Archive(archive_path),
        },
    );
    Ok(())
}

pub fn set_dlc_self_context(name: Option<&str>) {
    DLC_SELF_CONTEXT.with(|ctx| {
        *ctx.borrow_mut() = name.map(|v| v.to_ascii_lowercase());
    });
}

#[derive(Debug, Clone)]
pub enum ResolvedPath {
    Disk(PathBuf),
    PerroAssets(String),
    DlcPerroAssets {
        dlc: String,
        virtual_path: String,
    },
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
                ProjectRoot::PerroAssets { name, .. } => name.as_str(),
            })
            .expect("Project root not set");

        let base = data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(app_name)
            .join("data");
        return ResolvedPath::Disk(base.join(stripped));
    }

    if let Some(rest) = path.strip_prefix("dlc://") {
        let (mount_raw, rel_raw) = rest.split_once('/').unwrap_or((rest, ""));
        let rel = rel_raw.trim_start_matches('/');
        let mount_name = if mount_raw.eq_ignore_ascii_case("self") {
            DLC_SELF_CONTEXT.with(|ctx| ctx.borrow().clone())
        } else {
            Some(mount_raw.to_ascii_lowercase())
        };
        if let Some(name) = mount_name
            && let Some(mount) = DLC_MOUNTS.read().unwrap().get(&name)
        {
            return match &mount.source {
                DlcMountSource::Disk(root) => ResolvedPath::Disk(root.join(rel)),
                DlcMountSource::Archive(_) => ResolvedPath::DlcPerroAssets {
                    dlc: name,
                    virtual_path: format!("res/{rel}"),
                },
            };
        }
        return ResolvedPath::Disk(PathBuf::from(path));
    }

    // Handle absolute filesystem paths
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        return ResolvedPath::Disk(path_buf);
    }

    match project_root_opt {
        Some(ProjectRoot::Disk { root, .. }) => {
            if let Some(stripped) = path.strip_prefix("res://") {
                let primary = root.join("res").join(stripped);
                if primary.exists() {
                    ResolvedPath::Disk(primary)
                } else {
                    // Fallback: if root already points at a res directory, avoid res/res.
                    ResolvedPath::Disk(root.join(stripped))
                }
            } else {
                ResolvedPath::Disk(root.join(path))
            }
        }
        Some(ProjectRoot::PerroAssets { .. }) => {
            if let Some(stripped) = path.strip_prefix("res://") {
                ResolvedPath::PerroAssets(format!("res/{}", stripped))
            } else {
                ResolvedPath::PerroAssets(path.to_string())
            }
        }
        None => ResolvedPath::Disk(PathBuf::from(path)),
    }
}

/// Load an asset fully into memory
pub fn load_asset(path: &str) -> io::Result<Vec<u8>> {
    match resolve_path(path) {
        ResolvedPath::Disk(pb) => fs::read(pb),
        ResolvedPath::PerroAssets(virtual_path) => {
            if let Some(archive) = PERRO_ASSETS_ARCHIVE.read().unwrap().as_ref() {
                archive.read_file(&virtual_path)
            } else {
                Err(io::Error::other("PerroAssets archive not loaded"))
            }
        }
        ResolvedPath::DlcPerroAssets { dlc, virtual_path } => {
            if let Some(archive) = DLC_ARCHIVES.read().unwrap().get(&dlc) {
                archive.read_file(&virtual_path)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("dlc archive mount not found: {dlc}"),
                ))
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
        ResolvedPath::PerroAssets(virtual_path) => {
            if let Some(archive) = PERRO_ASSETS_ARCHIVE.read().unwrap().as_ref() {
                let file: PerroAssetsFile = archive.stream_file(&virtual_path)?;
                Ok(Box::new(file))
            } else {
                Err(io::Error::other("PerroAssets archive not loaded"))
            }
        }
        ResolvedPath::DlcPerroAssets { dlc, virtual_path } => {
            if let Some(archive) = DLC_ARCHIVES.read().unwrap().get(&dlc) {
                let file: PerroAssetsFile = archive.stream_file(&virtual_path)?;
                Ok(Box::new(file))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("dlc archive mount not found: {dlc}"),
                ))
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
        ResolvedPath::PerroAssets(_) | ResolvedPath::DlcPerroAssets { .. } => {
            Err(io::Error::other("Cannot save to packed archive"))
        }
    }
}
