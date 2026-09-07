use std::{
    collections::HashSet,
    fs,
    io::{self, Read},
    path::Path,
};

#[cfg(test)]
#[path = "demo_sync_perf_tests.rs"]
mod perf_tests;

/// Mirror generated bundles while preserving identical files and their mtimes.
/// Compare bytes as well as length: file timestamps alone do not establish an
/// unchanged build output. Buffers stay bounded even for large WASM modules.
pub(crate) fn sync_dir(src: &Path, dst: &Path) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(dst) {
        if !meta.is_dir() || meta.file_type().is_symlink() {
            remove_path(dst)?;
        }
    }
    fs::create_dir_all(dst)?;
    let mut names = HashSet::new();
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let source = entry.path();
        let target = dst.join(&name);
        names.insert(name);
        if source.is_dir() {
            sync_dir(&source, &target)?;
        } else {
            if let Ok(meta) = fs::symlink_metadata(&target) {
                if meta.is_dir() || meta.file_type().is_symlink() {
                    remove_path(&target)?;
                }
            }
            if !same_file(&source, &target)? {
                fs::copy(&source, &target)?;
            }
        }
    }
    for entry in fs::read_dir(dst)? {
        let entry = entry?;
        if !names.contains(&entry.file_name()) {
            remove_path(&entry.path())?;
        }
    }
    Ok(())
}

fn same_file(src: &Path, dst: &Path) -> io::Result<bool> {
    let dst_meta = match fs::metadata(dst) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };
    if fs::metadata(src)?.len() != dst_meta.len() {
        return Ok(false);
    }
    let mut source = fs::File::open(src)?;
    let mut target = fs::File::open(dst)?;
    let mut left = [0u8; 64 * 1024];
    let mut right = [0u8; 64 * 1024];
    loop {
        let read = source.read(&mut left)?;
        if read == 0 {
            return Ok(true);
        }
        target.read_exact(&mut right[..read])?;
        if left[..read] != right[..read] {
            return Ok(false);
        }
    }
}

fn remove_path(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        if path.is_dir() {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        }
    } else if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_preserves_equal_mtime_and_applies_same_size_edits_and_removals() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("perro_demo_sync_{}_{stamp}", std::process::id()));
        let src = root.join("src");
        let dst = root.join("dst");
        fs::create_dir_all(src.join("dir")).expect("fixture");
        fs::write(src.join("app.wasm"), b"original").expect("source");
        fs::write(src.join("dir/old.txt"), b"old").expect("nested source");
        sync_dir(&src, &dst).expect("first sync");
        let mtime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        fs::OpenOptions::new()
            .write(true)
            .open(dst.join("app.wasm"))
            .expect("output")
            .set_times(fs::FileTimes::new().set_modified(mtime))
            .expect("set mtime");
        sync_dir(&src, &dst).expect("no-op sync");
        assert_eq!(
            fs::metadata(dst.join("app.wasm"))
                .expect("stat")
                .modified()
                .expect("mtime"),
            mtime
        );
        fs::write(src.join("app.wasm"), b"modified").expect("same-size source edit");
        fs::remove_dir_all(src.join("dir")).expect("remove source dir");
        fs::write(src.join("dir"), b"replace directory with file").expect("replace source");
        sync_dir(&src, &dst).expect("changed sync");
        assert_eq!(fs::read(dst.join("app.wasm")).expect("output"), b"modified");
        assert_eq!(
            fs::read(dst.join("dir")).expect("replacement"),
            b"replace directory with file"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
