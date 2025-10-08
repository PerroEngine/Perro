use std::{collections::HashSet, path::{Path, PathBuf}, time::Instant};
use walkdir::WalkDir;

use crate::{
    asset_io::{get_project_root, load_asset, resolve_path, ProjectRoot, ResolvedPath},
    compiler::{BuildProfile, CompileTarget, Compiler},
    lang::{codegen::write_to_crate, pup::parser::PupParser}
};

/// Convert file path to identifier: "bob.pup" -> "bob_pup"
pub fn script_path_to_identifier(path: &str) -> Result<String, String> {
    let path_obj = Path::new(path);
    let base_name = path_obj
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Failed to extract filename".to_string())?;

    let extension = path_obj
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| "Failed to extract file extension".to_string())?;

    Ok(format!("{}_{}", base_name.to_lowercase(), extension.to_lowercase()))
}

/// Discover all script files in res/ directory, returns res:// paths
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
                if ext == "pup" || ext == "rs" {
                    scripts.push(path.to_path_buf()); // ✅ full absolute disk path
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
    use std::collections::HashSet;
    use std::fs;
    
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

/// Completely rebuild lib.rs with only active scripts
fn rebuild_lib_rs(project_root: &Path, active_ids: &HashSet<String>) -> Result<(), String> {
    use std::fs;
    
    let lib_rs_path = project_root.join(".perro/scripts/src/lib.rs");
    
    let mut content = String::from(
        "use perro_core::script::{CreateFn, Script};\n\
         use std::collections::HashMap;\n\n"
    );
    
    // Add modules
    for id in active_ids {
        content.push_str(&format!("pub mod {};\n", id));
    }
    content.push('\n');
    
    // Add imports
    for id in active_ids {
        content.push_str(&format!("use {}::{}_create_script;\n", id, id));
    }
    content.push('\n');
    
    // Add registry function
    content.push_str("pub fn get_script_registry() -> HashMap<String, CreateFn> {\n");
    content.push_str("    let mut map: HashMap<String, CreateFn> = HashMap::new();\n");
    
    for id in active_ids {
        content.push_str(&format!(
            "    map.insert(\"{}\".to_string(), {}_create_script as CreateFn);\n",
            id, id
        ));
    }
    
    content.push_str("    map\n");
    content.push_str("}\n");
    
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

    // Build active script names for cleanup
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

        let code = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        match extension {
            "pup" => {
                let script = PupParser::new(&code).parse_script()?;
                script.to_rust(&identifier, project_root);
                println!(
                    "  ✅ Transpiled: {} -> {} ({:.2?})",
                    path.display(),
                    identifier,
                    script_start.elapsed()
                );
            }
            "rs" => {
                write_to_crate(project_root, &code, &identifier)?;
                println!(
                    "  ✅ Copied: {} -> {} ({:.2?})",
                    path.display(),
                    identifier,
                    script_start.elapsed()
                );
            }
            _ => return Err(format!("Unsupported file extension: {}", extension)),
        }
    }

    println!("✅ Total transpilation time: {:.2?}", total_start.elapsed());
    Ok(())
}
