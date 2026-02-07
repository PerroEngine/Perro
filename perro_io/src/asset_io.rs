use std::{path::PathBuf, sync::RwLock};

use crate::brk::archive::BrkArchive;

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

    // 3. If not an absolute path, assume it's a BRK path
    ResolvedPath::Brk(path.to_string())
}
