use std::{
    io,
    path::{Path, PathBuf},
};

use perro_io::{ProjectRoot, load_asset, save_asset, set_project_root};

pub fn set_project_root_disk(root: &str, name: &str) {
    set_project_root(ProjectRoot::Disk {
        root: PathBuf::from(root),
        name: name.to_string(),
    });
}

pub fn load_bytes(path: &str) -> io::Result<Vec<u8>> {
    load_asset(path)
}

pub fn load_string(path: &str) -> io::Result<String> {
    let bytes = load_bytes(path)?;
    String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

pub fn save_bytes(path: &str, data: &[u8]) -> io::Result<()> {
    validate_write_path(path)?;
    save_asset(path, data)
}

pub fn save_string(path: &str, data: &str) -> io::Result<()> {
    save_bytes(path, data.as_bytes())
}

pub fn exists(path: &str) -> bool {
    match perro_io::resolve_path(path) {
        perro_io::ResolvedPath::Disk(pb) => pb.exists(),
        perro_io::ResolvedPath::Brk(_) => load_asset(path).is_ok(),
    }
}

pub fn resolve_path_string(path: &str) -> String {
    match perro_io::resolve_path(path) {
        perro_io::ResolvedPath::Disk(pb) => pb.to_string_lossy().to_string(),
        perro_io::ResolvedPath::Brk(vpath) => format!("brk://{vpath}"),
    }
}

fn validate_write_path(path: &str) -> io::Result<()> {
    if path.starts_with("user://") {
        return Ok(());
    }

    if Path::new(path).is_absolute() {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "writes are restricted to `user://` or absolute paths",
    ))
}
