#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use perro_core::runtime::run_dev;

fn main() {
    run_dev();
}