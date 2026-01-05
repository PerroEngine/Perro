#[cfg(debug_assertions)]
use std::ffi::CStr;
#[cfg(debug_assertions)]
use std::os::raw::c_char;
use perro_core::script::CreateFn;
use phf::{phf_map, Map};

pub mod scripts_manager_rs;
pub mod scripts_repair_rs;
pub mod scripts_root_rs;
pub mod scripts_updater_rs;
// __PERRO_MODULES__
use scripts_manager_rs::scripts_manager_rs_create_script;
use scripts_repair_rs::scripts_repair_rs_create_script;
use scripts_root_rs::scripts_root_rs_create_script;
use scripts_updater_rs::scripts_updater_rs_create_script;
// __PERRO_IMPORTS__

pub fn get_script_registry() -> &'static Map<&'static str, CreateFn> {
&SCRIPT_REGISTRY
}

static SCRIPT_REGISTRY: Map<&'static str, CreateFn> = phf_map! {
    "scripts_manager_rs" => scripts_manager_rs_create_script as CreateFn,
        "scripts_repair_rs" => scripts_repair_rs_create_script as CreateFn,
        "scripts_root_rs" => scripts_root_rs_create_script as CreateFn,
        "scripts_updater_rs" => scripts_updater_rs_create_script as CreateFn,
    // __PERRO_REGISTRY__
};


#[cfg(debug_assertions)]
#[unsafe(no_mangle)]
pub extern "C" fn perro_set_project_root(path: *const c_char, name: *const c_char) {
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let name_str = unsafe { CStr::from_ptr(name).to_str().unwrap() };
    perro_core::asset_io::set_project_root(
        perro_core::asset_io::ProjectRoot::Disk {
            root: std::path::PathBuf::from(path_str),
            name: name_str.to_string(),
        }
    );
    // Set up panic handler when project root is injected (this runs early)
    setup_dll_panic_handler();
}


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
