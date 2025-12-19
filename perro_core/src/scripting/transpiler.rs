use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use walkdir::WalkDir;

use crate::{
    asset_io::{ProjectRoot, ResolvedPath, get_project_root, load_asset, resolve_path},
    codegen::derive_rust_perro_script,
    compiler::{BuildProfile, CompileTarget, Compiler},
    lang::{
        csharp::parser::CsParser, pup::parser::PupParser, typescript::parser::TypeScriptParser,
    },
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
    identifier.push_str(&format!(
        "{}_{}",
        base_name.to_lowercase(),
        extension.to_lowercase()
    ));

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
                if ["pup", "rs", "cs", "ts"].contains(&ext) {
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

    if let Some(parent) = lib_rs_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return Err(format!("Failed to create script source dir: {}", e));
        }
    }

    let mut content = String::from(
        "#[cfg(debug_assertions)]\nuse std::ffi::CStr;\n#[cfg(debug_assertions)]\nuse std::os::raw::c_char;\n\
        use perro_core::script::CreateFn;\nuse phf::{phf_map, Map};\n\n\
        // __PERRO_MODULES__\n// __PERRO_IMPORTS__\n\n\
        pub fn get_script_registry() -> &'static Map<&'static str, CreateFn> {\n\
        &SCRIPT_REGISTRY\n\
        }\n\n\
        static SCRIPT_REGISTRY: Map<&'static str, CreateFn> = phf_map! {\n\
        // __PERRO_REGISTRY__\n\
        };\n\n",
    );

    // Sort IDs for deterministic ordering
    let mut sorted_ids: Vec<_> = active_ids.iter().collect();
    sorted_ids.sort();

    // Modules
    for id in &sorted_ids {
        content = content.replace(
            "// __PERRO_MODULES__",
            &format!("pub mod {};\n// __PERRO_MODULES__", id),
        );
    }

    // Imports
    for id in &sorted_ids {
        content = content.replace(
            "// __PERRO_IMPORTS__",
            &format!("use {}::{}_create_script;\n// __PERRO_IMPORTS__", id, id),
        );
    }

    // Registry entries
    for id in &sorted_ids {
        content = content.replace(
            "// __PERRO_REGISTRY__",
            &format!(
                "    \"{}\" => {}_create_script as CreateFn,\n    // __PERRO_REGISTRY__",
                id, id
            ),
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

pub fn transpile(project_root: &Path, verbose: bool) -> Result<(), String> {
    let total_start = Instant::now();

    let script_paths = discover_scripts(project_root)?;

    // Always ensure lib.rs exists, even if there are no scripts
    let script_res_paths: Vec<String> = script_paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    clean_orphaned_scripts(project_root, &script_res_paths)?;

    if script_paths.is_empty() {
        println!("📜 No scripts found. Creating minimal lib.rs...");
        // Still create a minimal lib.rs so the DLL can be built
        rebuild_lib_rs(project_root, &std::collections::HashSet::new())?;
        return Ok(());
    }

    println!("📜 Found {} script(s)", script_paths.len());

    // For summarizing timings at the end
    struct Timing {
        path: String,
        io_time: Duration,
        parse_time: Duration,
        transpile_time: Duration,
        total_time: Duration,
    }

    let mut timings: Vec<Timing> = Vec::new();

    for path in &script_paths {
        let script_total_start = Instant::now();
        let identifier = script_path_to_identifier(&path.to_string_lossy())?;

        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Failed to extract file extension".to_string())?;

        // I/O timing
        let io_start = Instant::now();
        let code = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let io_time = io_start.elapsed();

        let parse_start = Instant::now();
        let transpile_start;
        let parse_time;
        let transpile_time;

        match extension {
            "pup" => {
                let mut script = PupParser::new(&code).parse_script()?;
                parse_time = parse_start.elapsed();

                transpile_start = Instant::now();
                script.to_rust(&identifier, project_root, None, verbose);
                transpile_time = transpile_start.elapsed();
            }
            "cs" => {
                let mut script = CsParser::new(&code).parse_script()?;
                parse_time = parse_start.elapsed();

                transpile_start = Instant::now();
                script.to_rust(&identifier, project_root, None, verbose);
                transpile_time = transpile_start.elapsed();
            }
            "ts" => {
                let mut script = TypeScriptParser::new(&code).parse_script()?;
                parse_time = parse_start.elapsed();

                transpile_start = Instant::now();
                script.to_rust(&identifier, project_root, None, verbose);
                transpile_time = transpile_start.elapsed();
            }
            "rs" => {
                parse_time = Duration::ZERO;
                transpile_start = Instant::now();
                derive_rust_perro_script(project_root, &code, &identifier)?;
                transpile_time = transpile_start.elapsed();
            }
            _ => return Err(format!("Unsupported file extension: {}", extension)),
        }

        let total_time = script_total_start.elapsed();

        timings.push(Timing {
            path: path.display().to_string(),
            io_time,
            parse_time,
            transpile_time,
            total_time,
        });

        println!(
            "  ✅ {} -> {} (I/O: {:.2?}, Parse: {:.2?}, Transpile: {:.2?}, Total: {:.2?})",
            path.display(),
            identifier,
            io_time,
            parse_time,
            transpile_time,
            total_time
        );
    }

    // Overall summary
    let total_time = total_start.elapsed();
    let total_scripts = timings.len();

    let total_io: Duration = timings.iter().map(|t| t.io_time).sum();
    let total_parse: Duration = timings.iter().map(|t| t.parse_time).sum();
    let total_transpile: Duration = timings.iter().map(|t| t.transpile_time).sum();

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(
        "📊 **Transpilation Summary**\n  \
        📜 Scripts: {}\n  \
        💾 Total I/O: {:.2?}\n  \
        🧩 Total Parse: {:.2?}\n  \
        🔧 Total Transpile: {:.2?}\n  \
        ⏱️  Overall Time: {:.2?}",
        total_scripts, total_io, total_parse, total_transpile, total_time
    );
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}
