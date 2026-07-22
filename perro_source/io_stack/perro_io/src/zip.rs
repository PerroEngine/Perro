use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

const DEFAULT_MAX_ENTRIES: usize = 10_000;
const DEFAULT_MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZipLimits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_compression_ratio: u64,
}

impl Default for ZipLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            max_entry_bytes: DEFAULT_MAX_ENTRY_BYTES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_compression_ratio: DEFAULT_MAX_COMPRESSION_RATIO,
        }
    }
}

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
    list_zip_file_with_limits(path, ZipLimits::default())
}

pub fn list_zip_file_with_limits(
    path: impl AsRef<Path>,
    limits: ZipLimits,
) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_io)?;
    check_entry_count(archive.len(), limits)?;
    let mut out = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(zip_error_to_io)?;
        out.push(file.name().to_string());
    }
    out.sort();
    Ok(out)
}

pub fn read_zip_file_entry(path: impl AsRef<Path>, name: &str) -> io::Result<Vec<u8>> {
    read_zip_file_entry_with_limits(path, name, ZipLimits::default())
}

pub fn read_zip_file_entry_with_limits(
    path: impl AsRef<Path>,
    name: &str,
    limits: ZipLimits,
) -> io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_io)?;
    check_entry_count(archive.len(), limits)?;
    let mut entry = archive.by_name(name).map_err(zip_error_to_io)?;
    check_entry_metadata(&entry, limits)?;
    let byte_limit = limits.max_entry_bytes.min(limits.max_total_bytes);
    let capacity = usize::try_from(entry.size().min(byte_limit)).unwrap_or(usize::MAX);
    let mut out = Vec::new();
    out.try_reserve(capacity)
        .map_err(|err| io::Error::other(format!("zip entry allocation failed: {err}")))?;
    read_limited(&mut entry, &mut out, byte_limit)?;
    Ok(out)
}

pub fn extract_zip_file(path: impl AsRef<Path>, output_dir: impl AsRef<Path>) -> io::Result<()> {
    extract_zip_file_with_limits(path, output_dir, ZipLimits::default())
}

/// Extract an archive with explicit resource limits.
///
/// Existing symlink/reparse components and output targets are rejected. Output
/// files use create-new semantics. Platforms without directory-relative
/// no-follow opens retain an ancestor replacement race, so the output tree must
/// not be writable by an untrusted concurrent process.
pub fn extract_zip_file_with_limits(
    path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    limits: ZipLimits,
) -> io::Result<()> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;
    let output_dir = output_dir.canonicalize()?;
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error_to_io)?;
    check_entry_count(archive.len(), limits)?;

    let mut total_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(zip_error_to_io)?;
        check_entry_metadata(&entry, limits)?;
        let enclosed_name = entry.enclosed_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "zip entry path escapes output directory",
            )
        })?;
        validate_relative_path(&enclosed_name)?;

        if entry.is_dir() {
            ensure_secure_dirs(&output_dir, &enclosed_name)?;
            continue;
        }

        let parent = enclosed_name.parent().unwrap_or_else(|| Path::new(""));
        ensure_secure_dirs(&output_dir, parent)?;
        let target = output_dir.join(&enclosed_name);
        reject_existing_target(&target)?;

        let remaining_total = limits.max_total_bytes.saturating_sub(total_bytes);
        if entry.size() > remaining_total {
            return Err(limit_error("zip total size limit exceeded"));
        }
        let byte_limit = limits.max_entry_bytes.min(remaining_total);
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let result = read_limited(&mut entry, &mut out, byte_limit).and_then(|written| {
            out.flush()?;
            Ok(written)
        });
        match result {
            Ok(written) => total_bytes = total_bytes.saturating_add(written),
            Err(err) => {
                drop(out);
                let _ = fs::remove_file(&target);
                return Err(err);
            }
        }
    }
    Ok(())
}

fn check_entry_count(count: usize, limits: ZipLimits) -> io::Result<()> {
    if count > limits.max_entries {
        Err(limit_error("zip entry count limit exceeded"))
    } else {
        Ok(())
    }
}

fn check_entry_metadata(entry: &zip::read::ZipFile<'_>, limits: ZipLimits) -> io::Result<()> {
    if entry.size() > limits.max_entry_bytes {
        return Err(limit_error("zip entry size limit exceeded"));
    }
    if entry.size() > limits.max_total_bytes {
        return Err(limit_error("zip total size limit exceeded"));
    }
    let ratio_limit = entry
        .compressed_size()
        .saturating_mul(limits.max_compression_ratio);
    if entry.size() > ratio_limit {
        return Err(limit_error("zip compression ratio limit exceeded"));
    }
    Ok(())
}

fn read_limited<R: Read, W: Write>(reader: &mut R, writer: &mut W, limit: u64) -> io::Result<u64> {
    let mut written = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let remaining = limit.saturating_sub(written);
        let read_cap = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        if read_cap == 0 {
            let mut extra = [0_u8; 1];
            if reader.read(&mut extra)? != 0 {
                return Err(limit_error("zip byte limit exceeded"));
            }
            return Ok(written);
        }
        let count = reader.read(&mut buffer[..read_cap])?;
        if count == 0 {
            return Ok(written);
        }
        writer.write_all(&buffer[..count])?;
        written += count as u64;
    }
}

fn validate_relative_path(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "invalid zip entry path",
        ));
    }
    Ok(())
}

fn ensure_secure_dirs(root: &Path, relative: &Path) -> io::Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "invalid zip entry path",
            ));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => validate_directory(root, &current, &metadata)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(err) => return Err(err),
                }
                let metadata = fs::symlink_metadata(&current)?;
                validate_directory(root, &current, &metadata)?;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn validate_directory(root: &Path, path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if is_link_or_reparse(metadata) || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "zip output path contains symlink, reparse point, or non-directory",
        ));
    }
    if !path.canonicalize()?.starts_with(root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "zip entry escapes output directory",
        ));
    }
    Ok(())
}

fn reject_existing_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "zip output target is symlink or reparse point",
        )),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "zip output target already exists",
        )),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn limit_error(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn zip_error_to_io(err: zip::result::ZipError) -> io::Error {
    io::Error::other(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "perro_zip_{name}_{}_{}",
                std::process::id(),
                id
            ));
            fs::create_dir_all(&path).expect("required value must be present");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).expect("required value must be present");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            writer
                .start_file(*name, options)
                .expect("required value must be present");
            writer
                .write_all(data)
                .expect("required value must be present");
        }
        writer.finish().expect("required value must be present");
    }

    #[test]
    fn read_rejects_high_compression_ratio() {
        let temp = TempDir::new("ratio");
        let archive = temp.0.join("data.zip");
        let data = vec![0_u8; 1024 * 1024];
        make_zip(&archive, &[("zeros.bin", &data)]);
        let limits = ZipLimits {
            max_compression_ratio: 2,
            ..ZipLimits::default()
        };

        let err = read_zip_file_entry_with_limits(&archive, "zeros.bin", limits)
            .expect_err("operation must fail in this test");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("compression ratio"));
    }

    #[test]
    fn extract_rejects_entry_count_limit() {
        let temp = TempDir::new("count");
        let archive = temp.0.join("data.zip");
        let entries = (0..8)
            .map(|index| (format!("{index}.txt"), b"x".as_slice()))
            .collect::<Vec<_>>();
        let refs = entries
            .iter()
            .map(|(name, data)| (name.as_str(), *data))
            .collect::<Vec<_>>();
        make_zip(&archive, &refs);
        let limits = ZipLimits {
            max_entries: 4,
            ..ZipLimits::default()
        };

        let err = extract_zip_file_with_limits(&archive, temp.0.join("out"), limits)
            .expect_err("operation must fail in this test");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("entry count"));
    }

    #[test]
    fn extract_removes_file_that_hits_total_limit() {
        let temp = TempDir::new("total");
        let archive = temp.0.join("data.zip");
        make_zip(&archive, &[("one.bin", b"1234"), ("two.bin", b"5678")]);
        let output = temp.0.join("out");
        let limits = ZipLimits {
            max_total_bytes: 6,
            ..ZipLimits::default()
        };

        let err = extract_zip_file_with_limits(&archive, &output, limits)
            .expect_err("operation must fail in this test");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read(output.join("one.bin")).expect("required value must be present"),
            b"1234"
        );
        assert!(!output.join("two.bin").exists());
    }

    #[cfg(unix)]
    #[test]
    fn extract_rejects_final_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new("final_link");
        let archive = temp.0.join("data.zip");
        make_zip(&archive, &[("file.txt", b"bad")]);
        let output = temp.0.join("out");
        fs::create_dir(&output).expect("required value must be present");
        let outside = temp.0.join("outside.txt");
        fs::write(&outside, b"safe").expect("required value must be present");
        symlink(&outside, output.join("file.txt")).expect("required value must be present");

        assert!(extract_zip_file(&archive, &output).is_err());
        assert_eq!(
            fs::read(outside).expect("required value must be present"),
            b"safe"
        );
    }

    #[cfg(unix)]
    #[test]
    fn extract_rejects_nested_symlink() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new("nested_link");
        let archive = temp.0.join("data.zip");
        make_zip(&archive, &[("nested/file.txt", b"bad")]);
        let output = temp.0.join("out");
        let outside = temp.0.join("outside");
        fs::create_dir(&output).expect("required value must be present");
        fs::create_dir(&outside).expect("required value must be present");
        symlink(&outside, output.join("nested")).expect("required value must be present");

        assert!(extract_zip_file(&archive, &output).is_err());
        assert!(!outside.join("file.txt").exists());
    }

    #[cfg(windows)]
    fn try_symlink_file(original: &Path, link: &Path) -> bool {
        match std::os::windows::fs::symlink_file(original, link) {
            Ok(()) => true,
            Err(err)
                if err.kind() == io::ErrorKind::PermissionDenied
                    || err.raw_os_error() == Some(1314) =>
            {
                false
            }
            Err(err) => panic!("symlink create failed: {err}"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn extract_rejects_final_symlink() {
        let temp = TempDir::new("final_link");
        let archive = temp.0.join("data.zip");
        make_zip(&archive, &[("file.txt", b"bad")]);
        let output = temp.0.join("out");
        fs::create_dir(&output).expect("required value must be present");
        let outside = temp.0.join("outside.txt");
        fs::write(&outside, b"safe").expect("required value must be present");
        if !try_symlink_file(&outside, &output.join("file.txt")) {
            return;
        }

        assert!(extract_zip_file(&archive, &output).is_err());
        assert_eq!(
            fs::read(outside).expect("required value must be present"),
            b"safe"
        );
    }

    #[cfg(windows)]
    #[test]
    fn extract_rejects_nested_symlink() {
        let temp = TempDir::new("nested_link");
        let archive = temp.0.join("data.zip");
        make_zip(&archive, &[("nested/file.txt", b"bad")]);
        let output = temp.0.join("out");
        let outside = temp.0.join("outside");
        fs::create_dir(&output).expect("required value must be present");
        fs::create_dir(&outside).expect("required value must be present");
        match std::os::windows::fs::symlink_dir(&outside, output.join("nested")) {
            Ok(()) => {}
            Err(err)
                if err.kind() == io::ErrorKind::PermissionDenied
                    || err.raw_os_error() == Some(1314) =>
            {
                return;
            }
            Err(err) => panic!("symlink create failed: {err}"),
        }

        assert!(extract_zip_file(&archive, &output).is_err());
        assert!(!outside.join("file.txt").exists());
    }
}
