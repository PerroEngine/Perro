use std::{
    io,
    path::{Path, PathBuf},
};

use perro_io::{ZipEntry, validate_virtual_asset_path};
use perro_resource_api::ResPathSource;

pub fn list<P: ResPathSource>(path: P) -> io::Result<Vec<String>> {
    let path = readable_disk_path(path.as_res_path_str())?;
    perro_io::list_zip_file(path)
}

pub fn read_entry<P: ResPathSource>(path: P, name: &str) -> io::Result<Vec<u8>> {
    let path = readable_disk_path(path.as_res_path_str())?;
    perro_io::read_zip_file_entry(path, name)
}

pub fn extract_all<P: ResPathSource, O: ResPathSource>(path: P, output_dir: O) -> io::Result<()> {
    let path = readable_disk_path(path.as_res_path_str())?;
    let output_dir = writable_disk_path(output_dir.as_res_path_str())?;
    perro_io::extract_zip_file(path, output_dir)
}

pub fn write_files<P: ResPathSource>(path: P, files: &[(&str, &str)]) -> io::Result<()> {
    let path = writable_file_path(path.as_res_path_str())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let entries = files
        .iter()
        .map(|(source, name)| {
            let source = readable_disk_path(source)?;
            Ok(ZipEntry::new(source, *name))
        })
        .collect::<io::Result<Vec<_>>>()?;
    perro_io::write_zip_file(path, &entries)
}

fn readable_disk_path(path: &str) -> io::Result<PathBuf> {
    validate_virtual_asset_path(path)?;
    match perro_io::resolve_path(path) {
        perro_io::ResolvedPath::Disk(path) => Ok(path),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "path does not resolve to disk",
        )),
    }
}

fn writable_disk_path(path: &str) -> io::Result<PathBuf> {
    validate_write_path(path)?;
    match perro_io::resolve_path(path) {
        perro_io::ResolvedPath::Disk(path) => Ok(path),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "path does not resolve to disk",
        )),
    }
}

fn writable_file_path(path: &str) -> io::Result<PathBuf> {
    if path.starts_with("user://") {
        return writable_disk_path(path);
    }
    if Path::new(path).is_absolute() {
        return Ok(PathBuf::from(path));
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "writes are restricted to `user://` or absolute paths",
    ))
}

fn validate_write_path(path: &str) -> io::Result<()> {
    if path.starts_with("user://") {
        validate_virtual_asset_path(path)?;
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
