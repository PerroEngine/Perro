use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;

// Hardcoded relative paths (from .perro/project/ crate to project root)
const PROJECT_MANIFEST: &str = "../../project.toml";
const RES_DIR: &str = "../../res";

// Extensions we want to skip (transpiled source files)
const SKIP_EXTENSIONS: &[&str] = &["pup", "cs", "ts"];

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("res.zip");
    let file = File::create(&dest_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);

    let options = FileOptions::default();

    // Add project.toml
    let manifest_path = Path::new(PROJECT_MANIFEST);
    if !manifest_path.exists() {
        panic!("project.toml not found at {:?}", manifest_path);
    }
    zip.start_file("project.toml", options).unwrap();
    let manifest = std::fs::read(&manifest_path).unwrap();
    zip.write_all(&manifest).unwrap();

    // Add res/ folder
    let res_path = Path::new(RES_DIR);
    if !res_path.exists() {
        panic!("res/ folder not found at {:?}", res_path);
    }

    for entry in WalkDir::new(&res_path) {
        let entry = entry.unwrap();
        let path = entry.path();

        if entry.file_type().is_file() {
            // Skip unwanted extensions
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SKIP_EXTENSIONS.contains(&ext) {
                    println!("Skipping {:?}", path);
                    continue;
                }
            }

            let name = path.strip_prefix(&res_path).unwrap().to_str().unwrap();

            // ✅ Normalize to forward slashes for cross-platform zip compatibility
            let name = name.replace("\\", "/");

            zip.start_file(format!("res/{}", name), options).unwrap();
            let data = std::fs::read(path).unwrap();
            zip.write_all(&data).unwrap();
        }
    }

    zip.finish().unwrap();

    // Re-run build.rs if these change
    println!("cargo:rerun-if-changed={}", PROJECT_MANIFEST);
    println!("cargo:rerun-if-changed={}", RES_DIR);
}