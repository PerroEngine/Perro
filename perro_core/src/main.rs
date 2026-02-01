use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dirs;
use perro_core::asset_io::{ProjectRoot, set_project_root};
use perro_core::compiler::{BuildProfile, CompileTarget, Compiler};
use perro_core::runtime::run_dev_with_path;
use perro_core::transpiler::transpile;

/// Expand shorthands like "desktop" and "documents" to the user's Desktop/Documents path.
/// Case-insensitive. Returns the path as a string for use with resolve_project_root.
fn expand_path_shorthand_str(s: &str) -> String {
    match s.to_lowercase().as_str() {
        "desktop" => dirs::desktop_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| s.to_string()),
        "documents" => dirs::document_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| s.to_string()),
        _ => s.to_string(),
    }
}

/// Expand shorthands for an output path (returns PathBuf).
fn expand_path_shorthand(s: &str) -> PathBuf {
    match s.to_lowercase().as_str() {
        "desktop" => dirs::desktop_dir().unwrap_or_else(|| PathBuf::from(s)),
        "documents" => dirs::document_dir().unwrap_or_else(|| PathBuf::from(s)),
        _ => PathBuf::from(s),
    }
}

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

        match perro_core::project_creator::create_new_project(
            project_name,
            &project_path,
            true,
            false,
        ) {
            Ok(_) => {
                println!("✅ Project created successfully!");

                // Set project root for script building
                set_project_root(ProjectRoot::Disk {
                    root: project_path.clone(),
                    name: project_name.clone(),
                });

                // Build scripts automatically so the project is ready to run
                println!("📜 Building scripts...");
                if let Err(e) = transpile(&project_path, true, false, true) {
                    eprintln!("⚠️  Warning: Failed to transpile scripts: {}", e);
                    eprintln!(
                        "   You can build scripts later with: cargo run -p perro_core -- --path {} --scripts",
                        project_path.display()
                    );
                } else {
                    let compiler = Compiler::new(&project_path, CompileTarget::Scripts, true);
                    if let Err(e) = compiler.compile(BuildProfile::Dev) {
                        eprintln!("⚠️  Warning: Failed to compile scripts: {}", e);
                        eprintln!(
                            "   You can build scripts later with: cargo run -p perro_core -- --path {} --scripts",
                            project_path.display()
                        );
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

    // Handle list-meshes: print mesh indices (and internal names) for use with mesh_path res://file.glb:0, :1, …
    if args.contains(&"list-meshes".to_string()) {
        let path_flag_i = args.iter().position(|a| a == "--path");
        let path_arg = path_flag_i.and_then(|i| args.get(i + 1)).unwrap_or_else(|| {
            eprintln!("❌ list-meshes requires: --path <project> list-meshes <res://file.glb>");
            std::process::exit(1);
        });
        let list_meshes_i = args.iter().position(|a| a == "list-meshes").unwrap();
        let mesh_path_arg = args.get(list_meshes_i + 1).unwrap_or_else(|| {
            eprintln!("❌ list-meshes requires a path: --path <project> list-meshes res://file.glb");
            std::process::exit(1);
        });
        let project_root = resolve_project_root(path_arg);
        let res_root = project_root.join("res");
        let file_path = if let Some(stripped) = mesh_path_arg.strip_prefix("res://") {
            res_root.join(stripped.replace('\\', "/"))
        } else {
            PathBuf::from(mesh_path_arg)
        };
        let bytes = match std::fs::read(&file_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("❌ Failed to read {}: {}", file_path.display(), e);
                std::process::exit(1);
            }
        };
        match perro_core::rendering::mesh_loader::list_gltf_mesh_names(&bytes) {
            Some(names) => {
                let base = mesh_path_arg.trim_start_matches("res://");
                println!("Meshes in {} (use mesh_path: res://file.glb for single mesh, or res://file.glb:0, :1, … for multiple):", file_path.display());
                for (i, name) in names {
                    println!("  {}  (internal name: \"{}\")  -> mesh_path: \"res://{}:{}\"", i, name, base, i);
                }
            }
            None => {
                eprintln!("❌ No meshes found or invalid GLB/GLTF: {}", file_path.display());
                std::process::exit(1);
            }
        }
        std::process::exit(0);
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
            eprintln!(
                "   Build with: cargo run -p perro_core --features profiling -- --path <path> --convert-flamegraph"
            );
            std::process::exit(1);
        }
    }

    // Handle --run command (spawn cargo run -p perro_dev, no transpiling)
    if args.contains(&"--run".to_string()) || args.contains(&"-run".to_string()) {
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

        let project_toml_path = project_root.join("project.toml");
        if !project_toml_path.exists() {
            eprintln!(
                "❌ project.toml not found at: {}",
                project_toml_path.display()
            );
            eprintln!("   Resolved from input: {}", path_arg);
            std::process::exit(1);
        }

        println!("🚀 Spawning perro_dev (cargo run -p perro_dev --release)…");

        let cargo_bin = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let workspace_root = find_workspace_root()
            .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

        let mut cmd = Command::new(&cargo_bin);
        cmd.current_dir(&workspace_root)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .arg("run")
            .arg("-p")
            .arg("perro_dev")
            .arg("--release")
            .arg("--")
            .arg("--path")
            .arg(project_root.to_string_lossy().as_ref());

        if path_arg == "--editor" {
            if let Some(project_path) = path_flag_i.and_then(|i| args.get(i + 2)) {
                if !project_path.starts_with("--") {
                    cmd.arg("--editor").arg(project_path);
                }
            }
        }

        let status = cmd.status().expect("Failed to spawn perro_dev");
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        std::process::exit(0);
    }

    // Handle --profile command (build scripts + run with headless profiling)
    // --flamegraph is kept as an alias for backwards compatibility
    let enable_profiling =
        args.contains(&"--profile".to_string()) || args.contains(&"--flamegraph".to_string());

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
        if let Err(e) = transpile(&project_root, true, false, true) {
            eprintln!("❌ Transpile failed: {}", e);
            std::process::exit(1);
        }

        let compiler = Compiler::new(&project_root, CompileTarget::Scripts, true);
        if let Err(e) = compiler.compile(BuildProfile::Dev) {
            eprintln!("❌ Script compile failed: {}", e);
            std::process::exit(1);
        }

        println!("✅ Scripts built! Starting dev runner with profiling…");
        println!(
            "🔥 Profiling enabled! Flamegraph will be written to {:?}",
            project_root.join("flamegraph.folded")
        );

        // Run the project in dev mode with profiling using run_dev_with_path
        // The profiling feature is already enabled in this build
        run_dev_with_path(project_root);
    }

    // Handle --dev command (build scripts, then spawn cargo run -p perro_dev)
    if args.contains(&"--dev".to_string()) || args.contains(&"-dev".to_string()) {
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

        let project_toml_path = project_root.join("project.toml");
        if !project_toml_path.exists() {
            eprintln!(
                "❌ project.toml not found at: {}",
                project_toml_path.display()
            );
            eprintln!("   Resolved from input: {}", path_arg);
            std::process::exit(1);
        }

        set_project_root(ProjectRoot::Disk {
            root: project_root.clone(),
            name: "Perro Engine".into(),
        });

        println!("📜 Building scripts…");
        if let Err(e) = transpile(&project_root, true, false, true) {
            eprintln!("❌ Transpile failed: {}", e);
            std::process::exit(1);
        }

        let compiler = Compiler::new(&project_root, CompileTarget::Scripts, true);
        if let Err(e) = compiler.compile(BuildProfile::Dev) {
            eprintln!("❌ Script compile failed: {}", e);
            std::process::exit(1);
        }

        println!("✅ Scripts built! Spawning perro_dev (cargo run -p perro_dev --release)…");

        let cargo_bin = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let workspace_root = find_workspace_root()
            .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

        let mut cmd = Command::new(&cargo_bin);
        cmd.current_dir(&workspace_root)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .arg("run")
            .arg("-p")
            .arg("perro_dev")
            .arg("--release")
            .arg("--")
            .arg("--path")
            .arg(project_root.to_string_lossy().as_ref());

        if path_arg == "--editor" {
            if let Some(project_path) = path_flag_i.and_then(|i| args.get(i + 2)) {
                if !project_path.starts_with("--") {
                    cmd.arg("--editor").arg(project_path);
                }
            }
        }

        let status = cmd.status().expect("Failed to spawn perro_dev");
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        std::process::exit(0);
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
            eprintln!("   Or use: cargo run -p perro_core -- --path <path> --project [--out <dir>] (export)");
            std::process::exit(1);
        });

    // Decide build type
    let target = if args.contains(&"--project".to_string()) || args.contains(&"-project".to_string()) {
        // Check for --verbose flag
        if args.contains(&"--verbose".to_string()) || args.contains(&"-verbose".to_string()) {
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

    // Optional output path for project export (copy .app or executable to this directory)
    let out_path = args
        .iter()
        .position(|a| a == "--out" || a == "-out")
        .and_then(|i| args.get(i + 1))
        .filter(|s| !s.starts_with("-"))
        .map(|s| expand_path_shorthand(s));

    // Resolve the path properly (expand shorthands: desktop, documents)
    let path_to_resolve = expand_path_shorthand_str(path_arg);
    let project_root = resolve_project_root(&path_to_resolve);
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
            if let Err(e) = transpile(&project_root, true, false, true) {
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
            if let Err(e) = transpile(&project_root, false, true, true) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            let compiler = match &out_path {
                Some(out) => Compiler::new(&project_root, CompileTarget::Project, true).with_output_path(out),
                None => Compiler::new(&project_root, CompileTarget::Project, true),
            };
            if let Err(e) = compiler.compile(BuildProfile::Release) {
                eprintln!("❌ Project build failed: {}", e);
                return;
            }

            println!("✅ Project built!");
        }
        CompileTarget::VerboseProject => {
            println!("🏗️  Building verbose project…");
            if let Err(e) = transpile(&project_root, true, true, true) {
                eprintln!("❌ Transpile failed: {}", e);
                return;
            }

            let compiler = match &out_path {
                Some(out) => Compiler::new(&project_root, CompileTarget::VerboseProject, true).with_output_path(out),
                None => Compiler::new(&project_root, CompileTarget::VerboseProject, true),
            };
            if let Err(e) = compiler.compile(BuildProfile::Release) {
                eprintln!("❌ Project build failed: {}", e);
                return;
            }

            println!("✅ Project built!");
        }
    }
}
