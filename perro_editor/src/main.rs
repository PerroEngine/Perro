#[cfg(debug_assertions)]
use std::path::Path;
use std::process::Command;
use std::path::PathBuf;

fn main() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    run_game(&project_root);
}

#[cfg(debug_assertions)]
fn run_game(project_root: &Path) {
    // Dev mode: use cargo run
    let status = Command::new("cargo")
        .args(&["run", "-p", "perro_game", "--", "--path"])
        .arg(project_root)
        .status()
        .expect("Failed to run perro_game");
    if !status.success() {
        eprintln!("❌ Failed to run perro_game");
    }
}

#[cfg(not(debug_assertions))]
fn run_game(project_root: &Path) {
    // Dist mode: run prebuilt game.exe
    let game_exe = std::env::current_exe()
        .unwrap()
        .parent().unwrap() // folder with editor.exe
        .join("game.exe");
    let status = Command::new(game_exe)
        .arg("--path")
        .arg(project_root)
        .status()
        .expect("Failed to run game.exe");
    if !status.success() {
        eprintln!("❌ Failed to run game.exe");
    }
}