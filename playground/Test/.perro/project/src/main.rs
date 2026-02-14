use std::path::PathBuf;

fn project_root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    root.canonicalize().unwrap_or(root)
}

fn main() {
    let root = project_root();
    perro_app::entry::run_dev_project_from_path(&root, "Perro Project")
        .expect("failed to run project");
}
