use std::path::Path;

use crate::{
    asset_io::{get_project_root, load_asset, resolve_path, ProjectRoot, ResolvedPath}, compiler::{BuildProfile, CompileTarget, Compiler}, lang::{codegen::write_to_crate, pup::parser::PupParser}
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

    // Create identifier: base_ext (all lowercase)
    Ok(format!("{}_{}", base_name.to_lowercase(), extension.to_lowercase()))
}

/// Transpile one or more scripts, then compile once at the end.
pub fn transpile(paths: &[&str]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No script paths provided".into());
    }

    // Extract a real PathBuf from ProjectRoot
    let project_root_path = match get_project_root() {
        ProjectRoot::Disk { root, .. } => root,
        ProjectRoot::Brk { .. } => {
            return Err("Transpilation is not supported in release/pak mode".into());
        }
    };

    // Transpile all scripts
    for path in paths {
        match resolve_path(path) {
            ResolvedPath::Disk(script_path) => {
                let path_obj = Path::new(&script_path);
                
                // Get identifier like "bob_pup" or "bob_rs"
                let identifier = script_path_to_identifier(script_path.to_str().ok_or("Invalid UTF-8 in path")?)?;

                let extension = path_obj
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .ok_or_else(|| "Failed to extract file extension".to_string())?;

                // Load the source code
                let code_bytes = load_asset(path)
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                let code = String::from_utf8(code_bytes)
                    .map_err(|e| format!("Invalid UTF-8 in script: {}", e))?;

                match extension {
                    "pup" => {
                        // Parse and transpile .pup files
                        let script = PupParser::new(&code).parse_script()?;
                        script.to_rust(&identifier);
                        println!("✅ Transpiled: {} -> {}", path, identifier);
                    }
                    "rs" => {
                        // For .rs files, just write the raw code directly
                        write_to_crate(&code, &identifier)?;
                        println!("✅ Copied raw Rust script: {} -> {}", path, identifier);
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

    Ok(())
}