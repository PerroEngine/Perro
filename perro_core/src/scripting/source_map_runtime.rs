// Runtime utilities for using source maps to convert errors
use crate::scripting::source_map::SourceMap;

/// Load source map from TOML file
pub fn load_source_map(project_root: &std::path::Path) -> Option<SourceMap> {
    let source_map_path = project_root.join(".perro/scripts/sourcemap.toml");
    let content = std::fs::read_to_string(&source_map_path).ok()?;
    toml::from_str(&content).ok()
}

/// Convert an error message using source map
pub fn convert_error_with_source_map(
    source_map: &SourceMap,
    script_identifier: &str,
    error_message: &str,
) -> String {
    let mut result = error_message.to_string();
    
    // Try to find the script in the source map
    if let Some(script_map) = source_map.scripts.get(script_identifier) {
        // Replace identifier names (variables and functions)
        // Use identifier_names, with fallback to variable_names for backwards compatibility
        let name_map = if !script_map.identifier_names.is_empty() {
            &script_map.identifier_names
        } else {
            &script_map.variable_names
        };
        
        for (gen_name, orig_name) in name_map {
            // Replace whole word matches
            let pattern = format!(r"\b{}\b", regex::escape(gen_name));
            if let Ok(re) = regex::Regex::new(&pattern) {
                result = re.replace_all(&result, orig_name.as_str()).to_string();
            }
        }
        
        // Try to extract and convert line numbers from error messages
        // Look for patterns like "line 123" or ":123:" or "at line 123"
        let line_pattern = regex::Regex::new(r"(?:line\s+)?(\d+)(?::|,|\s|$)").unwrap();
        result = line_pattern.replace_all(&result, |caps: &regex::Captures| {
            if let Ok(gen_line) = caps[1].parse::<u32>() {
                if let Some(source_line) = source_map.find_source_line(script_identifier, gen_line) {
                    format!("line {}", source_line)
                } else {
                    caps[0].to_string()
                }
            } else {
                caps[0].to_string()
            }
        }).to_string();
    }
    
    result
}

/// Convert a panic message using source map
pub fn convert_panic_with_source_map(
    source_map: &SourceMap,
    panic_info: &std::panic::PanicInfo,
) -> String {
    let mut result = String::new();
    
    // Get the panic message
    if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        result.push_str(s);
    } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
        result.push_str(s);
    }
    
    // Try to extract script identifier from location
    // This is a heuristic - in practice, you might need to pass the script identifier explicitly
    if let Some(location) = panic_info.location() {
        let file = location.file();
        // Try to extract script identifier from file path
        // Generated files are in .perro/scripts/src/{identifier}.rs
        if let Some(identifier) = extract_script_identifier_from_path(file) {
            result = convert_error_with_source_map(source_map, &identifier, &result);
        }
        
        // Add location info
        result.push_str(&format!("\n  at {}:{}:{}", location.file(), location.line(), location.column()));
    }
    
    result
}

/// Extract script identifier from a file path
pub fn extract_script_identifier_from_path(path: &str) -> Option<String> {
    // Look for patterns like ".perro/scripts/src/{identifier}.rs"
    let re = regex::Regex::new(r"\.perro/scripts/src/([^/]+)\.rs").ok()?;
    re.captures(path)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

