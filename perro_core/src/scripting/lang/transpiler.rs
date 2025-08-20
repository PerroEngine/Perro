use std::path::Path;

use crate::{
    compiler::{BuildProfile, Compiler}, get_project_root, lang::pup::parser::PupParser, resolve_res_path
};

/// Transpile one or more scripts, then compile once at the end.
pub fn transpile(paths: &[&str]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No script paths provided".into());
    }

    let project_root = get_project_root();

    // Now transpile all scripts
    for path in paths {
        let script_path = resolve_res_path(path);
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

        let code = std::fs::read_to_string(&script_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let script = match extension {
            "pup" => PupParser::new(&code).parse_script()?,
            _ => return Err(format!("Unsupported file extension: {}", extension)),
        };

        script.to_rust(&script_name);

        println!("✅ Transpile succeeded: {}", path);
    }

    // 🔑 Compile once after all transpiles
    let compiler = Compiler::new(&project_root);
    compiler.compile(BuildProfile::Dev)?;

    Ok(())
}