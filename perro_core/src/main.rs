use std::env;
use std::path::{Path, PathBuf};

use perro_core::asset_io::{ProjectRoot, set_project_root};
use perro_core::compiler::{BuildProfile, CompileTarget, Compiler};
use perro_core::transpiler::transpile;


/// Get the path to the *project root* using the location of the perro_core crate root.
/// This will resolve properly even when running from target/debug.
fn resolve_project_root(path_arg: &str) -> PathBuf {
    // Get the directory where cargo executed the compiled binary
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| env::current_dir().expect("Failed to get working directory"));

    // Step upward to crate workspace root if we’re inside target/
    // (e.g., d:\Rust\perro\target\debug -> d:\Rust\perro)
    let workspace_root = exe_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists())
        .unwrap_or_else(|| exe_dir.parent().unwrap_or_else(|| Path::new(".")));

    // Handle the input
    if path_arg.eq_ignore_ascii_case("--editor") {
        // Go up to workspace root, then into perro_editor
        let editor_path = workspace_root.join("perro_editor");
        editor_path.canonicalize().unwrap_or(editor_path)
    } else if path_arg.eq_ignore_ascii_case("--test") {
        let test_path = workspace_root.join("test_projects/test");
        test_path.canonicalize().unwrap_or(test_path)
    } else {
        // Treat it as path (absolute or relative to cwd)
        let candidate = PathBuf::from(path_arg);
        if candidate.is_absolute() {
            candidate
        } else {
            env::current_dir()
                .expect("Failed to get current dir")
                .join(candidate)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(path_arg))
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Require --path to be present
    let path_flag_i = args.iter().position(|a| a == "--path");
    let path_arg = path_flag_i
        .and_then(|i| args.get(i + 1))
        .unwrap_or_else(|| {
            eprintln!("❌ Missing required flag: --path <path or --editor>");
            std::process::exit(1);
        });

    // Decide build type
    let target = if args.contains(&"--project".to_string()) {
        CompileTarget::Project
    } else {
        CompileTarget::Scripts
    };

    // Resolve the path properly
    let project_root = resolve_project_root(path_arg);
    println!("📁 Using project root: {}", project_root.display());

    // Register in engine core
    set_project_root(ProjectRoot::Disk {
        root: project_root.clone(),
        name: "Perro Engine".into(),
    });

    // Do the build / script flow
    match target {
        CompileTarget::Scripts => {
            println!("📜 Transpiling scripts…");
            if let Err(e) = transpile(&project_root, true) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            let compiler = Compiler::new(&project_root, CompileTarget::Scripts, true);
            if let Err(e) = compiler.compile(BuildProfile::Dev) {
                eprintln!("❌ Script compile failed: {}", e);
                return;
            }

            println!("✅ Scripts ready!");
        }
        CompileTarget::Project => {
            println!("🏗️  Building project…");
            if let Err(e) = transpile(&project_root, false) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            let compiler = Compiler::new(&project_root, CompileTarget::Project, true);
            if let Err(e) = compiler.compile(BuildProfile::Release) {
                eprintln!("❌ Project build failed: {}", e);
                return;
            }

            println!("✅ Project built!");
        }
        CompileTarget::VerboseProject => {
            println!("🏗️  Building verbose project…");
            if let Err(e) = transpile(&project_root, true) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            let compiler = Compiler::new(&project_root, CompileTarget::VerboseProject, true);
            if let Err(e) = compiler.compile(BuildProfile::Release) {
                eprintln!("❌ Project build failed: {}", e);
                return;
            }

            println!("✅ Project built!");
        }
    }
}
