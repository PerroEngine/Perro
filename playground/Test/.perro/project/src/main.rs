use std::path::PathBuf;

fn project_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn main() {
    let root = project_root();
    perro_app::entry::run_static_project_from_path(&root, "Perro Project")
        .expect("failed to run project");
}
