use std::path::Path;

use crate::{lang::pup::parser::{PupParser}, resolve_res_path};


pub fn transpile(path: &str) -> Result<String, String> {
    let script_path = resolve_res_path(path);

    let path_obj = Path::new(&script_path);

    let script_name = path_obj
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Failed to extract filename".to_string())?
        .to_string(); // Convert to owned String

    let script_name = if let Some(first_char) = script_name.chars().next() {
        let mut chars = script_name.chars();
        chars.next(); // Remove first char
        format!("{}{}", first_char.to_uppercase(), chars.as_str())
    } else {
        script_name
    };

    let extension = path_obj
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| "Failed to extract file extension".to_string())?;

    // Read the code from the file
    let code = std::fs::read_to_string(&script_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Match on the extension and instantiate the correct parser
   let script = match extension {
        "pup" => PupParser::new(&code).parse_script()?,

        _ => return Err(format!("Unsupported file extension: {}", extension)),
    };

    let rust_code = script.to_rust(&script_name);
    Ok(rust_code) // ✅ Return the generated code
}