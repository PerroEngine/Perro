use std::{
    io,
    path::{Path, PathBuf},
};

use perro_io::{ProjectRoot, load_asset, save_asset, set_project_root};
use perro_resource_api::ResPathSource;

pub fn set_project_root_disk(root: &str, name: &str) {
    set_project_root(ProjectRoot::Disk {
        root: PathBuf::from(root),
        name: name.to_string(),
    });
}

pub fn load_bytes<P: ResPathSource>(path: P) -> io::Result<Vec<u8>> {
    load_asset(path.as_res_path_str())
}

pub fn load_string<P: ResPathSource>(path: P) -> io::Result<String> {
    let bytes = load_bytes(path)?;
    String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

pub fn save_bytes<P: ResPathSource>(path: P, data: &[u8]) -> io::Result<()> {
    let path = path.as_res_path_str();
    validate_write_path(path)?;
    save_asset(path, data)
}

pub fn save_string<P: ResPathSource>(path: P, data: &str) -> io::Result<()> {
    save_bytes(path, data.as_bytes())
}

pub fn exists<P: ResPathSource>(path: P) -> bool {
    let path = path.as_res_path_str();
    match perro_io::resolve_path(path) {
        perro_io::ResolvedPath::Disk(pb) => pb.exists(),
        perro_io::ResolvedPath::WebUserStorage(_)
        | perro_io::ResolvedPath::PerroAssets(_)
        | perro_io::ResolvedPath::StaticBinary(_)
        | perro_io::ResolvedPath::DlcStaticBinary { .. }
        | perro_io::ResolvedPath::DlcPerroAssets { .. } => load_asset(path).is_ok(),
    }
}

pub fn resolve_path_string<P: ResPathSource>(path: P) -> String {
    let path = path.as_res_path_str();
    match perro_io::resolve_path(path) {
        perro_io::ResolvedPath::Disk(pb) => pb.to_string_lossy().to_string(),
        perro_io::ResolvedPath::WebUserStorage(key) => format!("webstorage://{key}"),
        perro_io::ResolvedPath::PerroAssets(vpath) => format!("perroassets://{vpath}"),
        perro_io::ResolvedPath::StaticBinary(path) => format!("staticbinary://{path}"),
        perro_io::ResolvedPath::DlcStaticBinary { dlc, path } => {
            format!("staticbinary+dlc://{dlc}/{path}")
        }
        perro_io::ResolvedPath::DlcPerroAssets { dlc, virtual_path } => {
            format!("perroassets+dlc://{dlc}/{virtual_path}")
        }
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
