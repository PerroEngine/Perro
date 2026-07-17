#[cfg(any(not(target_arch = "wasm32"), feature = "profile"))]
use super::*;

#[cfg(target_os = "windows")]
pub(super) fn runtime_scripts_dylib_name() -> &'static str {
    "scripts.dll"
}

#[cfg(target_os = "linux")]
pub(super) fn runtime_scripts_dylib_name() -> &'static str {
    "libscripts.so"
}

#[cfg(target_os = "macos")]
pub(super) fn runtime_scripts_dylib_name() -> &'static str {
    "libscripts.dylib"
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn resolve_dev_dlc_scripts_dylib_path(
    project_root: &Path,
    dlc_name: &str,
) -> Option<PathBuf> {
    let staged = project_root
        .join(".perro")
        .join("dlc")
        .join(dlc_name)
        .join("scripts")
        .join(runtime_scripts_dylib_name());
    if staged.exists() {
        return Some(staged);
    }
    None
}

#[cfg(target_os = "windows")]
pub(super) fn runtime_pack_dylib_name() -> &'static str {
    "pack.dll"
}

#[cfg(target_os = "linux")]
pub(super) fn runtime_pack_dylib_name() -> &'static str {
    "libpack.so"
}

#[cfg(target_os = "macos")]
pub(super) fn runtime_pack_dylib_name() -> &'static str {
    "libpack.dylib"
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn parse_manifest_string(manifest: &str, key: &str) -> Option<String> {
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || !trimmed.starts_with(key) {
            continue;
        }
        let (_, rhs) = trimmed.split_once('=')?;
        let value = rhs.trim().trim_matches('"').to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn extract_dlc_archive_file_to_cache(
    dlc_name: &str,
    virtual_path: &str,
    cache_root: &Path,
) -> Result<PathBuf, std::io::Error> {
    validate_asset_relative_path(virtual_path)?;
    let bytes = read_mounted_dlc_file(dlc_name, virtual_path)?;
    write_dlc_cache_file(cache_root, virtual_path, &bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn write_dlc_cache_file(
    cache_root: &Path,
    virtual_path: &str,
    bytes: &[u8],
) -> io::Result<PathBuf> {
    validate_asset_relative_path(virtual_path)?;
    let relative = Path::new(virtual_path);
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let secure_parent = ensure_secure_cache_dir(cache_root, parent)?;
    let canonical_root = cache_root.canonicalize()?;
    let mut target = cache_root.to_path_buf();
    for segment in virtual_path.split('/') {
        if !segment.is_empty() {
            target.push(segment);
        }
    }
    if target.parent() != Some(secure_parent.as_path()) {
        return Err(cache_permission_error(
            "dlc cache target escapes cache root",
        ));
    }
    reject_linked_cache_target(&target)?;

    let mut file = open_cache_target_no_follow(&target)?;
    let metadata = file.metadata()?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(cache_permission_error(
            "dlc cache target is link, reparse point, or non-file",
        ));
    }
    if !target.canonicalize()?.starts_with(&canonical_root) {
        return Err(cache_permission_error(
            "dlc cache target escapes cache root",
        ));
    }
    file.set_len(0)?;
    file.write_all(bytes)?;
    Ok(target)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn ensure_secure_cache_dir(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    if relative
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
        && !relative.as_os_str().is_empty()
    {
        return Err(cache_permission_error("invalid dlc cache path"));
    }

    let root_metadata = fs::symlink_metadata(root)?;
    if is_link_or_reparse(&root_metadata) || !root_metadata.is_dir() {
        return Err(cache_permission_error(
            "dlc cache root is link, reparse point, or non-directory",
        ));
    }
    let canonical_root = root.canonicalize()?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(cache_permission_error("invalid dlc cache path"));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => validate_cache_dir(&canonical_root, &current, &metadata)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(err) => return Err(err),
                }
                let metadata = fs::symlink_metadata(&current)?;
                validate_cache_dir(&canonical_root, &current, &metadata)?;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(current)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn validate_cache_dir(
    canonical_root: &Path,
    path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<()> {
    if is_link_or_reparse(metadata) || !metadata.is_dir() {
        return Err(cache_permission_error(
            "dlc cache path contains link, reparse point, or non-directory",
        ));
    }
    if !path.canonicalize()?.starts_with(canonical_root) {
        return Err(cache_permission_error("dlc cache path escapes cache root"));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn reject_linked_cache_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) => Err(cache_permission_error(
            "dlc cache target is link or reparse point",
        )),
        Ok(metadata) if !metadata.is_file() => {
            Err(cache_permission_error("dlc cache target is not a file"))
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn open_cache_target_no_follow(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true);
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        const O_NOFOLLOW: i32 = 0x2_0000;
        options.custom_flags(O_NOFOLLOW);
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        const O_NOFOLLOW: i32 = 0x100;
        options.custom_flags(O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x20_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

#[cfg(windows)]
pub(super) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
pub(super) fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn cache_permission_error(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(feature = "profile")]
pub(super) fn as_us(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

#[cfg(feature = "profile")]
pub(super) fn fmt_duration(duration: Option<Duration>) -> String {
    duration
        .map(|value| format!("{:.3}", as_us(value)))
        .unwrap_or_else(|| "n/a".to_string())
}
