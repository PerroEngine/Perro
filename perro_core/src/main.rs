use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use perro_core::asset_io::{ProjectRoot, set_project_root};
use perro_core::compiler::{BuildProfile, CompileTarget, Compiler};
use perro_core::transpiler::transpile;

/// Find the workspace root by looking for Cargo.toml
fn find_workspace_root() -> Option<PathBuf> {
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .or_else(|| env::current_dir().ok());

    let exe_dir = exe_dir?;

    // Step upward to workspace root if we're inside target/
    exe_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists())
        .map(|p| p.to_path_buf())
}

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

        // On Windows, paths starting with / are not valid absolute paths
        // Treat them as relative to workspace root instead
        let is_valid_absolute = {
            #[cfg(windows)]
            {
                if path_arg.starts_with('/') {
                    false // Unix-style path on Windows - treat as relative
                } else {
                    candidate.is_absolute()
                }
            }
            #[cfg(not(windows))]
            {
                candidate.is_absolute()
            }
        };

        if is_valid_absolute {
            candidate
        } else {
            // If it starts with / on Windows, treat as relative to workspace root
            let base_path: PathBuf = {
                #[cfg(windows)]
                {
                    if path_arg.starts_with('/') {
                        workspace_root.to_path_buf()
                    } else {
                        env::current_dir().expect("Failed to get current dir")
                    }
                }
                #[cfg(not(windows))]
                {
                    env::current_dir().expect("Failed to get current dir")
                }
            };

            let full_path = if path_arg.starts_with('/') {
                // Strip leading / and join to base
                base_path.join(&path_arg[1..])
            } else {
                base_path.join(&candidate)
            };

            // Try to canonicalize, but if it fails, ensure we have a proper absolute path
            // Use dunce::canonicalize which handles Windows paths better
            use dunce;
            dunce::canonicalize(&full_path).unwrap_or_else(|_| {
                // If canonicalize fails, the path might not exist yet
                // But we still need an absolute path, so ensure base_path is absolute
                if full_path.is_absolute() {
                    full_path
                } else {
                    // This shouldn't happen since base_path should be absolute, but be safe
                    env::current_dir()
                        .expect("Failed to get current dir")
                        .join(&full_path)
                        .canonicalize()
                        .unwrap_or_else(|_| {
                            env::current_dir()
                                .expect("Failed to get current dir")
                                .join(&full_path)
                        })
                }
            })
        }
    }
}

fn main() {
    // Name the main thread
    perro_core::thread_utils::set_current_thread_name("Main");
    
    let args: Vec<String> = env::args().collect();

    // Handle "new" command to create a new project
    if args.len() >= 3 && args[1] == "new" {
        let project_name = &args[2];
        let project_path = if args.len() >= 4 {
            // User provided a path - check if it's absolute or relative
            let provided_path = PathBuf::from(&args[3]);
            if provided_path.is_absolute() {
                // Absolute path - use it directly (assumes it's the final project path)
                provided_path
            } else {
                // Relative path - treat as directory to create project in
                env::current_dir()
                    .expect("Failed to get current directory")
                    .join(provided_path)
                    .join(project_name)
            }
        } else {
            // Default to workspace_root/projects/project_name
            let workspace_root = find_workspace_root()
                .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));
            let projects_dir = workspace_root.join("projects");
            // Create projects directory if it doesn't exist
            let _ = std::fs::create_dir_all(&projects_dir);
            projects_dir.join(project_name)
        };

        match perro_core::project_creator::create_new_project(project_name, &project_path, true) {
            Ok(_) => {
                println!("✅ Project created successfully!");
                
                // Set project root for script building
                set_project_root(ProjectRoot::Disk {
                    root: project_path.clone(),
                    name: project_name.clone(),
                });
                
                // Build scripts automatically so the project is ready to run
                println!("📜 Building scripts...");
                if let Err(e) = transpile(&project_path, true) {
                    eprintln!("⚠️  Warning: Failed to transpile scripts: {}", e);
                    eprintln!("   You can build scripts later with: cargo run -p perro_core -- --path {} --scripts", project_path.display());
                } else {
                    let compiler = Compiler::new(&project_path, CompileTarget::Scripts, true);
                    if let Err(e) = compiler.compile(BuildProfile::Dev) {
                        eprintln!("⚠️  Warning: Failed to compile scripts: {}", e);
                        eprintln!("   You can build scripts later with: cargo run -p perro_core -- --path {} --scripts", project_path.display());
                    } else {
                        println!("✅ Scripts built successfully!");
                    }
                }
                
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("❌ Failed to create project: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Handle --convert-flamegraph command (convert existing folded file to SVG)
    if args.contains(&"--convert-flamegraph".to_string()) {
        let path_flag_i = args.iter().position(|a| a == "--path");
        let path_arg = path_flag_i
            .and_then(|i| args.get(i + 1))
            .unwrap_or_else(|| {
                eprintln!("❌ Missing required flag: --path <path or --editor>");
                eprintln!("   Usage: cargo run -p perro_core --features profiling -- --path <path> --convert-flamegraph");
                eprintln!("   (This converts an existing flamegraph.folded file to SVG without running the game)");
                std::process::exit(1);
            });

        let project_root = resolve_project_root(path_arg);
        let folded_path = project_root.join("flamegraph.folded");
        let svg_path = project_root.join("flamegraph.svg");
        
        if !folded_path.exists() {
            eprintln!("❌ Flamegraph folded file not found at: {:?}", folded_path);
            eprintln!("   Run with --profile first to generate the folded file");
            std::process::exit(1);
        }
        
        println!("📊 Converting {:?} to {:?}...", folded_path, svg_path);
        
        #[cfg(feature = "profiling")]
        {
            perro_core::runtime::convert_flamegraph(&folded_path, &svg_path);
        }
        
        #[cfg(not(feature = "profiling"))]
        {
            eprintln!("❌ Profiling feature not enabled!");
            eprintln!("   Build with: cargo run -p perro_core --features profiling -- --path <path> --convert-flamegraph");
            std::process::exit(1);
        }
        
        return;
    }

    // Handle --run command (just run, no compilation)
    if args.contains(&"--run".to_string()) {
        let path_flag_i = args.iter().position(|a| a == "--path");
        let path_arg = path_flag_i
            .and_then(|i| args.get(i + 1))
            .unwrap_or_else(|| {
                eprintln!("❌ Missing required flag: --path <path or --editor>");
                eprintln!("   Usage: cargo run -p perro_core -- --path <path> --run");
                std::process::exit(1);
            });

        let project_root = resolve_project_root(path_arg);
        println!("📁 Using project root: {}", project_root.display());

        // Verify project.toml exists at the resolved path
        let project_toml_path = project_root.join("project.toml");
        if !project_toml_path.exists() {
            eprintln!(
                "❌ project.toml not found at: {}",
                project_toml_path.display()
            );
            eprintln!("   Resolved from input: {}", path_arg);
            eprintln!("   Project root: {}", project_root.display());
            std::process::exit(1);
        }

        println!("🚀 Running project (no compilation)…");

        // Spawn perro_dev with the same path
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "-p", "perro_dev", "--", "--path", path_arg]);
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        let status = cmd.status().expect("Failed to start perro_dev");
        std::process::exit(status.code().unwrap_or(1));
    }

    // Handle --profile command (build scripts + run with headless profiling)
    // --flamegraph is kept as an alias for backwards compatibility
    let enable_profiling = args.contains(&"--profile".to_string()) || args.contains(&"--flamegraph".to_string());
    
    if enable_profiling {
        let path_flag_i = args.iter().position(|a| a == "--path");
        let path_arg = path_flag_i
            .and_then(|i| args.get(i + 1))
            .unwrap_or_else(|| {
                eprintln!("❌ Missing required flag: --path <path or --editor>");
                eprintln!("   Usage: cargo run -p perro_core --features profiling -- --path <path> --profile");
                std::process::exit(1);
            });

        let project_root = resolve_project_root(path_arg);
        println!("📁 Using project root: {}", project_root.display());

        // Verify project.toml exists at the resolved path
        let project_toml_path = project_root.join("project.toml");
        if !project_toml_path.exists() {
            eprintln!(
                "❌ project.toml not found at: {}",
                project_toml_path.display()
            );
            eprintln!("   Resolved from input: {}", path_arg);
            eprintln!("   Project root: {}", project_root.display());
            std::process::exit(1);
        }

        // Register in engine core
        set_project_root(ProjectRoot::Disk {
            root: project_root.clone(),
            name: "Perro Engine".into(),
        });

        // Build scripts first
        println!("📜 Building scripts…");
        if let Err(e) = transpile(&project_root, true) {
            eprintln!("❌ Transpile failed: {}", e);
            std::process::exit(1);
        }

        let compiler = Compiler::new(&project_root, CompileTarget::Scripts, true);
        if let Err(e) = compiler.compile(BuildProfile::Dev) {
            eprintln!("❌ Script compile failed: {}", e);
            std::process::exit(1);
        }

        println!("✅ Scripts built! Starting dev runner with profiling…");
        println!("🔥 Profiling enabled! Flamegraph will be written to {:?}", project_root.join("flamegraph.folded"));

        // Spawn perro_dev with profiling enabled (headless mode)
        // Pass the profiling feature through to perro_dev
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "-p", "perro_dev", "--features", "profiling", "--", "--path", path_arg, "--profile"]);
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        let status = cmd.status().expect("Failed to start perro_dev");
        std::process::exit(status.code().unwrap_or(1));
    }

    // Handle --dev command (build scripts + run)
    if args.contains(&"--dev".to_string()) {
        let path_flag_i = args.iter().position(|a| a == "--path");
        let path_arg = path_flag_i
            .and_then(|i| args.get(i + 1))
            .unwrap_or_else(|| {
                eprintln!("❌ Missing required flag: --path <path or --editor>");
                eprintln!("   Usage: cargo run -p perro_core -- --path <path> --dev");
                std::process::exit(1);
            });

        let project_root = resolve_project_root(path_arg);
        println!("📁 Using project root: {}", project_root.display());

        // Verify project.toml exists at the resolved path
        let project_toml_path = project_root.join("project.toml");
        if !project_toml_path.exists() {
            eprintln!(
                "❌ project.toml not found at: {}",
                project_toml_path.display()
            );
            eprintln!("   Resolved from input: {}", path_arg);
            eprintln!("   Project root: {}", project_root.display());
            std::process::exit(1);
        }

        // Register in engine core
        set_project_root(ProjectRoot::Disk {
            root: project_root.clone(),
            name: "Perro Engine".into(),
        });

        // Build scripts first
        println!("📜 Building scripts…");
        if let Err(e) = transpile(&project_root, true) {
            eprintln!("❌ Transpile failed: {}", e);
            std::process::exit(1);
        }

        let compiler = Compiler::new(&project_root, CompileTarget::Scripts, true);
        if let Err(e) = compiler.compile(BuildProfile::Dev) {
            eprintln!("❌ Script compile failed: {}", e);
            std::process::exit(1);
        }

        println!("✅ Scripts built! Starting dev runner…");

        // Spawn perro_dev with the same path
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "-p", "perro_dev", "--", "--path", path_arg]);
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        let status = cmd.status().expect("Failed to start perro_dev");
        std::process::exit(status.code().unwrap_or(1));
    }

    // Require --path to be present for build commands
    let path_flag_i = args.iter().position(|a| a == "--path");
    let path_arg = path_flag_i
        .and_then(|i| args.get(i + 1))
        .unwrap_or_else(|| {
            eprintln!("❌ Missing required flag: --path <path or --editor>");
            eprintln!("   Or use: perro new <project_name> [path]");
            eprintln!(
                "   Or use: cargo run -p perro_core -- --path <path> --scripts (compile only)"
            );
            eprintln!("   Or use: cargo run -p perro_core -- --path <path> --dev (compile + run)");
            eprintln!("   Or use: cargo run -p perro_core -- --path <path> --run (run only)");
            std::process::exit(1);
        });

    // Decide build type
    let target = if args.contains(&"--project".to_string()) {
        // Check for --verbose flag
        if args.contains(&"--verbose".to_string()) {
            CompileTarget::VerboseProject
        } else {
            CompileTarget::Project
        }
    } else if args.contains(&"--scripts".to_string()) {
        CompileTarget::Scripts
    } else {
        // Default to scripts if no explicit target
        CompileTarget::Scripts
    };

    // Resolve the path properly
    let project_root = resolve_project_root(path_arg);
    println!("📁 Using project root: {}", project_root.display());

    // Verify project.toml exists at the resolved path
    let project_toml_path = project_root.join("project.toml");
    if !project_toml_path.exists() {
        eprintln!(
            "❌ project.toml not found at: {}",
            project_toml_path.display()
        );
        eprintln!("   Resolved from input: {}", path_arg);
        eprintln!("   Project root: {}", project_root.display());
        std::process::exit(1);
    }

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
