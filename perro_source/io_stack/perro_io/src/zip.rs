use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub struct ZipEntry {
    pub source: PathBuf,
    pub name: String,
    pub unix_permissions: u32,
}

impl ZipEntry {
    pub fn new(source: impl Into<PathBuf>, name: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            name: name.into(),
            unix_permissions: 0o644,
        }
    }

    pub fn executable(mut self) -> Self {
        self.unix_permissions = 0o755;
        self
    }
}

pub fn write_zip_file(path: impl AsRef<Path>, entries: &[ZipEntry]) -> io::Result<()> {
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    for entry in entries {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(entry.unix_permissions);
        zip.start_file(&entry.name, options)
            .map_err(zip_error_to_io)?;
        let mut input = fs::File::open(&entry.source)?;
        io::copy(&mut input, &mut zip)?;
    }
    zip.finish().map_err(zip_error_to_io)?;
    Ok(())
}

pub fn list_zip_file(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_io)?;
    let mut out = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(zip_error_to_io)?;
        out.push(file.name().to_string());
    }
    out.sort();
    Ok(out)
}

pub fn read_zip_file_entry(path: impl AsRef<Path>, name: &str) -> io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_io)?;
    let mut entry = archive.by_name(name).map_err(zip_error_to_io)?;
    let mut out = Vec::new();
    entry.read_to_end(&mut out)?;
    Ok(out)
}

pub fn extract_zip_file(path: impl AsRef<Path>, output_dir: impl AsRef<Path>) -> io::Result<()> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;
    let output_dir = output_dir.canonicalize()?;
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_io)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error_to_io)?;
        let Some(enclosed_name) = entry.enclosed_name() else {
            continue;
        };
        let target = output_dir.join(enclosed_name);
        ensure_inside_dir(&output_dir, &target)?;
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&target)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

fn ensure_inside_dir(root: &Path, target: &Path) -> io::Result<()> {
    let parent = target.parent().unwrap_or(root);
    fs::create_dir_all(parent)?;
    let parent = parent.canonicalize()?;
    if parent.starts_with(root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "zip entry escapes output directory",
        ))
    }
}

fn zip_error_to_io(err: zip::result::ZipError) -> io::Error {
    io::Error::other(err)
}
