use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use walkdir::WalkDir;
use sha2::{Sha256, Digest};

use crate::{
    codegen::derive_rust_perro_script,
    lang::{
        csharp::parser::CsParser, pup::parser::PupParser, typescript::parser::TypeScriptParser,
    },
    scripting::source_map::{SourceMap, build_source_map_from_script},
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

/// Embed source map into generated Rust file as a static constant

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

/// Compute a hash of all script files in res/ directory
/// Files are sorted alphabetically for deterministic hashing
pub fn compute_script_hash(project_root: &Path) -> Result<String, String> {
    let script_paths = discover_scripts(project_root)?;
    
    // Sort paths alphabetically for deterministic ordering
    let mut sorted_paths: Vec<_> = script_paths.iter().collect();
    sorted_paths.sort_by(|a, b| {
        a.to_string_lossy().cmp(&b.to_string_lossy())
    });
    
    let mut hasher = Sha256::new();

    for path in sorted_paths {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        
        // Hash the file path and content
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(b"\0"); // Separator
        hasher.update(content.as_bytes());
        hasher.update(b"\0"); // Separator
    }
    
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// Read the stored script hash from builds directory
pub fn read_stored_hash(project_root: &Path) -> Result<Option<String>, String> {
    let hash_file = project_root.join(".perro/scripts/builds/scripts_hash");
    
    if !hash_file.exists() {
        return Ok(None);
    }
    
    match fs::read_to_string(&hash_file) {
        Ok(content) => Ok(Some(content.trim().to_string())),
        Err(e) => Err(format!("Failed to read script hash file: {}", e)),
    }
}

/// Write the script hash to builds directory
pub fn write_script_hash(project_root: &Path, hash: &str) -> Result<(), String> {
    let builds_dir = project_root.join(".perro/scripts/builds");
    fs::create_dir_all(&builds_dir)
        .map_err(|e| format!("Failed to create builds directory: {}", e))?;
    
    let hash_file = builds_dir.join("scripts_hash");
    fs::write(&hash_file, hash)
        .map_err(|e| format!("Failed to write script hash file: {}", e))?;
    
    Ok(())
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

    rebuild_lib_rs(project_root, &active_ids, &std::collections::HashSet::new())?;
    Ok(())
}
pub fn rebuild_lib_rs(project_root: &Path, active_ids: &HashSet<String>, module_identifiers: &HashSet<String>) -> Result<(), String> {
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

    // Separate scripts from modules
    // Modules don't have _create_script functions, so we need to identify them
    // For now, we'll check if the identifier corresponds to a module by checking if it's in module_names
    // But we need to map from file identifier to module name... 
    // Actually, modules are in active_ids but don't have _create_script, so we can detect them that way
    // Or better: track which identifiers are modules vs scripts
    
    // Sort IDs for deterministic ordering
    let mut sorted_ids: Vec<_> = active_ids.iter().collect();
    sorted_ids.sort();

    // Add all modules (they're just pub mod declarations, no imports/registry)
    for id in &sorted_ids {
        content = content.replace(
            "// __PERRO_MODULES__",
            &format!("pub mod {};\n// __PERRO_MODULES__", id),
        );
    }

    // Only add imports and registry entries for scripts (not modules)
    // Modules don't have _create_script functions
    for id in &sorted_ids {
        // Check if this identifier is a module (modules don't have _create_script functions)
        // id is &&String from iterator, convert to &str for HashSet lookup
        let is_module = module_identifiers.contains((*id).as_str());
        
        if !is_module {
            // Only add imports and registry for scripts
            content = content.replace(
                "// __PERRO_IMPORTS__",
                &format!("use {}::{}_create_script;\n// __PERRO_IMPORTS__", id, id),
            );
            
            content = content.replace(
                "// __PERRO_REGISTRY__",
                &format!(
                    "    \"{}\" => {}_create_script as CreateFn,\n    // __PERRO_REGISTRY__",
                    id, id
                ),
            );
        }
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
    // Set up panic handler when project root is injected (this runs early)
    setup_dll_panic_handler();
}}
"#,
        ffi_fn_marker
    );

    content.push_str(&debug_root_fn);

    // Add panic handler for DLL panics with source map support
    // This is critical because panics in DLLs don't trigger the main binary's panic hook
    // Only include in debug builds to reduce release binary size
    let panic_handler = r#"

#[cfg(debug_assertions)]
mod panic_handler {
// Panic handler for script DLL - handles panics that occur in the DLL
// Panics in DLLs don't propagate to the main binary's panic hook, so we need our own

fn get_project_root_from_dll() -> Option<std::path::PathBuf> {
    // Try to get project root from perro_core's global state
    // get_project_root() panics if not set, so we need to catch that
    match std::panic::catch_unwind(|| perro_core::asset_io::get_project_root()) {
        Ok(root) => {
            if let perro_core::asset_io::ProjectRoot::Disk { root, .. } = root {
                return Some(root);
            }
        }
        Err(_) => {
            // Project root not set yet, try fallback
        }
    }
    
    // Fallback: try to infer from DLL location
    // DLL is typically at: <project>/.perro/scripts/builds/scripts.dll
    // We want: <project>
    if let Ok(exe_path) = std::env::current_exe() {
        // This won't work for DLLs, but try anyway
        if let Some(parent) = exe_path.parent() {
            // Look for .perro directory going up
            for ancestor in parent.ancestors() {
                if ancestor.join(".perro").exists() {
                    return Some(ancestor.to_path_buf());
                }
            }
        }
    }
    None
}

fn handle_dll_panic(panic_info: &std::panic::PanicHookInfo) {
    // Get panic message
    let mut panic_msg = String::new();
    if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        panic_msg = s.to_string();
    } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
        panic_msg = s.clone();
    }
    
    // Get location and try to convert using source map
    let mut source_file = None;
    let mut source_line = 0u32;
    
    if let Some(location) = panic_info.location() {
        let file_path = location.file();
        let generated_line = location.line();
        
        // Try to load source map and convert
        if let Some(project_root) = get_project_root_from_dll() {
            let source_map_path = project_root.join(".perro/scripts/sourcemap.toml");
            if let Ok(content) = std::fs::read_to_string(&source_map_path) {
                if let Ok(sm) = toml::from_str::<perro_core::scripting::source_map::SourceMap>(&content) {
                    // Extract script identifier from path
                    let normalized = file_path.replace('\\', "/");
                    let patterns = [
                        r"\.perro/scripts/src/([^/]+)\.rs",
                        r"scripts/src/([^/]+)\.rs",
                        r"src/([^/]+)\.rs",
                    ];
                    
                    for pattern in &patterns {
                        if let Ok(re) = regex::Regex::new(pattern) {
                            if let Some(caps) = re.captures(&normalized) {
                                if let Some(m) = caps.get(1) {
                                    let identifier = m.as_str();
                                    
                                    // Convert error message
                                    panic_msg = perro_core::scripting::source_map_runtime::convert_error_with_source_map(&sm, identifier, &panic_msg);
                                    
                                    // Try to find source file and convert line number
                                    if let Some(script_map) = sm.scripts.get(identifier) {
                                        // Replace identifier.pup patterns with source path (keep extension)
                                        let identifier_pattern = format!(r"\b{}\.pup\b", regex::escape(identifier));
                                        if let Ok(re) = regex::Regex::new(&identifier_pattern) {
                                            let source_filename = script_map.source_path.split('/').last().unwrap_or(&script_map.source_path);
                                            panic_msg = re.replace_all(&panic_msg, source_filename).to_string();
                                        }
                                        
                                        // Extract filename from source path (e.g., "res://player.pup" -> "player.pup")
                                        // Keep the file extension
                                        source_file = Some(script_map.source_path.split('/').last().unwrap_or(&script_map.source_path).to_string());
                                        
                                        // Try to extract line number from panic message if it contains "at line X:"
                                        // Format: "player.pup at line X: error message"
                                        if let Some(line_match) = regex::Regex::new(r" at line (\d+):").ok()
                                            .and_then(|re| re.captures(&panic_msg))
                                            .and_then(|caps| caps.get(1))
                                            .and_then(|m| m.as_str().parse::<u32>().ok()) {
                                            source_line = line_match;
                                        } else if let Some(line) = sm.find_source_line(identifier, generated_line) {
                                            source_line = line;
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Print simple error message in red: "[Panic] player.pup at line 0: error message"
    let red = "\x1b[31m";
    let reset = "\x1b[0m";
    
    if let Some(file) = source_file {
        eprintln!("{}[Panic] {} at line {}: {}{}", red, file, source_line, panic_msg, reset);
    } else {
        // Fallback if we couldn't find source file
        eprintln!("{}[Panic] {}{}", red, panic_msg, reset);
    }
}

// Set panic hook - use Once to ensure it's only set once
use std::sync::Once;
static PANIC_HOOK_SETUP: Once = Once::new();

pub fn setup_dll_panic_handler() {
    PANIC_HOOK_SETUP.call_once(|| {
        std::panic::set_hook(Box::new(|panic_info| {
            handle_dll_panic(panic_info);
        }));
    });
}
}

#[cfg(debug_assertions)]
use panic_handler::setup_dll_panic_handler;

#[cfg(not(debug_assertions))]
fn setup_dll_panic_handler() {
    // No-op in release builds - panic handler not needed
}
"#;

    content.push_str(panic_handler);

    fs::write(&lib_rs_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Transpile all scripts in res/. When `project_mode` is true, generated module code
/// gets `#[inline]` on functions and constants (for release/project builds).
pub fn transpile(project_root: &Path, verbose: bool, project_mode: bool) -> Result<(), String> {
    let total_start = Instant::now();

    // In project mode, skip hash compare so we always transpile
    if !project_mode {
        let current_hash = compute_script_hash(project_root)?;
        let stored_hash = read_stored_hash(project_root)?;
        if let Some(stored) = stored_hash {
            if stored == current_hash {
                println!("✅ Script hash unchanged, skipping transpilation");
                return Ok(());
            }
        }
    }

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
        rebuild_lib_rs(project_root, &std::collections::HashSet::new(), &std::collections::HashSet::new())?;
        if !project_mode {
            let current_hash = compute_script_hash(project_root)?;
            write_script_hash(project_root, &current_hash)?;
        }
        return Ok(());
    }

    println!("📜 Found {} script(s)", script_paths.len());

    // For summarizing timings at the end
    struct Timing {
        #[allow(dead_code)]
        path: String,
        io_time: Duration,
        parse_time: Duration,
        transpile_time: Duration,
        #[allow(dead_code)]
        total_time: Duration,
    }

    // ============================================================
    // FIRST PASS: Collect all module names
    // This ensures we know about all modules before generating any code
    // This is necessary because:
    // 1. Scripts might reference modules that are processed later
    // 2. Modules might reference other modules
    // ============================================================
    let mut module_names: HashSet<String> = HashSet::new(); // Track module names (e.g., "Utils")
    let mut module_identifiers: HashSet<String> = HashSet::new(); // Track which file identifiers are modules
    let mut identifier_to_module_name: std::collections::HashMap<String, String> = std::collections::HashMap::new(); // Map identifier -> module name
    let mut module_name_to_identifier: std::collections::HashMap<String, String> = std::collections::HashMap::new(); // Map module name -> identifier (reverse map)
    let mut module_functions: std::collections::HashMap<String, Vec<crate::scripting::ast::Function>> = std::collections::HashMap::new(); // Map module name -> functions
    let mut module_variables: std::collections::HashMap<String, Vec<crate::scripting::ast::Variable>> = std::collections::HashMap::new(); // Map module name -> variables (constants)
    
    for path in &script_paths {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Failed to extract file extension".to_string())?;
        
        if extension == "pup" {
            let code = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            
            // Check if it's a module
            if code.trim_start().starts_with("@module") {
                let mut parser = PupParser::new(&code);
                if let Ok(module) = parser.parse_module() {
                    let identifier = script_path_to_identifier(&path.to_string_lossy())?;
                    module_names.insert(module.module_name.clone());
                    module_identifiers.insert(identifier.clone());
                    identifier_to_module_name.insert(identifier.clone(), module.module_name.clone());
                    module_name_to_identifier.insert(module.module_name.clone(), identifier.clone());
                    // Store module functions for type inference
                    module_functions.insert(module.module_name.clone(), module.functions.clone());
                    // Store module variables (constants) for type inference
                    module_variables.insert(module.module_name.clone(), module.variables.clone());
                }
            }
        }
    }
    
    println!("📦 Found {} module(s): {:?}", module_names.len(), module_names.iter().collect::<Vec<_>>());

    // ============================================================
    // SECOND PASS: Generate code for all files with full module knowledge
    // ============================================================
    let mut timings: Vec<Timing> = Vec::new();
    let mut source_map = SourceMap::new();
    let mut active_ids: HashSet<String> = HashSet::new(); // Track all identifiers (scripts + modules)

    for path in &script_paths {
        let script_total_start = Instant::now();
        let identifier = script_path_to_identifier(&path.to_string_lossy())?;
        active_ids.insert(identifier.clone()); // Track this identifier

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
                // Convert absolute path to res:// relative path for source location tracking
                let res_path = {
                    let res_dir = project_root.join("res");
                    if let Ok(relative) = path.strip_prefix(&res_dir) {
                        // Remove leading slash if present
                        let relative_str = relative.to_string_lossy().replace('\\', "/");
                        let clean_relative = relative_str.strip_prefix('/').unwrap_or(&relative_str);
                        format!("res://{}", clean_relative)
                    } else {
                        // Fallback: try to extract from path string
                        let path_str = path.to_string_lossy().replace('\\', "/");
                        if let Some(res_idx) = path_str.find("/res/") {
                            let after_res = &path_str[res_idx + 6..]; // Skip "/res/"
                            format!("res://{}", after_res)
                        } else {
                            // Last resort: use filename only
                            path.file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| format!("res://{}", n))
                                .unwrap_or_else(|| path.to_string_lossy().to_string())
                        }
                    }
                };
                
                let mut parser = PupParser::new(&code);
                parser.set_source_file(res_path.clone());
                
                // Try to parse as module first (check for @module)
                let is_module = code.trim_start().starts_with("@module");
                
                if is_module {
                    let mut module = parser.parse_module()?;
                    module.source_file = Some(res_path.clone());
                    // Module names were already collected in first pass, but ensure identifier is tracked
                    // (should already be in module_identifiers from first pass, but be safe)
                    if !module_identifiers.contains(&identifier) {
                        module_identifiers.insert(identifier.clone());
                        identifier_to_module_name.insert(identifier.clone(), module.module_name.clone());
                        module_name_to_identifier.insert(module.module_name.clone(), identifier.clone());
                    }
                    parse_time = parse_start.elapsed();

                    transpile_start = Instant::now();
                    // Use the identifier from the path (same as scripts)
                    // The module name is used internally but file-based identifier is used for file naming
                    // Pass module_names so modules can reference other modules
                    let _generated_code = module.to_rust(&identifier, project_root, verbose, &module_names, project_mode);
                    transpile_time = transpile_start.elapsed();
                    
                    // Modules don't need source maps for now (they're simple)
                    // The identifier is already tracked via script_paths -> active_ids
                } else {
                    let mut script = parser.parse_script()?;
                    script.source_file = Some(res_path.clone());
                    parse_time = parse_start.elapsed();

                    transpile_start = Instant::now();
                    // Pass module names and mapping to script codegen so it knows which identifiers are modules
                    let generated_code = script.to_rust(&identifier, project_root, None, verbose, &module_names, &module_name_to_identifier, &module_functions, &module_variables);
                    transpile_time = transpile_start.elapsed();
                    
                    // Build source map and embed it in the generated code
                    let script_source_map = build_source_map_from_script(
                        &res_path,
                        &identifier,
                        &code,
                        &generated_code,
                        &script,
                    );
                    source_map.add_script(identifier.clone(), script_source_map.clone());
                }
            }
            "cs" => {
                let mut script = CsParser::new(&code).parse_script()?;
                parse_time = parse_start.elapsed();

                transpile_start = Instant::now();
                let generated_code = script.to_rust(&identifier, project_root, None, verbose, &module_names, &module_name_to_identifier, &module_functions, &module_variables);
                transpile_time = transpile_start.elapsed();
                
                // Convert absolute path to res:// relative path
                let res_path = {
                    let res_dir = project_root.join("res");
                    if let Ok(relative) = path.strip_prefix(&res_dir) {
                        // Remove leading slash if present
                        let relative_str = relative.to_string_lossy().replace('\\', "/");
                        let clean_relative = relative_str.strip_prefix('/').unwrap_or(&relative_str);
                        format!("res://{}", clean_relative)
                    } else {
                        // Fallback: try to extract from path string
                        let path_str = path.to_string_lossy().replace('\\', "/");
                        if let Some(res_idx) = path_str.find("/res/") {
                            let after_res = &path_str[res_idx + 6..]; // Skip "/res/"
                            format!("res://{}", after_res)
                        } else {
                            // Last resort: use filename only
                            path.file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| format!("res://{}", n))
                                .unwrap_or_else(|| path.to_string_lossy().to_string())
                        }
                    }
                };
                
                // Build source map and embed it in the generated code
                let script_source_map = build_source_map_from_script(
                    &res_path,
                    &identifier,
                    &code,
                    &generated_code,
                    &script,
                );
                source_map.add_script(identifier.clone(), script_source_map.clone());
            }
            "ts" => {
                let mut script = TypeScriptParser::new(&code).parse_script()?;
                parse_time = parse_start.elapsed();

                transpile_start = Instant::now();
                let generated_code = script.to_rust(&identifier, project_root, None, verbose, &module_names, &module_name_to_identifier, &module_functions, &module_variables);
                transpile_time = transpile_start.elapsed();
                
                // Convert absolute path to res:// relative path
                let res_path = {
                    let res_dir = project_root.join("res");
                    if let Ok(relative) = path.strip_prefix(&res_dir) {
                        // Remove leading slash if present
                        let relative_str = relative.to_string_lossy().replace('\\', "/");
                        let clean_relative = relative_str.strip_prefix('/').unwrap_or(&relative_str);
                        format!("res://{}", clean_relative)
                    } else {
                        // Fallback: try to extract from path string
                        let path_str = path.to_string_lossy().replace('\\', "/");
                        if let Some(res_idx) = path_str.find("/res/") {
                            let after_res = &path_str[res_idx + 6..]; // Skip "/res/"
                            format!("res://{}", after_res)
                        } else {
                            // Last resort: use filename only
                            path.file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| format!("res://{}", n))
                                .unwrap_or_else(|| path.to_string_lossy().to_string())
                        }
                    }
                };
                
                // Build source map and embed it in the generated code
                let script_source_map = build_source_map_from_script(
                    &res_path,
                    &identifier,
                    &code,
                    &generated_code,
                    &script,
                );
                source_map.add_script(identifier.clone(), script_source_map.clone());
            }
            "rs" => {
                parse_time = Duration::ZERO;
                transpile_start = Instant::now();
                derive_rust_perro_script(project_root, &code, &identifier, verbose)?;
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

    // Write source map to file
    let source_map_path = project_root.join(".perro/scripts/sourcemap.toml");
    if let Ok(toml_str) = toml::to_string(&source_map) {
        if let Err(e) = fs::write(&source_map_path, toml_str) {
            eprintln!("⚠️  Warning: Failed to write source map: {}", e);
        } else {
            println!("📝 Source map written to: {}", source_map_path.display());
        }
    } else {
        eprintln!("⚠️  Warning: Failed to serialize source map");
    }

    // Rebuild lib.rs with all active identifiers and module information
    rebuild_lib_rs(project_root, &active_ids, &module_identifiers)?;

    // Note: Hash is written after successful compilation, not here
    // This allows the compile step to check if recompilation is needed

    Ok(())
}
