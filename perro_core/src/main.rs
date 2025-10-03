use std::env;
use std::path::PathBuf;

use perro_core::asset_io::{set_project_root, ProjectRoot};
use perro_core::compiler::{Compiler, BuildProfile, CompileTarget};
use perro_core::lang::transpiler::transpile;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Decide target based on CLI args
    let target = if args.contains(&"--project".to_string()) {
        CompileTarget::Project
    } else {
        CompileTarget::Scripts
    };

    // Set project root (Disk mode, name = "unknown")
    let project_root = PathBuf::from(r"D:\Rust\perro\perro_editor");
    set_project_root(ProjectRoot::Disk {
        root: project_root.clone(),
        name: "unknown".into(),
    });

    let scripts = ["res://scripts/poop.pup", "res://scripts/editor.pup", "res://scripts/bob.pup", "res://scripts/bob.rs"];


    match target {
        CompileTarget::Scripts => {
            println!("📜 Running transpiler + compiling scripts…");


            if let Err(e) = transpile(&scripts) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            // Compile once after all transpiles
            let compiler = Compiler::new(&project_root, CompileTarget::Scripts);
            if let Err(e) = compiler.compile(BuildProfile::Dev) {
                eprintln!("❌ Script compile failed: {}", e);
                return;
            }


            println!("✅ Scripts transpiled + compiled successfully!");
        }

        CompileTarget::Project => {
            println!("📜 Running transpiler + compiling project…");

            if let Err(e) = transpile(&scripts) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            let compiler = Compiler::new(&project_root, CompileTarget::Project);
            if let Err(e) = compiler.compile(BuildProfile::Release) {
                eprintln!("❌ Project build failed: {}", e);
                return;
            }

            println!("✅ Project built successfully!");
        }
    }
}