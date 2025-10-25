use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;

use crate::{
    asset_io::{get_project_root, load_asset, resolve_path, ProjectRoot, ResolvedPath},
    compiler::{BuildProfile, CompileTarget, Compiler},
    lang::{codegen::{derive_rust_perro_script, write_to_crate}, csharp::parser::CsParser, pup::parser::PupParser},
};

/// Convert a *res:// path* or absolute path under res/
/// into a unique Rust-safe identifier.
///
/// Examples:
/// - res://bob.pup → bob_pup
/// - res://scripts/bob.pup → scripts_bob_pup
/// - /abs/path/myproject/res/scripts/test/bob.pup → scripts_test_bob_pup
pub fn script_path_to_identifier(path: &str) -> Result<String, String> {
    // Normalize path separators
    let mut cleaned = path.replace('\\', "/");

    // Strip "res://" or "user://" prefixes
    if cleaned.starts_with("res://") {
        cleaned = cleaned.trim_start_matches("res://").to_string();
    } else if cleaned.starts_with("user://") {
        cleaned = cleaned.trim_start_matches("user://").to_string();
    } else if let Some(idx) = cleaned.find("/res/") {
        // Strip everything before /res/
        cleaned = cleaned[(idx + 5)..].to_string(); // skip past '/res/'
    } else if let Some(idx) = cleaned.find("res/") {
        cleaned = cleaned[(idx + 4)..].to_string();
    }

    // Now cleaned should look like "scripts/editor.pup"
    let path_obj = Path::new(&cleaned);

    let base_name = path_obj
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or("Failed to extract filename")?;

    let extension = path_obj
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Failed to extract extension")?;

    let parent_str = path_obj
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .replace('/', "_");

    let mut identifier = String::new();
    if !parent_str.is_empty() {
        identifier.push_str(&parent_str);
        identifier.push('_');
    }
    identifier.push_str(&format!("{}_{}", base_name.to_lowercase(), extension.to_lowercase()));

    Ok(identifier)
}

/// Discover all script files in res/ directory, returns disk paths
fn discover_scripts(project_root: &Path) -> Result<Vec<PathBuf>, String> {
    let res_dir = project_root.join("res");
    if !res_dir.exists() {
        return Err(format!("res/ directory not found at {:?}", res_dir));
    }

    let mut scripts = Vec::new();

    for entry in WalkDir::new(&res_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ["pup", "rs", "cs"].contains(&ext) {
                    scripts.push(path.to_path_buf());
                }
            }
        }
    }

    if scripts.is_empty() {
        println!("⚠️  No scripts found in res/");
    }

    Ok(scripts)
}

/// Clean up orphaned script files
fn clean_orphaned_scripts(project_root: &Path, active_scripts: &[String]) -> Result<(), String> {
    let scripts_src = project_root.join(".perro/scripts/src");

    // Get identifiers for all active scripts
    let active_ids: HashSet<String> = active_scripts
        .iter()
        .filter_map(|path| script_path_to_identifier(path).ok())
        .collect();

    // Remove orphaned .rs files
    if let Ok(entries) = fs::read_dir(&scripts_src) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if stem != "lib" && !active_ids.contains(stem) {
                        println!("🗑️  Removing orphaned script: {}", stem);
                        fs::remove_file(&path).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }

    rebuild_lib_rs(project_root, &active_ids)?;
    Ok(())
}

pub fn rebuild_lib_rs(project_root: &Path, active_ids: &HashSet<String>) -> Result<(), String> {
    let lib_rs_path = project_root.join(".perro/scripts/src/lib.rs");

    let mut content = String::from(
        "#[cfg(debug_assertions)]\nuse std::ffi::CStr;\n#[cfg(debug_assertions)]\nuse std::os::raw::c_char;\n\
        use perro_core::script::CreateFn;\nuse std::collections::HashMap;\n\n\
        // __PERRO_MODULES__\n// __PERRO_IMPORTS__\n\n\
        pub fn get_script_registry() -> HashMap<String, CreateFn> {\n\
        let mut map: HashMap<String, CreateFn> = HashMap::new();\n\
        // __PERRO_REGISTRY__\n\
        map\n}\n\n"
    );

    // Modules
    for id in active_ids {
        content = content.replace(
            "// __PERRO_MODULES__",
            &format!("pub mod {};\n// __PERRO_MODULES__", id)
        );
    }

    // Imports
    for id in active_ids {
        content = content.replace(
            "// __PERRO_IMPORTS__",
            &format!("use {}::{}_create_script;\n// __PERRO_IMPORTS__", id, id)
        );
    }

    // Registry entries
    for id in active_ids {
        content = content.replace(
            "// __PERRO_REGISTRY__",
            &format!("    map.insert(\"{}\".to_string(), {}_create_script as CreateFn);\n    // __PERRO_REGISTRY__", id, id)
        );
    }

    // Add debug-only FFI function for project root
    let ffi_fn_marker = "perro_set_project_root";
    let debug_root_fn = format!(
        r#"
#[cfg(debug_assertions)]
#[unsafe(no_mangle)]
pub extern "C" fn {}(path: *const c_char, name: *const c_char) {{
    let path_str = unsafe {{ CStr::from_ptr(path).to_str().unwrap() }};
    let name_str = unsafe {{ CStr::from_ptr(name).to_str().unwrap() }};
    perro_core::asset_io::set_project_root(
        perro_core::asset_io::ProjectRoot::Disk {{
            root: std::path::PathBuf::from(path_str),
            name: name_str.to_string(),
        }}
    );
}}
"#,
        ffi_fn_marker
    );

    content.push_str(&debug_root_fn);

    fs::write(&lib_rs_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn transpile(project_root: &Path) -> Result<(), String> {
    let total_start = Instant::now();

    let script_paths = discover_scripts(project_root)?;
    if script_paths.is_empty() {
        return Ok(());
    }

    println!("📜 Found {} script(s)", script_paths.len());

    let script_res_paths: Vec<String> = script_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    clean_orphaned_scripts(project_root, &script_res_paths)?;

    for path in &script_paths {
        let script_start = Instant::now();
        let identifier = script_path_to_identifier(&path.to_string_lossy())?;

        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Failed to extract file extension".to_string())?;

        let code = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        match extension {
            "pup" => {
                let script = PupParser::new(&code).parse_script()?;
                script.to_rust(&identifier, project_root);
            }
            "cs" => {
                let script = CsParser::new(&code).parse_script()?;
                script.to_rust(&identifier, project_root);
            }
            "rs" => {
                derive_rust_perro_script(project_root, &code, &identifier)?;
            }
            _ => return Err(format!("Unsupported file extension: {}", extension)),
        }

        println!(
            "  ✅ Transpiled: {} -> {} ({:.2?})",
            path.display(),
            identifier,
            script_start.elapsed()
        );
    }

    println!("✅ Total transpilation time: {:.2?}", total_start.elapsed());
    Ok(())
}