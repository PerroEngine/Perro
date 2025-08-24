use std::env;
use perro_core::globals::set_project_root;
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

    // Set project root (adjust path as needed)
    let project_root = r"C:\Users\super\perro\perro_editor";
    set_project_root(project_root.into());

    match target {
        CompileTarget::Scripts => {
            println!("📜 Running transpiler + compiling scripts…");

            // Example: list of script entrypoints
            let scripts = ["res://scripts/editor.pup"];

            if let Err(e) = transpile(&scripts) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            println!("✅ Scripts transpiled + compiled successfully!");
        }

        CompileTarget::Project => {
            println!("🛠️ Building project crate…");

            let compiler = Compiler::new(project_root.as_ref(), CompileTarget::Project);
            if let Err(e) = compiler.compile(BuildProfile::Release) {
                eprintln!("❌ Project build failed: {}", e);
                return;
            }

            println!("✅ Project built successfully!");
        }
    }
}