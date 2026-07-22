use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    #[cfg(not(windows))]
    false
}

fn reject_link(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if is_link_or_reparse(metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing to walk symbolic link or reparse point: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn canonical_in_root(path: &Path, root: &Path) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(path)?;
    if !canonical.starts_with(root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("walk path escapes root: {}", path.display()),
        ));
    }
    Ok(canonical)
}

fn validate_collection_root(dir: &Path, base: &Path) -> io::Result<()> {
    let base_metadata = fs::symlink_metadata(base)?;
    reject_link(base, &base_metadata)?;
    if !base_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("collection base is not a directory: {}", base.display()),
        ));
    }

    let canonical_base = fs::canonicalize(base)?;
    let canonical_dir = fs::canonicalize(dir)?;
    if !canonical_dir.starts_with(&canonical_base) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("collection root escapes base: {}", dir.display()),
        ));
    }
    Ok(())
}

/// Visits all files in a directory tree, calling the provided callback for each file.
pub fn walk_dir<F>(dir: &Path, callback: &mut F) -> io::Result<()>
where
    F: FnMut(&Path) -> io::Result<()>,
{
    let root_metadata = fs::symlink_metadata(dir)?;
    reject_link(dir, &root_metadata)?;
    if !root_metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("walk root is not a directory: {}", dir.display()),
        ));
    }

    let root = fs::canonicalize(dir)?;
    let mut pending = vec![dir.to_path_buf()];
    let mut visited = HashSet::new();

    while let Some(current) = pending.pop() {
        let canonical = canonical_in_root(&current, &root)?;
        if !visited.insert(canonical) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("directory walk loop: {}", current.display()),
            ));
        }

        for entry in fs::read_dir(&current)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            reject_link(&path, &metadata)?;

            if metadata.is_dir() {
                canonical_in_root(&path, &root)?;
                pending.push(path);
            } else if metadata.is_file() {
                canonical_in_root(&path, &root)?;
                callback(&path)?;
            }
        }
    }
    Ok(())
}

/// Collect all files in a directory tree with their relative paths
pub fn collect_files(dir: &Path, base: &Path) -> io::Result<Vec<(String, Vec<u8>)>> {
    validate_collection_root(dir, base)?;
    let mut files = Vec::new();

    walk_dir(dir, &mut |path| {
        let rel = path
            .strip_prefix(base)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?
            .to_string_lossy()
            .to_string();
        let data = fs::read(path)?;
        files.push((rel, data));
        Ok(())
    })?;

    Ok(files)
}

/// Collect file paths only (no data)
pub fn collect_file_paths(dir: &Path, base: &Path) -> io::Result<Vec<String>> {
    validate_collection_root(dir, base)?;
    let mut paths = Vec::new();

    walk_dir(dir, &mut |path| {
        let rel = path
            .strip_prefix(base)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?
            .to_string_lossy()
            .to_string();
        paths.push(rel);
        Ok(())
    })?;

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::{collect_file_paths, walk_dir};
    use std::{
        fs, io,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "perro-assets-walk-{label}-{}-{serial}",
                std::process::id()
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

    fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(target, link)
        }
    }

    #[test]
    fn walk_rejects_link_outside_root() {
        let root = TempDir::new("outside-root");
        let outside = TempDir::new("outside-target");
        fs::write(outside.0.join("secret.txt"), b"secret").expect("required value must be present");
        if symlink_dir(&outside.0, &root.0.join("outside")).is_err() {
            return;
        }

        let err = walk_dir(&root.0, &mut |_| Ok(())).expect_err("operation must fail in this test");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn walk_rejects_link_loop() {
        let root = TempDir::new("loop");
        let child = root.0.join("child");
        fs::create_dir(&child).expect("required value must be present");
        if symlink_dir(&root.0, &child.join("loop")).is_err() {
            return;
        }

        let err = walk_dir(&root.0, &mut |_| Ok(())).expect_err("operation must fail in this test");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn collect_rejects_base_outside_walk_root() {
        let root = TempDir::new("base-root");
        let other = TempDir::new("base-other");
        fs::write(root.0.join("asset.bin"), b"asset").expect("required value must be present");

        let err =
            collect_file_paths(&root.0, &other.0).expect_err("operation must fail in this test");
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }
}
