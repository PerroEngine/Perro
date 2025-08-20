use std::path:: {PathBuf};
use std::sync::OnceLock;

/// Global project root
static PROJECT_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Set the project root (where settings.toml and .perro/ live)
pub fn set_project_root(path: PathBuf) {
    let _ = PROJECT_ROOT.set(path);
}

/// Get the project root
pub fn get_project_root() -> PathBuf {
    PROJECT_ROOT
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap())
}

/// Resolve a resource path like res:// or editor:// into a PathBuf
pub fn resolve_res_path(res_path: &str) -> PathBuf {
    if let Some(stripped) = res_path.strip_prefix("res://") {
        let mut pb = get_project_root();
        pb.push("res");
        pb.push(stripped);
        return pb;
    }
    PathBuf::from(res_path)
}