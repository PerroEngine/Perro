#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use perro_core::runtime::run_dev;

fn main() {
    // Name the main thread early
    perro_core::thread_utils::set_current_thread_name("Main");
    run_dev();
}
