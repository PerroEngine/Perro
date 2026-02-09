use std::{fs, io, path::Path};

/// Visits all files in a directory tree, calling the provided callback for each file.
pub fn walk_dir<F>(dir: &Path, callback: &mut F) -> io::Result<()>
where
    F: FnMut(&Path) -> io::Result<()>,
{
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            walk_dir(&path, callback)?;
        } else if path.is_file() {
            callback(&path)?;
        }
    }
    Ok(())
}

/// Collect all files in a directory tree with their relative paths
pub fn collect_files(dir: &Path, base: &Path) -> io::Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();

    walk_dir(dir, &mut |path| {
        let rel = path
            .strip_prefix(base)
            .unwrap()
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
    let mut paths = Vec::new();

    walk_dir(dir, &mut |path| {
        let rel = path
            .strip_prefix(base)
            .unwrap()
            .to_string_lossy()
            .to_string();
        paths.push(rel);
        Ok(())
    })?;

    Ok(paths)
}
