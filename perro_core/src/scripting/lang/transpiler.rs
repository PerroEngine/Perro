use std::path::Path;

use crate::{
    compiler::{BuildProfile, CompileTarget, Compiler},
    lang::pup::parser::PupParser,
    asset_io::{get_project_root, resolve_path, load_asset, ResolvedPath, ProjectRoot},
};

/// Transpile one or more scripts, then compile once at the end.
pub fn transpile(paths: &[&str]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No script paths provided".into());
    }

    // ✅ Extract a real PathBuf from ProjectRoot
    let project_root_path = match get_project_root() {
        ProjectRoot::Disk { root, .. } => root,
        ProjectRoot::Pak { .. } => {
            return Err("Transpilation is not supported in release/pak mode".into());
        }
    };

    // Now transpile all scripts
    for path in paths {
        match resolve_path(path) {
            ResolvedPath::Disk(script_path) => {
                let path_obj = Path::new(&script_path);

                let script_name = path_obj
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| "Failed to extract filename".to_string())?
                    .to_string();

                let script_name = if let Some(first_char) = script_name.chars().next() {
                    let mut chars = script_name.chars();
                    chars.next();
                    format!("{}{}", first_char.to_uppercase(), chars.as_str())
                } else {
                    script_name
                };

                let extension = path_obj
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .ok_or_else(|| "Failed to extract file extension".to_string())?;

                // ✅ Use load_asset instead of std::fs::read_to_string
                let code_bytes = load_asset(path)
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                let code = String::from_utf8(code_bytes)
                    .map_err(|e| format!("Invalid UTF-8 in script: {}", e))?;

                let script = match extension {
                    "pup" => PupParser::new(&code).parse_script()?,
                    _ => return Err(format!("Unsupported file extension: {}", extension)),
                };

                script.to_rust(&script_name);

                println!("✅ Transpile succeeded: {}", path);
            }
            ResolvedPath::Pak(_) => {
                return Err("Transpilation is only supported in dev/disk mode".into());
            }
        }
    }

    // 🔑 Compile once after all transpiles
    let compiler = Compiler::new(&project_root_path, CompileTarget::Scripts);
    compiler.compile(BuildProfile::Dev)?;

    Ok(())
}