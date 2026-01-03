// #![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod embedded_icon {
    // Include the generated embedded icon module
    // OUT_DIR is set by Cargo and points to the build output directory
    include!(concat!(env!("OUT_DIR"), "/embedded_icon.rs"));
}

use perro_core::runtime::run_dev;

fn main() {
    // Name the main thread early
    perro_core::thread_utils::set_current_thread_name("Main");
    run_dev();
}
