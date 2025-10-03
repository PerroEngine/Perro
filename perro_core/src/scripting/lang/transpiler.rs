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
fn discover_scripts(project_root: &Path) -> Result<Vec<String>, String> {
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
        
        // Skip if not a file
        if !path.is_file() {
            continue;
        }
        
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            // Exact match only
            if ext == "pup" || ext == "rs" {
                // Convert to res:// path
                if let Ok(relative) = path.strip_prefix(&res_dir) {
                    let res_path = format!("res://{}", relative.display());
                    scripts.push(res_path);
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

/// Transpile all scripts found in res/ directory
pub fn transpile() -> Result<(), String> {
    let total_start = Instant::now();
    
    let project_root = match get_project_root() {
        ProjectRoot::Disk { root, .. } => root,
        ProjectRoot::Brk { .. } => {
            return Err("Transpilation is not supported in release/pak mode".into());
        }
    };

    // Discover all scripts as res:// paths
    let script_paths = discover_scripts(&project_root)?;
    
    if script_paths.is_empty() {
        return Ok(());
    }
    
    println!("📜 Found {} script(s)", script_paths.len());
    
    // Clean up orphans
    clean_orphaned_scripts(&project_root, &script_paths)?;

    // Transpile all discovered scripts
    for path in &script_paths {
        let script_start = Instant::now();
        
        let resolved = resolve_path(path);
        
        match resolved {
            ResolvedPath::Disk(script_path) => {
                let identifier = script_path_to_identifier(path)?;
                
                let extension = script_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .ok_or_else(|| "Failed to extract file extension".to_string())?;

                let code_bytes = load_asset(path)
                    .map_err(|e| format!("Failed to read {}: {}", path, e))?;
                let code = String::from_utf8(code_bytes)
                    .map_err(|e| format!("Invalid UTF-8 in {}: {}", path, e))?;

                match extension {
                    "pup" => {
                        let script = PupParser::new(&code).parse_script()?;
                        script.to_rust(&identifier);
                        let elapsed = script_start.elapsed();
                        println!("  ✅ Transpiled: {} -> {} ({:.2?})", path, identifier, elapsed);
                    }
                    "rs" => {
                        write_to_crate(&code, &identifier)?;
                        let elapsed = script_start.elapsed();
                        println!("  ✅ Copied: {} -> {} ({:.2?})", path, identifier, elapsed);
                    }
                    _ => {
                        return Err(format!("Unsupported file extension: {}", extension));
                    }
                }
            }
            ResolvedPath::Brk(_) => {
                return Err("Transpilation is only supported in dev/disk mode".into());
            }
        }
    }

    let total_elapsed = total_start.elapsed();
    println!("✅ Total transpilation time: {:.2?}", total_elapsed);

    Ok(())
}