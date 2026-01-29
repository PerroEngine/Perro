// #![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod embedded_icon {
    // Include the generated embedded icon module
    // OUT_DIR is set by Cargo and points to the build output directory
    include!(concat!(env!("OUT_DIR"), "/embedded_icon.rs"));
}

use perro_core::runtime::{run_dev, run_dev_with_path};
use std::env;

fn main() {
    // Name the main thread early
    perro_core::thread_utils::set_current_thread_name("Main");

    // Check if --path argument is present - if so, run with that path directly
    // Otherwise, use run_dev() which will resolve the path from args or environment
    let args: Vec<String> = env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--path") {
        if let Some(path_arg) = args.get(i + 1) {
            // Resolve the path to an absolute PathBuf
            let project_path = if std::path::Path::new(path_arg).is_absolute() {
                std::path::PathBuf::from(path_arg)
            } else {
                match std::fs::canonicalize(path_arg) {
                    Ok(abs_path) => abs_path,
                    Err(_) => env::current_dir()
                        .expect("Failed to get current directory")
                        .join(path_arg),
                }
            };

            // Run in dev mode with the specified project path
            run_dev_with_path(project_path);
            return;
        }
    }

    // Default: use run_dev() which handles path resolution
    run_dev();
}
