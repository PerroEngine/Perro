use std::collections::{HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::RngCore;
use rand::seq::SliceRandom;

use crate::apply_fur::parse_fur_file;
use crate::asset_io::{ResolvedPath, resolve_path};

use crate::SceneData;
use crate::brk::build_brk;
use crate::fur_ast::{FurElement, FurNode};
use crate::node_registry::BaseNode;
use crate::structs::engine_registry::ENGINE_REGISTRY;
use image::GenericImageView;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub enum BuildProfile {
    Dev,
    Release,
    Check, // just validate
}

pub enum CompileTarget {
    Scripts, //.perro/scripts
    Project, // .perro/project
    VerboseProject,
}

#[derive(Debug, Clone)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        }
    }

    pub fn toolchain_name(&self, version: &str) -> String {
        match self {
            Platform::Windows => format!("rust-{}-x86_64-pc-windows-gnu", version),
            Platform::MacOS => format!("rust-{}-x86_64-apple-darwin", version),
            Platform::Linux => format!("rust-{}-x86_64-unknown-linux-gnu", version),
        }
    }

    pub fn cargo_exe(&self) -> &'static str {
        match self {
            Platform::Windows => "cargo.exe",
            Platform::MacOS | Platform::Linux => "cargo",
        }
    }
}

pub fn script_dylib_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "scripts.dll"
    } else if cfg!(target_os = "macos") {
        // Final copied name in .perro/scripts/builds should be "scripts.dylib" (no lib prefix)
        "scripts.dylib"
    } else {
        // Final copied name in .perro/scripts/builds should be "scripts.so" (no lib prefix)
        "scripts.so"
    }
}

/// Zstd compression levels for the asset pipeline (1–22; higher = smaller output, slower build).
/// Decompress speed at runtime is barely affected by level.
pub const ASSET_COMPRESSION: AssetCompression = AssetCompression {
    texture: 10,
    mesh: 10,
};

#[derive(Debug, Clone, Copy)]
pub struct AssetCompression {
    pub texture: i32,
    pub mesh: i32,
}

/// Derive a valid Rust crate name from a project display name (e.g. "Perro Test Project" → "perro_test_project").
fn project_name_to_crate_name(name: &str) -> String {
    let s = name.trim();
    let mut out = String::with_capacity(s.len());
    let mut prev_underscore = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_lowercase().next().unwrap_or(c));
            prev_underscore = false;
        } else if c == ' ' || c == '-' || c == '_' {
            if !prev_underscore {
                out.push('_');
                prev_underscore = true;
            }
        }
    }
    // Trim leading/trailing underscores
    let out = out.trim_matches('_').to_string();
    // Must be valid Rust identifier: start with letter or underscore, rest alphanumeric or underscore
    if out.is_empty() {
        return "perro_game".to_string();
    }
    let mut chars = out.chars();
    let first = chars.next().unwrap();
    if first.is_ascii_digit() {
        return format!("game_{}", out);
    }
    if !first.is_ascii_alphabetic() && first != '_' {
        return format!("game_{}", out);
    }
    out
}

pub struct Compiler {
    pub crate_manifest_path: PathBuf,
    target: CompileTarget,
    toolchain_root: Option<PathBuf>,
    platform: Platform,
    toolchain_version: Option<String>,
    project_root: PathBuf,
    from_source: bool,
    /// When set, copy the built artifact (.app or executable) to this directory after build.
    output_path: Option<PathBuf>,
}

impl Compiler {
    pub fn new(project_root: &Path, target: CompileTarget, from_source: bool) -> Self {
        let manifest = match target {
            CompileTarget::Scripts => project_root.join(".perro/scripts/Cargo.toml"),
            CompileTarget::Project | CompileTarget::VerboseProject => {
                project_root.join(".perro/project/Cargo.toml")
            }
        };

        let manifest = dunce::canonicalize(&manifest).unwrap_or(manifest);

        let mut compiler = Self {
            crate_manifest_path: manifest,
            target,
            toolchain_root: None,
            platform: Platform::current(),
            toolchain_version: None,
            project_root: project_root.to_path_buf(),
            from_source,
            output_path: None,
        };

        compiler.load_toolchain_config();
        compiler
    }

    /// Copy the built artifact (.app or executable) to this directory after a successful project build.
    pub fn with_output_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.output_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_toolchain_root<P: AsRef<Path>>(mut self, toolchain_root: P) -> Self {
        self.toolchain_root = Some(toolchain_root.as_ref().to_path_buf());
        self
    }

    fn load_toolchain_config(&mut self) {
        if let Ok(project) = crate::manifest::Project::load(Some(&self.project_root)) {
            if let Some(toolchain_version) = project.get_meta("toolchain") {
                eprintln!(
                    "📋 Found toolchain version in project metadata: {}",
                    toolchain_version
                );
                self.toolchain_version = Some(toolchain_version.to_string());

                if self.toolchain_root.is_none() {
                    match resolve_path("user://toolchains") {
                        ResolvedPath::Disk(path_buf) => {
                            self.toolchain_root = Some(path_buf);
                        }
                        ResolvedPath::Brk(_) => {
                            eprintln!(
                                "⚠️  user://toolchains resolved to BRK path, falling back to project-relative"
                            );
                            let toolchain_root =
                                self.project_root.join(".perro").join("toolchains");
                            self.toolchain_root = Some(toolchain_root);
                        }
                    }
                }
            }
        }
    }

    fn get_toolchain_dir(&self) -> Option<PathBuf> {
        let version = self.toolchain_version.as_deref().unwrap_or("1.93.0");
        let toolchain_name = self.platform.toolchain_name(version);
        let toolchain_path_str = format!("user://toolchains/{}", toolchain_name);

        match resolve_path(&toolchain_path_str) {
            ResolvedPath::Disk(path_buf) => Some(path_buf),
            ResolvedPath::Brk(_) => None,
        }
    }

    fn get_cargo_path(&self) -> Option<PathBuf> {
        self.get_toolchain_dir().map(|toolchain_dir| {
            toolchain_dir
                .join("cargo")
                .join("bin")
                .join(self.platform.cargo_exe())
        })
    }

    /// Returns the build cache directory when using toolchain cargo
    fn toolchain_build_cache(&self) -> Option<PathBuf> {
        if self.from_source {
            return None;
        }

        let version = self.toolchain_version.as_deref().unwrap_or("1.93.0");
        let toolchain_name = self.platform.toolchain_name(version);

        match resolve_path("user://build-cache") {
            ResolvedPath::Disk(root) => Some(root.join(toolchain_name)),
            ResolvedPath::Brk(_) => None,
        }
    }

    /// Returns the cargo target directory based on whether we're using toolchain or system cargo
    fn get_cargo_target_dir(&self) -> Option<PathBuf> {
        if self.from_source {
            // When using system cargo, force it to use the parent workspace's target
            // even though .perro/scripts is its own workspace (for development purposes)
            self.find_parent_workspace_target_dir()
        } else {
            // When using toolchain cargo, use the build cache
            self.toolchain_build_cache()
        }
    }

    /// Find the parent workspace root's target directory, skipping the immediate workspace
    fn find_parent_workspace_target_dir(&self) -> Option<PathBuf> {
        // The manifest is at: perro_editor\.perro\project\Cargo.toml or .perro\scripts\Cargo.toml
        // Start from .perro (parent of scripts/project) and walk up from there
        // IMPORTANT: Skip any .perro subdirectories (like .perro/project) as they are not the root workspace

        let manifest_dir = self.crate_manifest_path.parent()?; // .perro/scripts or .perro/project
        let perro_dir = manifest_dir.parent()?; // .perro

        // OPTIMIZED: Try canonicalize, but fall back to non-canonicalized path if it fails
        // This handles cases where the path might not exist yet or canonicalize fails
        let mut current = dunce::canonicalize(perro_dir)
            .ok()
            .unwrap_or_else(|| perro_dir.to_path_buf());

        loop {
            // Move up to parent first
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                // Hit filesystem root
                break;
            }

            // Skip if we're still inside a .perro directory (like .perro/project)
            // Check if any component of the path is ".perro"
            if current.components().any(|c| {
                if let std::path::Component::Normal(name) = c {
                    name == ".perro"
                } else {
                    false
                }
            }) {
                continue;
            }

            // Look for Cargo.toml that defines a workspace
            let workspace_manifest = current.join("Cargo.toml");
            if workspace_manifest.exists() {
                // Check if it's a workspace by reading the file
                if let Ok(contents) = std::fs::read_to_string(&workspace_manifest) {
                    if contents.contains("[workspace]") {
                        let target_dir = current.join("target");
                        // Canonicalize to ensure absolute path (required for CARGO_TARGET_DIR)
                        // If canonicalize fails, use the path as-is (it should still work)
                        let target_dir_abs =
                            dunce::canonicalize(&target_dir).unwrap_or_else(|_| {
                                // If canonicalize fails, make it absolute manually
                                if target_dir.is_absolute() {
                                    target_dir
                                } else {
                                    // Try to make it absolute by joining with current dir
                                    std::env::current_dir()
                                        .ok()
                                        .and_then(|cwd| cwd.join(&target_dir).canonicalize().ok())
                                        .unwrap_or(target_dir)
                                }
                            });
                        eprintln!(
                            "📂 Found parent workspace at: {} (target: {})",
                            current.display(),
                            target_dir_abs.display()
                        );
                        return Some(target_dir_abs);
                    }
                }
            }
        }

        eprintln!(
            "⚠️  Could not find parent workspace root (searched from: {})",
            perro_dir.display()
        );
        None
    }

    /// Get the source path of the built DLL
    fn get_built_dll_path(&self, profile: &str) -> PathBuf {
        let crate_name = match self.target {
            CompileTarget::Scripts => "scripts",
            _ => "project",
        };

        // Get the target directory (either build cache or workspace target)
        let target_base = self
            .get_cargo_target_dir()
            .expect("Could not determine target directory for build");

        let profile_dir = target_base.join(profile);

        // Platform-specific library naming - Cargo prefixes library artifacts with "lib" on *nix
        let dll_path = if cfg!(target_os = "windows") {
            profile_dir.join(format!("{}.dll", crate_name))
        } else if cfg!(target_os = "macos") {
            profile_dir.join(format!("lib{}.dylib", crate_name))
        } else {
            profile_dir.join(format!("lib{}.so", crate_name))
        };

        eprintln!("🔍 Looking for built DLL at: {}", dll_path.display());
        dll_path
    }

    /// Copy the built DLL to the project's build output directory
    fn copy_script_dll(&self, profile: &str) -> std::io::Result<()> {
        let src_file = self.get_built_dll_path(profile);

        // Output to .perro/scripts/builds/ in the project directory
        let output_dir = self.project_root.join(".perro/scripts/builds");
        fs::create_dir_all(&output_dir)?;

        let final_dylib_name = script_dylib_name();
        let dest_file = output_dir.join(final_dylib_name);

        eprintln!(
            "📦 Copying {} -> {}",
            src_file.display(),
            dest_file.display()
        );
        fs::copy(&src_file, &dest_file)?;

        Ok(())
    }

    fn build_command(&self, profile: &BuildProfile) -> Result<Command, String> {
        let (mut cmd, toolchain_dir) = if self.from_source {
            eprintln!("🔧 Using system cargo (source code mode)");
            (Command::new("cargo"), None)
        } else {
            // Try to use toolchain cargo, fallback to system
            if let Some(cargo_path) = self.get_cargo_path() {
                if cargo_path.exists() {
                    eprintln!("✅ Using toolchain cargo: {}", cargo_path.display());
                    let toolchain_dir = self.get_toolchain_dir();
                    (Command::new(cargo_path), toolchain_dir)
                } else {
                    eprintln!("⚠️  Toolchain cargo not found, using system cargo");
                    (Command::new("cargo"), None)
                }
            } else {
                eprintln!("🔧 Using system cargo (no custom toolchain)");
                (Command::new("cargo"), None)
            }
        };

        match profile {
            BuildProfile::Check => cmd.arg("check"),
            _ => cmd.arg("build"),
        };

        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        cmd.arg("--manifest-path")
            .arg(&self.crate_manifest_path)
            .arg("-j")
            .arg(num_cpus.to_string());

        // Hide console window in release mode (when not from_source)
        // In dev mode, we want to see cargo output for debugging
        if !self.from_source && !cfg!(debug_assertions) {
            // Release mode: hide console window
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());
        } else {
            // Dev mode: show output
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        }

        // Set working directory to the manifest directory so relative paths resolve correctly
        if let Some(manifest_dir) = self.crate_manifest_path.parent() {
            cmd.current_dir(manifest_dir);
        }

        // When using toolchain cargo, add rustc to PATH and set up environment for standard library
        if let Some(ref toolchain_dir) = toolchain_dir {
            eprintln!("📂 Toolchain directory: {}", toolchain_dir.display());
            let rustc_bin_dir = toolchain_dir.join("rustc").join("bin");
            if rustc_bin_dir.exists() {
                // Get current PATH
                let mut path_entries = Vec::new();
                path_entries.push(rustc_bin_dir.display().to_string());

                // On Windows with GNU target, set up MinGW for both linking and C compilation
                #[cfg(target_os = "windows")]
                {
                    // First, check for bundled GCC compiler (w64devkit) for C/C++ compilation
                    let bundled_mingw_bin = toolchain_dir.join("mingw").join("bin");
                    let bundled_gcc = bundled_mingw_bin.join("gcc.exe");

                    if bundled_gcc.exists() {
                        eprintln!(
                            "✅ Found bundled GCC compiler at: {}",
                            bundled_mingw_bin.display()
                        );
                        path_entries.push(bundled_mingw_bin.display().to_string());

                        // Set CC and CXX environment variables for build scripts
                        cmd.env("CC", &bundled_gcc);
                        cmd.env("CXX", bundled_mingw_bin.join("g++.exe"));
                        cmd.env("CC_x86_64-pc-windows-gnu", &bundled_gcc);
                        cmd.env(
                            "CXX_x86_64-pc-windows-gnu",
                            bundled_mingw_bin.join("g++.exe"),
                        );
                        eprintln!("✅ Set CC={}", bundled_gcc.display());
                        eprintln!("✅ Set CXX={}", bundled_mingw_bin.join("g++.exe").display());
                    } else {
                        eprintln!("⚠️  Bundled GCC compiler not found (mingw/bin/gcc.exe)");
                        eprintln!(
                            "ℹ️  C/C++ compilation in build scripts will not work without it"
                        );
                    }

                    // Also add self-contained MinGW linker to PATH (for linking Rust code)
                    let rust_mingw_dir = toolchain_dir.join("rust-mingw");
                    let possible_mingw_bins = vec![
                        rust_mingw_dir
                            .join("lib")
                            .join("rustlib")
                            .join("x86_64-pc-windows-gnu")
                            .join("bin")
                            .join("self-contained"),
                        rust_mingw_dir.join("lib").join("self-contained"),
                        rust_mingw_dir.join("bin"),
                    ];

                    for mingw_bin in &possible_mingw_bins {
                        if mingw_bin.exists() {
                            eprintln!("✅ Found MinGW linker at: {}", mingw_bin.display());
                            // Add to PATH but AFTER bundled GCC so bundled GCC is used for compilation
                            // The linker from rust-mingw is still used for linking Rust code
                            path_entries.push(mingw_bin.display().to_string());

                            // Set LIBRARY_PATH so the linker can find the libraries and C runtime files
                            let mut library_paths = vec![mingw_bin.display().to_string()];

                            // Add lib/self-contained directory which contains all MinGW libraries
                            // This is at rust-mingw/lib/rustlib/x86_64-pc-windows-gnu/lib/self-contained
                            if let Some(bin_dir) = mingw_bin.parent() {
                                if let Some(base_dir) = bin_dir.parent() {
                                    let lib_self_contained =
                                        base_dir.join("lib").join("self-contained");
                                    if lib_self_contained.exists() {
                                        library_paths
                                            .push(lib_self_contained.display().to_string());
                                        eprintln!(
                                            "✅ Added lib/self-contained to LIBRARY_PATH: {}",
                                            lib_self_contained.display()
                                        );
                                    }
                                }
                            }

                            // Also add bundled MinGW lib directories if they exist
                            if bundled_gcc.exists() {
                                let bundled_lib = toolchain_dir.join("mingw").join("lib");
                                if bundled_lib.exists() {
                                    library_paths.push(bundled_lib.display().to_string());
                                    eprintln!(
                                        "✅ Added bundled MinGW lib to LIBRARY_PATH: {}",
                                        bundled_lib.display()
                                    );
                                }
                            }

                            let current_lib_path =
                                std::env::var("LIBRARY_PATH").unwrap_or_default();
                            let lib_path_separator = if cfg!(target_os = "windows") {
                                ";"
                            } else {
                                ":"
                            };
                            let new_lib_path = if current_lib_path.is_empty() {
                                library_paths.join(lib_path_separator)
                            } else {
                                format!(
                                    "{}{}{}",
                                    library_paths.join(lib_path_separator),
                                    lib_path_separator,
                                    current_lib_path
                                )
                            };
                            cmd.env("LIBRARY_PATH", &new_lib_path);
                            eprintln!("✅ Set LIBRARY_PATH to: {}", new_lib_path);
                            break;
                        }
                    }
                }

                // Prepend all toolchain paths to PATH
                // Prepend toolchain paths to PATH
                // The self-contained MinGW is prepended first, so linking will use it
                // We don't filter PATH - if system MinGW exists, it might be used for C compilation
                // but since we don't set CC, build scripts won't try to use the linker-only gcc for compilation
                let current_path = std::env::var("PATH").unwrap_or_default();
                let path_separator = if cfg!(target_os = "windows") {
                    ";"
                } else {
                    ":"
                };

                let filtered_path: Vec<String> = current_path
                    .split(&path_separator)
                    .filter(|path| !path.is_empty())
                    .map(|s| s.to_string())
                    .collect();

                let filtered_path_str = filtered_path.join(&path_separator);
                let new_path = format!(
                    "{}{}{}",
                    path_entries.join(&path_separator),
                    path_separator,
                    filtered_path_str
                );
                cmd.env("PATH", &new_path);
                eprintln!("✅ Set PATH with toolchain paths prepended");

                // Also set RUSTC explicitly to be safe
                let rustc_exe = rustc_bin_dir.join(if cfg!(target_os = "windows") {
                    "rustc.exe"
                } else {
                    "rustc"
                });
                if rustc_exe.exists() {
                    cmd.env("RUSTC", &rustc_exe);
                }

                // Set sysroot so rustc can find the standard library
                // The sysroot is the directory containing lib/rustlib/
                // For standalone toolchains, check if standard library is merged or in rust-std subdirectory
                let target_triple = if cfg!(target_os = "windows") {
                    "x86_64-pc-windows-gnu"
                } else if cfg!(target_os = "macos") {
                    "x86_64-apple-darwin"
                } else {
                    "x86_64-unknown-linux-gnu"
                };

                // Determine where the standard library is located
                // The standard library structure in extracted toolchains:
                // - rust-std-<target>/lib/rustlib/<target>/lib/ contains the std library files
                // - The sysroot should be the rust-std-<target> directory itself
                let rust_std_dir = toolchain_dir.join(format!("rust-std-{}", target_triple));
                let rust_std_lib = rust_std_dir
                    .join("lib")
                    .join("rustlib")
                    .join(target_triple)
                    .join("lib");

                // Also check if it's been merged into the main toolchain structure
                let merged_std_lib = toolchain_dir
                    .join("lib")
                    .join("rustlib")
                    .join(target_triple)
                    .join("lib");

                // Helper to check if a directory exists and has files
                let has_files = |path: &Path| -> bool {
                    path.exists()
                        && std::fs::read_dir(path)
                            .map(|d| d.count() > 0)
                            .unwrap_or(false)
                };

                // Check both locations
                let rust_std_has_files = has_files(&rust_std_lib);
                let merged_has_files = has_files(&merged_std_lib);

                // Determine sysroot - ALWAYS prefer rust-std directory if it exists with files
                // This is the standard location in extracted toolchains
                let sysroot = if rust_std_has_files {
                    // Standard library is in rust-std subdirectory (normal case for extracted toolchains)
                    eprintln!("📦 Standard library found in rust-std subdirectory");
                    eprintln!("   Location: {}", rust_std_lib.display());
                    eprintln!("   Using sysroot: {}", rust_std_dir.display());
                    dunce::canonicalize(&rust_std_dir).unwrap_or_else(|_| rust_std_dir.clone())
                } else if merged_has_files {
                    // Standard library has been merged into main toolchain structure
                    eprintln!("📦 Standard library found in merged location");
                    eprintln!("   Location: {}", merged_std_lib.display());
                    eprintln!("   Using sysroot: {}", toolchain_dir.display());
                    dunce::canonicalize(toolchain_dir).unwrap_or_else(|_| toolchain_dir.clone())
                } else {
                    // Neither location has files - this is an error condition
                    eprintln!("❌ ERROR: Standard library not found in expected locations!");
                    eprintln!(
                        "   Checked rust-std: {} (exists: {}, has files: {})",
                        rust_std_lib.display(),
                        rust_std_lib.exists(),
                        rust_std_has_files
                    );
                    eprintln!(
                        "   Checked merged: {} (exists: {}, has files: {})",
                        merged_std_lib.display(),
                        merged_std_lib.exists(),
                        merged_has_files
                    );
                    eprintln!("   Toolchain directory: {}", toolchain_dir.display());
                    eprintln!("   Attempting to list toolchain contents...");

                    // Diagnostic: list what's actually in the toolchain directory
                    if let Ok(entries) = std::fs::read_dir(toolchain_dir) {
                        let mut dirs = Vec::new();
                        for entry in entries.flatten() {
                            if entry.path().is_dir() {
                                dirs.push(entry.file_name().to_string_lossy().to_string());
                            }
                        }
                        dirs.sort();
                        eprintln!("   Directories found: {}", dirs.join(", "));
                    }

                    // Fallback to toolchain root (will likely fail, but at least we tried)
                    eprintln!(
                        "   ⚠️  Falling back to toolchain root as sysroot (this may not work)"
                    );
                    dunce::canonicalize(toolchain_dir).unwrap_or_else(|_| toolchain_dir.clone())
                };

                // Create a .cargo/config.toml file to set the sysroot
                // This avoids RUSTFLAGS parsing issues with spaces in paths
                if let Some(manifest_dir) = self.crate_manifest_path.parent() {
                    let cargo_config_dir = manifest_dir.join(".cargo");
                    let cargo_config_file = cargo_config_dir.join("config.toml");

                    // Create .cargo directory if it doesn't exist
                    if let Err(e) = std::fs::create_dir_all(&cargo_config_dir) {
                        eprintln!("⚠️  Warning: Failed to create .cargo directory: {}", e);
                    } else {
                        // Write config.toml with sysroot setting
                        // Use TOML format with proper escaping for Windows paths
                        let sysroot_str = sysroot.to_string_lossy();
                        eprintln!("🔧 Setting sysroot in .cargo/config.toml: {}", sysroot_str);

                        // For TOML, we need to escape backslashes and use forward slashes or raw strings
                        // Actually, TOML strings can use forward slashes or we can use raw strings
                        // Let's use forward slashes for Windows paths in TOML (Cargo accepts this)
                        let toml_path = sysroot_str.replace('\\', "/");

                        // ALWAYS set RUSTC_SYSROOT as environment variable - this is the most reliable way
                        // Use the actual PathBuf to ensure correct path format for Windows
                        // RUSTC_SYSROOT avoids path parsing issues with spaces, unlike RUSTFLAGS
                        let sysroot_env = sysroot.to_string_lossy().to_string();
                        cmd.env("RUSTC_SYSROOT", &sysroot_env);
                        eprintln!("🔧 Set RUSTC_SYSROOT={}", sysroot_env);

                        // Don't set --sysroot in RUSTFLAGS when RUSTC_SYSROOT is set, as it causes
                        // parsing issues with paths containing spaces. RUSTC_SYSROOT is sufficient.
                        // RUSTFLAGS will be set via config.toml instead, which handles paths correctly.

                        // Build config content with sysroot and linker configuration
                        // Start with [build] section
                        let mut config_content = format!(
                            "[build]\nrustflags = [\"--sysroot\", \"{}\"]\n\n",
                            toml_path
                        );

                        // On Windows with GNU target, configure the linker and library paths
                        #[cfg(target_os = "windows")]
                        {
                            let rust_mingw_dir = toolchain_dir.join("rust-mingw");
                            // Check multiple possible locations for MinGW gcc
                            let possible_mingw_bins = vec![
                                rust_mingw_dir
                                    .join("lib")
                                    .join("rustlib")
                                    .join("x86_64-pc-windows-gnu")
                                    .join("bin")
                                    .join("self-contained"),
                                rust_mingw_dir.join("lib").join("self-contained"),
                                rust_mingw_dir.join("bin"),
                            ];

                            let mut found_linker = false;
                            for mingw_bin in &possible_mingw_bins {
                                if mingw_bin.exists() {
                                    let gcc_exe = mingw_bin.join("x86_64-w64-mingw32-gcc.exe");
                                    if gcc_exe.exists() {
                                        let linker_path = gcc_exe
                                            .to_string_lossy()
                                            .replace('\\', "\\\\")
                                            .replace('"', "\\\"");

                                        // The libraries are in the same self-contained directory as the gcc executable
                                        // Also check for lib subdirectory in rust-mingw
                                        // C runtime object files (crt2.o, crtbegin.o, crtend.o) are typically in:
                                        // 1. Same directory as gcc (self-contained)
                                        // 2. lib subdirectory of self-contained directory
                                        // 3. lib/gcc/x86_64-w64-mingw32/<version>/ directory
                                        // 4. lib subdirectory relative to gcc
                                        let mut lib_paths = vec![
                                            mingw_bin.clone(), // Same directory as gcc (contains crt*.o files)
                                            rust_mingw_dir
                                                .join("lib")
                                                .join("rustlib")
                                                .join("x86_64-pc-windows-gnu")
                                                .join("lib")
                                                .join("self-contained"),
                                        ];

                                        // Check for lib subdirectory within the self-contained directory
                                        let self_contained_lib = mingw_bin.join("lib");
                                        if self_contained_lib.exists() {
                                            lib_paths.push(self_contained_lib);
                                        }

                                        // The lib/self-contained directory is already in lib_paths above
                                        // This contains all the MinGW libraries (.a files)
                                        // C runtime files (crt2.o, crtbegin.o, crtend.o) may be embedded
                                        // in the libraries or provided by the linker in the self-contained setup

                                        // Check for C runtime files in the gcc directory for diagnostics
                                        let crt_files = ["crt2.o", "crtbegin.o", "crtend.o"];
                                        let mut found_crt_files = Vec::new();
                                        for crt_file in &crt_files {
                                            let crt_path = mingw_bin.join(crt_file);
                                            if crt_path.exists() {
                                                found_crt_files.push(crt_file);
                                            }
                                        }
                                        if !found_crt_files.is_empty() {
                                            eprintln!(
                                                "✅ Found C runtime files in {}: {:?}",
                                                mingw_bin.display(),
                                                found_crt_files
                                            );
                                        } else {
                                            eprintln!(
                                                "⚠️  C runtime files not found in {} (will search other paths)",
                                                mingw_bin.display()
                                            );
                                        }

                                        // Build rustflags array with sysroot and library search paths
                                        let mut rustflags_parts =
                                            vec![format!("\"--sysroot\", \"{}\"", toml_path)];

                                        // Add -B flag to tell gcc where to find its support files
                                        // This makes gcc look for crt files in the expected relative paths
                                        // The -B flag should point to the directory containing gcc (bin/self-contained)
                                        let gcc_base_dir =
                                            mingw_bin.to_string_lossy().replace('\\', "/");
                                        rustflags_parts.push("\"-C\"".to_string());
                                        rustflags_parts
                                            .push(format!("\"link-arg=-B{}\"", gcc_base_dir));
                                        eprintln!(
                                            "✅ Added GCC base directory (-B): {}",
                                            mingw_bin.display()
                                        );

                                        // Also add -B flag for bundled MinGW where crt2.o actually is
                                        // This is critical - crt2.o is in mingw/x86_64-w64-mingw32/lib/
                                        let bundled_mingw_target_lib = toolchain_dir
                                            .join("mingw")
                                            .join("x86_64-w64-mingw32")
                                            .join("lib");
                                        if bundled_mingw_target_lib.exists() {
                                            let bundled_mingw_target_lib_toml =
                                                bundled_mingw_target_lib
                                                    .to_string_lossy()
                                                    .replace('\\', "/");
                                            rustflags_parts.push("\"-C\"".to_string());
                                            rustflags_parts.push(format!(
                                                "\"link-arg=-B{}\"",
                                                bundled_mingw_target_lib_toml
                                            ));
                                            eprintln!(
                                                "✅ Added bundled MinGW target lib base directory (-B): {}",
                                                bundled_mingw_target_lib.display()
                                            );
                                        }

                                        // Add MinGW library search paths
                                        for lib_path in &lib_paths {
                                            if lib_path.exists() {
                                                let lib_toml_path =
                                                    lib_path.to_string_lossy().replace('\\', "/");
                                                rustflags_parts.push("\"-C\"".to_string());
                                                rustflags_parts.push(format!(
                                                    "\"link-arg=-L{}\"",
                                                    lib_toml_path
                                                ));
                                                eprintln!(
                                                    "✅ Added library search path: {}",
                                                    lib_path.display()
                                                );
                                            }
                                        }

                                        // Also add bundled MinGW lib directories (contains crt2.o, crtbegin.o, crtend.o)
                                        // This is critical - the bundled GCC has the C runtime files we need
                                        // crt2.o is in mingw/x86_64-w64-mingw32/lib/
                                        let bundled_mingw_target_lib = toolchain_dir
                                            .join("mingw")
                                            .join("x86_64-w64-mingw32")
                                            .join("lib");
                                        if bundled_mingw_target_lib.exists() {
                                            let bundled_mingw_target_lib_toml =
                                                bundled_mingw_target_lib
                                                    .to_string_lossy()
                                                    .replace('\\', "/");
                                            rustflags_parts.push("\"-C\"".to_string());
                                            rustflags_parts.push(format!(
                                                "\"link-arg=-L{}\"",
                                                bundled_mingw_target_lib_toml
                                            ));
                                            eprintln!(
                                                "✅ Added bundled MinGW target lib path (contains crt2.o): {}",
                                                bundled_mingw_target_lib.display()
                                            );
                                        }

                                        // Also add mingw/lib for other libraries
                                        let bundled_mingw_lib =
                                            toolchain_dir.join("mingw").join("lib");
                                        if bundled_mingw_lib.exists() {
                                            let bundled_mingw_lib_toml = bundled_mingw_lib
                                                .to_string_lossy()
                                                .replace('\\', "/");
                                            rustflags_parts.push("\"-C\"".to_string());
                                            rustflags_parts.push(format!(
                                                "\"link-arg=-L{}\"",
                                                bundled_mingw_lib_toml
                                            ));
                                            eprintln!(
                                                "✅ Added bundled MinGW lib path: {}",
                                                bundled_mingw_lib.display()
                                            );
                                        }

                                        // Also add bundled MinGW lib/gcc directory if it exists (for crtbegin.o/crtend.o)
                                        let bundled_mingw_gcc_lib =
                                            toolchain_dir.join("mingw").join("lib").join("gcc");
                                        if bundled_mingw_gcc_lib.exists() {
                                            // Find the versioned subdirectory (e.g., lib/gcc/x86_64-w64-mingw32/14.1.0/)
                                            if let Ok(entries) =
                                                std::fs::read_dir(&bundled_mingw_gcc_lib)
                                            {
                                                for entry in entries.flatten() {
                                                    let path = entry.path();
                                                    if path.is_dir() {
                                                        if let Ok(sub_entries) =
                                                            std::fs::read_dir(&path)
                                                        {
                                                            for sub_entry in sub_entries.flatten() {
                                                                let sub_path = sub_entry.path();
                                                                if sub_path.is_dir() {
                                                                    let gcc_version_lib_toml =
                                                                        sub_path
                                                                            .to_string_lossy()
                                                                            .replace('\\', "/");
                                                                    rustflags_parts
                                                                        .push("\"-C\"".to_string());
                                                                    rustflags_parts.push(format!(
                                                                        "\"link-arg=-L{}\"",
                                                                        gcc_version_lib_toml
                                                                    ));
                                                                    eprintln!(
                                                                        "✅ Added bundled MinGW GCC lib path: {}",
                                                                        sub_path.display()
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Add rust-std library directories where C runtime files are located
                                        // crt2.o and dllcrt2.o are in lib/rustlib/x86_64-pc-windows-gnu/lib/self-contained
                                        // rsbegin.o and rsend.o are in lib/rustlib/x86_64-pc-windows-gnu/lib
                                        // Canonicalize paths to ensure they're absolute and properly formatted
                                        let rust_std_lib_self_contained = dunce::canonicalize(
                                            sysroot
                                                .join("lib")
                                                .join("rustlib")
                                                .join(target_triple)
                                                .join("lib")
                                                .join("self-contained"),
                                        )
                                        .unwrap_or_else(|_| {
                                            sysroot
                                                .join("lib")
                                                .join("rustlib")
                                                .join(target_triple)
                                                .join("lib")
                                                .join("self-contained")
                                        });

                                        let rust_std_lib = dunce::canonicalize(
                                            sysroot
                                                .join("lib")
                                                .join("rustlib")
                                                .join(target_triple)
                                                .join("lib"),
                                        )
                                        .unwrap_or_else(|_| {
                                            sysroot
                                                .join("lib")
                                                .join("rustlib")
                                                .join(target_triple)
                                                .join("lib")
                                        });

                                        // Always add these paths - they're needed for crt2.o and other startup files
                                        // Even if the directory doesn't exist, adding the path won't hurt
                                        let rust_std_lib_self_contained_toml =
                                            rust_std_lib_self_contained
                                                .to_string_lossy()
                                                .replace('\\', "/");
                                        rustflags_parts.push("\"-C\"".to_string());
                                        rustflags_parts.push(format!(
                                            "\"link-arg=-L{}\"",
                                            rust_std_lib_self_contained_toml
                                        ));
                                        eprintln!(
                                            "✅ Added rust-std lib/self-contained path: {}",
                                            rust_std_lib_self_contained.display()
                                        );

                                        let rust_std_lib_toml =
                                            rust_std_lib.to_string_lossy().replace('\\', "/");
                                        rustflags_parts.push("\"-C\"".to_string());
                                        rustflags_parts
                                            .push(format!("\"link-arg=-L{}\"", rust_std_lib_toml));
                                        eprintln!(
                                            "✅ Added rust-std lib path: {}",
                                            rust_std_lib.display()
                                        );

                                        // Also add to LIBRARY_PATH environment variable for the linker
                                        // This is critical - the linker needs these paths to find crt2.o
                                        let current_lib_path =
                                            std::env::var("LIBRARY_PATH").unwrap_or_default();
                                        let lib_path_separator = if cfg!(target_os = "windows") {
                                            ";"
                                        } else {
                                            ":"
                                        };
                                        let mut additional_lib_paths = vec![
                                            rust_std_lib_self_contained.display().to_string(),
                                            rust_std_lib.display().to_string(),
                                        ];

                                        // Add bundled MinGW lib directories to LIBRARY_PATH (contains crt2.o)
                                        // crt2.o is in x86_64-w64-mingw32/lib/
                                        if bundled_mingw_target_lib.exists() {
                                            additional_lib_paths.push(
                                                bundled_mingw_target_lib.display().to_string(),
                                            );
                                        }
                                        if bundled_mingw_lib.exists() {
                                            additional_lib_paths
                                                .push(bundled_mingw_lib.display().to_string());
                                        }

                                        let new_lib_path = if current_lib_path.is_empty() {
                                            additional_lib_paths.join(lib_path_separator)
                                        } else {
                                            format!(
                                                "{}{}{}",
                                                additional_lib_paths.join(lib_path_separator),
                                                lib_path_separator,
                                                current_lib_path
                                            )
                                        };
                                        cmd.env("LIBRARY_PATH", &new_lib_path);
                                        eprintln!(
                                            "✅ Updated LIBRARY_PATH with all paths: {}",
                                            new_lib_path
                                        );

                                        // Tell Rust to use its own startup files (rsbegin.o, rsend.o, crt2.o)
                                        // instead of looking for GCC's crtbegin.o and crtend.o which don't exist
                                        rustflags_parts.push("\"-C\"".to_string());
                                        rustflags_parts
                                            .push("\"link-self-contained=yes\"".to_string());
                                        eprintln!(
                                            "✅ Added link-self-contained=yes to use Rust's startup files"
                                        );

                                        // Write target section with both linker and rustflags
                                        // Format rustflags as a multi-line array for better readability
                                        let rustflags_formatted = rustflags_parts
                                            .iter()
                                            .map(|s| format!("    {}", s))
                                            .collect::<Vec<_>>()
                                            .join(",\n");
                                        config_content.push_str(&format!(
                                            "[target.{}]\nlinker = \"{}\"\nrustflags = [\n{}\n]\n",
                                            target_triple, linker_path, rustflags_formatted
                                        ));
                                        eprintln!(
                                            "✅ Configured MinGW linker: {}",
                                            gcc_exe.display()
                                        );
                                        for lib_path in &lib_paths {
                                            if lib_path.exists() {
                                                eprintln!(
                                                    "✅ Added MinGW library path: {}",
                                                    lib_path.display()
                                                );
                                            }
                                        }
                                        found_linker = true;
                                        break;
                                    }
                                }
                            }

                            // If no MinGW linker found, still add target section with just rustflags
                            if !found_linker {
                                config_content.push_str(&format!(
                                    "[target.{}]\nrustflags = [\"--sysroot\", \"{}\"]\n",
                                    target_triple, toml_path
                                ));
                            }
                        }

                        #[cfg(not(target_os = "windows"))]
                        {
                            // For non-Windows, just add target section with rustflags
                            config_content.push_str(&format!(
                                "[target.{}]\nrustflags = [\"--sysroot\", \"{}\"]\n",
                                target_triple, toml_path
                            ));
                        }

                        eprintln!(
                            "📝 Writing .cargo/config.toml with content:\n{}",
                            config_content
                        );

                        if let Err(e) = std::fs::write(&cargo_config_file, config_content) {
                            eprintln!("⚠️  Warning: Failed to write .cargo/config.toml: {}", e);
                            eprintln!(
                                "   RUSTC_SYSROOT is already set, which should be sufficient"
                            );
                            // RUSTC_SYSROOT is already set above, so we don't need to set RUSTFLAGS
                            // Setting --sysroot in RUSTFLAGS would cause parsing issues with paths containing spaces
                        } else {
                            eprintln!(
                                "🔧 Created .cargo/config.toml with sysroot: {}",
                                sysroot.display()
                            );
                            // RUSTC_SYSROOT is already set above, which is the most reliable method
                        }
                    }
                }

                // Verify standard library exists and log for debugging
                // rustc expects it at <sysroot>/lib/rustlib/<target>/lib/
                let expected_std_lib = sysroot
                    .join("lib")
                    .join("rustlib")
                    .join(target_triple)
                    .join("lib");

                let std_lib_has_files = expected_std_lib.exists()
                    && std::fs::read_dir(&expected_std_lib)
                        .map(|d| d.count() > 0)
                        .unwrap_or(false);

                if std_lib_has_files {
                    eprintln!(
                        "✅ Standard library found at: {}",
                        expected_std_lib.display()
                    );
                } else {
                    eprintln!(
                        "⚠️  Warning: Standard library directory not found or empty: {}",
                        expected_std_lib.display()
                    );
                    eprintln!("   This may cause 'can't find crate for core' errors");
                    eprintln!("   Sysroot: {}", sysroot.display());
                    eprintln!(
                        "   Expected location: <sysroot>/lib/rustlib/{}/lib/",
                        target_triple
                    );

                    // Also check the rust-std location for diagnostic purposes
                    let rust_std_check = toolchain_dir
                        .join(format!("rust-std-{}", target_triple))
                        .join("lib")
                        .join("rustlib")
                        .join(target_triple)
                        .join("lib");
                    if rust_std_check.exists() {
                        eprintln!(
                            "   💡 Note: Standard library exists at: {}",
                            rust_std_check.display()
                        );
                        eprintln!(
                            "   💡 Consider merging it or using rust-std directory as sysroot"
                        );
                    }
                }
            } else {
                eprintln!(
                    "⚠️  Warning: rustc bin directory not found in toolchain: {}",
                    rustc_bin_dir.display()
                );
            }
        }

        // Set CARGO_TARGET_DIR to control where cargo builds
        if let Some(target_dir) = self.get_cargo_target_dir() {
            // Canonicalize to ensure absolute path (required for CARGO_TARGET_DIR)
            let target_dir_abs =
                dunce::canonicalize(&target_dir).unwrap_or_else(|_| target_dir.clone());

            if self.from_source {
                eprintln!(
                    "📁 Using workspace target directory: {}",
                    target_dir_abs.display()
                );
            } else {
                eprintln!("📁 Using build cache: {}", target_dir_abs.display());
            }
            // CARGO_TARGET_DIR must be an absolute path
            cmd.env("CARGO_TARGET_DIR", &target_dir_abs);
        } else {
            eprintln!("⚠️  Could not determine target directory, using cargo default");
        }

        match self.target {
            CompileTarget::Scripts => {
                cmd.arg("--profile").arg("dev");
            }
            CompileTarget::Project | CompileTarget::VerboseProject => {
                match profile {
                    BuildProfile::Dev => cmd.arg("--profile").arg("dev"),
                    BuildProfile::Release => cmd.arg("--release"),
                    BuildProfile::Check => &mut cmd,
                };

                cmd.env(
                    "PERRO_BUILD_TIMESTAMP",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        .to_string(),
                );
            }
        }

        Ok(cmd)
    }

    /// Build and copy the output to the final location
    pub fn compile(&self, profile: BuildProfile) -> Result<(), String> {
        // Handle verbose project builds (remove Windows subsystem flag for console visibility)
        if matches!(self.target, CompileTarget::VerboseProject) {
            self.remove_windows_subsystem_flag()?;
        }

        if matches!(
            self.target,
            CompileTarget::Project | CompileTarget::VerboseProject
        ) {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);

            println!("🔑 Compile-time AES key: {:02X?}", key);

            // Time key file writing (usually very fast, but good for completeness)
            let key_write_start = Instant::now();
            self.write_key_file(&key).map_err(|e| e.to_string())?;
            let key_write_elapsed = key_write_start.elapsed();
            println!("✔ Key file written (total {:.2?})", key_write_elapsed);

            // Generate scenes in the project crate instead of scripts crate
            let project_manifest = self.project_root.join(".perro/project/Cargo.toml");
            if project_manifest.exists() {
                // Sync project crate name from project.toml so binary name matches project name
                self.sync_project_crate_name_from_project_toml()?;

                let project_crate_root = project_manifest
                    .parent()
                    .expect("Project crate manifest has no parent");
                println!("⚙️ Generating static scene code for project crate...");
                let codegen_start = Instant::now();

                self.codegen_assets(project_crate_root)
                    .map_err(|e| e.to_string())?;

                // Generate main.rs
                self.codegen_main_file(project_crate_root)
                    .map_err(|e| e.to_string())?;

                // Generate build.rs
                self.codegen_build_rs(project_crate_root)
                    .map_err(|e| e.to_string())?;

                let codegen_elapsed = codegen_start.elapsed();
                println!("✅ Asset codegen complete (total {:.2?})", codegen_elapsed);
            } else {
                eprintln!(
                    "⚠️  Could not find project manifest at {}; skipping scene codegen.",
                    project_manifest.display()
                );
            }

            let res_dir = self.project_root.join("res");
            let output = self.project_root.join("assets.brk");

            // --- TIME THE BRK BUILD HERE ---
            println!("📦 Building BRK archive from {}...", res_dir.display());
            let brk_build_start = Instant::now();
            build_brk(&output, &res_dir, &self.project_root, &key).map_err(|e| e.to_string())?;
            let brk_build_elapsed = brk_build_start.elapsed();
            println!("✅ BRK archive built (total {:.2?})", brk_build_elapsed);
            // --- END BRK TIMING ---
        }

        let toolchain_info = if self.from_source {
            "system (local development)".to_string()
        } else {
            let version = self.toolchain_version.as_deref().unwrap_or("1.93.0");
            let toolchain_name = self.platform.toolchain_name(version);

            self.get_toolchain_dir()
                .map(|p| format!("{} ({})", toolchain_name, p.display()))
                .unwrap_or_else(|| "system (fallback)".to_string())
        };

        println!(
            "🚀 Compiling {:?} [{:?}] with toolchain: {}",
            self.target_name(),
            profile,
            toolchain_info
        );

        let start = Instant::now();
        let mut cmd = self.build_command(&profile)?;
        let status = cmd
            .status()
            .map_err(|e| format!("Failed to run cargo: {e}"))?;
        let elapsed = start.elapsed();

        if !status.success() {
            return Err(format!("❌ Compilation failed after {:.2?}", elapsed));
        }

        println!("✅ Compilation successful! (total {:.2?})", elapsed);

        // Copy the built DLL to the output location
        if matches!(self.target, CompileTarget::Scripts) {
            let profile_str = match profile {
                BuildProfile::Dev => "debug",
                BuildProfile::Release => "release",
                BuildProfile::Check => return Ok(()), // No copy needed for check
            };

            self.copy_script_dll(profile_str)
                .map_err(|e| format!("Failed to copy DLL: {}", e))?;
        }

        // On macOS, when building a Project we prefer to assemble a .app bundle
        // so users can launch it from Finder (this prevents Terminal from opening
        // when double-clicking the application). The generated build.rs already
        // creates Info.plist and icon.icns in the project root; here we copy
        // the built binary into a standard .app/Contents/MacOS layout.
        // Only package as .app for Project (non-verbose). VerboseProject leaves
        // the binary in target/release/ so it can be run from a terminal.
        if matches!(
            self.target,
            CompileTarget::Project | CompileTarget::VerboseProject
        ) {
            #[cfg(target_os = "macos")]
            let do_app_bundle = matches!(self.target, CompileTarget::Project);
            #[cfg(not(target_os = "macos"))]
            let do_app_bundle = false;
            if do_app_bundle {
                #[cfg(target_os = "macos")]
                {
                    use std::fs;
                    use toml::Value;

                    let project_toml = self.project_root.join("project.toml");
                    if project_toml.exists() {
                        if let Ok(content) = fs::read_to_string(&project_toml) {
                            if let Ok(cfg) = content.parse::<Value>() {
                                if let Some(project_table) = cfg.get("project") {
                                    if let Some(name_val) = project_table.get("name") {
                                        if let Some(name_str) = name_val.as_str() {
                                            let crate_name = project_name_to_crate_name(name_str);

                                            let profile_str = match profile {
                                                BuildProfile::Dev => "debug",
                                                _ => "release",
                                            };

                                            // Determine cargo target dir (toolchain cache or workspace target)
                                            let target_dir =
                                                self.get_cargo_target_dir().unwrap_or_else(|| {
                                                    self.project_root.join("target")
                                                });

                                            let binary_path =
                                                target_dir.join(profile_str).join(&crate_name);

                                            if binary_path.exists() {
                                                // Place the .app bundle next to the built binary (so output is colocated
                                                // in the target/<profile> folder). E.g. target/release/MyGame.app
                                                let app_bundle = binary_path.with_extension("app");
                                                let contents = app_bundle.join("Contents");
                                                let macos_dir = contents.join("MacOS");
                                                let resources_dir = contents.join("Resources");

                                                if let Err(e) = fs::create_dir_all(&macos_dir) {
                                                    eprintln!(
                                                        "⚠️ Failed to create .app MacOS dir: {}",
                                                        e
                                                    );
                                                }
                                                if let Err(e) = fs::create_dir_all(&resources_dir) {
                                                    eprintln!(
                                                        "⚠️ Failed to create .app Resources dir: {}",
                                                        e
                                                    );
                                                }

                                                let dest_bin = macos_dir.join(&crate_name);

                                                // Prefer to MOVE the binary into the .app so the target folder only
                                                // contains the .app bundle. If rename fails (cross-filesystem,
                                                // permissions, etc.) we fall back to copy+remove but log clearly.
                                                match fs::rename(&binary_path, &dest_bin) {
                                                    Ok(_) => {
                                                        println!(
                                                            "✔ Moved binary into .app: {} -> {}",
                                                            binary_path.display(),
                                                            dest_bin.display()
                                                        );

                                                        // Ensure executable bit on the moved binary
                                                        #[cfg(unix)]
                                                        {
                                                            use std::os::unix::fs::PermissionsExt;
                                                            if let Ok(meta) =
                                                                fs::metadata(&dest_bin)
                                                            {
                                                                let mut perms = meta.permissions();
                                                                perms.set_mode(0o755);
                                                                let _ = fs::set_permissions(
                                                                    &dest_bin, perms,
                                                                );
                                                            }
                                                        }
                                                    }
                                                    Err(rename_err) => {
                                                        eprintln!(
                                                            "⚠️ Failed to rename binary into .app: {}. Attempting copy+remove fallback.",
                                                            rename_err
                                                        );

                                                        // Fallback: try to copy and then remove original so end result is still
                                                        // a single .app. This is a best-effort fallback.
                                                        match fs::copy(&binary_path, &dest_bin) {
                                                            Ok(_) => {
                                                                // Ensure executable bit
                                                                #[cfg(unix)]
                                                                {
                                                                    use std::os::unix::fs::PermissionsExt;
                                                                    if let Ok(meta) =
                                                                        fs::metadata(&dest_bin)
                                                                    {
                                                                        let mut perms =
                                                                            meta.permissions();
                                                                        perms.set_mode(0o755);
                                                                        let _ = fs::set_permissions(
                                                                            &dest_bin, perms,
                                                                        );
                                                                    }
                                                                }

                                                                // Try to remove the original binary
                                                                if let Err(rem_err) =
                                                                    fs::remove_file(&binary_path)
                                                                {
                                                                    eprintln!(
                                                                        "⚠️ Copied binary into .app but failed to remove original: {}",
                                                                        rem_err
                                                                    );
                                                                } else {
                                                                    println!(
                                                                        "✔ Moved binary into .app via copy+remove: {} -> {}",
                                                                        binary_path.display(),
                                                                        dest_bin.display()
                                                                    );
                                                                }
                                                            }
                                                            Err(copy_err) => {
                                                                eprintln!(
                                                                    "❌ Failed to copy binary into .app fallback: {}",
                                                                    copy_err
                                                                );
                                                                eprintln!(
                                                                    "ℹ️ The built binary remains at: {}. You can move it into {} manually.",
                                                                    binary_path.display(),
                                                                    app_bundle.display()
                                                                );
                                                            }
                                                        }
                                                    }
                                                }

                                                // Copy Info.plist and icon if present in project root
                                                let info_src = self.project_root.join("Info.plist");
                                                if info_src.exists() {
                                                    let _ = fs::copy(
                                                        &info_src,
                                                        contents.join("Info.plist"),
                                                    );
                                                }

                                                let icns_src = self.project_root.join("icon.icns");
                                                if icns_src.exists() {
                                                    let _ = fs::copy(
                                                        &icns_src,
                                                        resources_dir.join("icon.icns"),
                                                    );
                                                }

                                                println!(
                                                    "✔ Created macOS .app bundle: {}",
                                                    app_bundle.display()
                                                );
                                            } else {
                                                println!(
                                                    "⚠️  Built binary not found at expected path: {}",
                                                    binary_path.display()
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // When --out is set, move the built artifact there (no duplicate left in target/release)
        if let Some(ref out_dir) = self.output_path {
            let target_dir = self
                .get_cargo_target_dir()
                .unwrap_or_else(|| self.project_root.join("target"));
            let profile_str = "release";
            let project_toml_path = self.project_root.join("project.toml");
            let config = fs::read_to_string(&project_toml_path)
                .ok()
                .and_then(|c| c.parse::<toml::Value>().ok());
            let project_name = config
                .as_ref()
                .and_then(|c| c.get("project"))
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Perro Game");
            let crate_name = project_name_to_crate_name(project_name);
            let exe_name = if cfg!(target_os = "windows") {
                format!("{}.exe", crate_name)
            } else {
                crate_name.clone()
            };
            let binary_path = target_dir.join(profile_str).join(&exe_name);
            #[cfg(target_os = "macos")]
            let is_app = matches!(self.target, CompileTarget::Project);
            #[cfg(not(target_os = "macos"))]
            let is_app = false;

            fs::create_dir_all(out_dir)
                .map_err(|e| format!("Failed to create output dir: {}", e))?;
            if is_app {
                let app_bundle = target_dir
                    .join(profile_str)
                    .join(format!("{}.app", crate_name));
                if app_bundle.exists() {
                    let dest = out_dir.join(format!("{}.app", crate_name));
                    if dest.exists() {
                        fs::remove_dir_all(&dest)
                            .map_err(|e| format!("Failed to remove existing .app: {}", e))?;
                    }
                    fs::rename(&app_bundle, &dest)
                        .map_err(|e| format!("Failed to move .app: {}", e))?;
                    println!("✔ Moved .app to {}", dest.display());
                } else {
                    return Err(format!("Built .app not found at {}", app_bundle.display()));
                }
            } else {
                if binary_path.exists() {
                    let dest = out_dir.join(&exe_name);
                    if dest.exists() {
                        fs::remove_file(&dest)
                            .map_err(|e| format!("Failed to remove existing executable: {}", e))?;
                    }
                    fs::rename(&binary_path, &dest)
                        .map_err(|e| format!("Failed to move binary: {}", e))?;
                    #[cfg(unix)]
                    if let Ok(meta) = fs::metadata(&dest) {
                        let _ = fs::set_permissions(&dest, meta.permissions());
                    }
                    println!("✔ Moved executable to {}", dest.display());
                } else {
                    return Err(format!(
                        "Built binary not found at {}",
                        binary_path.display()
                    ));
                }
            }
        }

        Ok(())
    }

    /// Remove Windows subsystem flag from Cargo.toml for verbose builds (shows console)
    fn remove_windows_subsystem_flag(&self) -> Result<(), String> {
        use std::fs;
        use toml::Value;

        let project_manifest = self.project_root.join(".perro/project/Cargo.toml");
        if !project_manifest.exists() {
            return Ok(()); // No project manifest, skip
        }

        let content = fs::read_to_string(&project_manifest)
            .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;

        let mut doc: Value = content
            .parse()
            .map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;

        // Remove the Windows subsystem flag if it exists
        // TOML structure: [target.'cfg(windows)'] rustflags = [...]
        if let Some(root_table) = doc.as_table_mut() {
            if let Some(target_value) = root_table.get_mut("target") {
                if let Some(target_table) = target_value.as_table_mut() {
                    // Look for the 'cfg(windows)' key
                    let windows_key = "'cfg(windows)'";
                    if let Some(cfg_value) = target_table.get_mut(windows_key) {
                        if let Some(cfg_table) = cfg_value.as_table_mut() {
                            if let Some(rustflags_value) = cfg_table.get_mut("rustflags") {
                                if let Some(rustflags) = rustflags_value.as_array_mut() {
                                    // Remove flags containing SUBSYSTEM:WINDOWS
                                    rustflags.retain(|flag| {
                                        if let Some(flag_str) = flag.as_str() {
                                            !flag_str.contains("SUBSYSTEM:WINDOWS")
                                        } else {
                                            true
                                        }
                                    });

                                    // Remove the entire cfg(windows) section if rustflags is now empty
                                    if rustflags.is_empty() {
                                        cfg_table.remove("rustflags");
                                        if cfg_table.is_empty() {
                                            target_table.remove(windows_key);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Write back the modified Cargo.toml
        let modified_content = toml::to_string_pretty(&doc)
            .map_err(|e| format!("Failed to serialize Cargo.toml: {}", e))?;

        fs::write(&project_manifest, modified_content)
            .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;

        println!("🔧 Removed Windows subsystem flag for verbose build (console will be visible)");

        Ok(())
    }

    /// Read project name from project.toml, derive a valid Rust crate name, and update
    /// .perro/project/Cargo.toml [package] name and [[bin]] name so the release binary
    /// matches the project name.
    fn sync_project_crate_name_from_project_toml(&self) -> Result<(), String> {
        use std::fs;
        use toml::Value;

        let project_toml_path = self.project_root.join("project.toml");
        if !project_toml_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&project_toml_path)
            .map_err(|e| format!("Failed to read project.toml: {}", e))?;
        let config: Value = content
            .parse()
            .map_err(|e| format!("Invalid project.toml: {}", e))?;
        let project_name = config
            .get("project")
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Perro Game");
        let crate_name = project_name_to_crate_name(project_name);

        let cargo_path = self.project_root.join(".perro/project/Cargo.toml");
        let cargo_content = fs::read_to_string(&cargo_path)
            .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;
        let mut doc: Value = cargo_content
            .parse()
            .map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;

        let mut changed = false;
        if let Some(root) = doc.as_table_mut() {
            if let Some(pkg) = root.get_mut("package").and_then(|v| v.as_table_mut()) {
                if let Some(name_val) = pkg.get("name").and_then(|v| v.as_str()) {
                    if name_val != crate_name {
                        pkg.insert("name".to_string(), Value::String(crate_name.clone()));
                        changed = true;
                    }
                }
            }
            if let Some(bins) = root.get_mut("bin").and_then(|v| v.as_array_mut()) {
                if let Some(first_bin) = bins.first_mut().and_then(|v| v.as_table_mut()) {
                    if let Some(name_val) = first_bin.get("name").and_then(|v| v.as_str()) {
                        if name_val != crate_name {
                            first_bin.insert("name".to_string(), Value::String(crate_name.clone()));
                            changed = true;
                        }
                    }
                }
            }
        }

        if changed {
            let out = toml::to_string_pretty(&doc)
                .map_err(|e| format!("Failed to serialize Cargo.toml: {}", e))?;
            fs::write(&cargo_path, out)
                .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;
            println!(
                "📝 Synced project crate name from project.toml → {}",
                crate_name
            );
        }

        Ok(())
    }

    fn target_name(&self) -> &'static str {
        match self.target {
            CompileTarget::Scripts => "scripts",
            CompileTarget::Project | CompileTarget::VerboseProject => "project",
        }
    }

    pub fn write_key_file(&self, key: &[u8; 32]) -> io::Result<()> {
        // Split key into 4 parts (8 bytes each)
        let mut parts: [[u8; 8]; 4] = [[0; 8]; 4];
        for (i, chunk) in key.chunks(8).enumerate() {
            parts[i].copy_from_slice(chunk);
        }

        // Generate random constants
        let mut consts: [u32; 8] = [0; 8];
        for i in 0..8 {
            consts[i] = rand::random::<u32>();
        }

        // Allowed operations
        let ops = ["^", "+", "-", ">>", "<<"];

        // Prepare mask storage
        let mut mask_exprs: [String; 4] =
            [String::new(), String::new(), String::new(), String::new()];
        let mut mask_values: [u8; 4] = [0; 4];

        // Track constants used
        let mut used_consts = HashSet::new();

        for i in 0..4 {
            let c1 = rand::random::<usize>() % 8;
            let c2 = rand::random::<usize>() % 8;
            used_consts.insert(c1);
            used_consts.insert(c2);

            let op = ops.choose(&mut rand::thread_rng()).unwrap();
            let expr = match *op {
                "^" => {
                    mask_values[i] = (consts[c1] as u8) ^ (consts[c2] as u8);
                    format!("((CONST{} as u8) ^ (CONST{} as u8))", c1 + 1, c2 + 1)
                }
                "+" => {
                    mask_values[i] = (consts[c1] as u8).wrapping_add(consts[c2] as u8);
                    format!(
                        "((CONST{} as u8).wrapping_add(CONST{} as u8))",
                        c1 + 1,
                        c2 + 1
                    )
                }
                "-" => {
                    mask_values[i] = (consts[c1] as u8).wrapping_sub(consts[c2] as u8);
                    format!(
                        "((CONST{} as u8).wrapping_sub(CONST{} as u8))",
                        c1 + 1,
                        c2 + 1
                    )
                }
                ">>" => {
                    mask_values[i] = ((consts[c1] >> 8) as u8) ^ (consts[c2] as u8);
                    format!("((CONST{} >> 8) as u8) ^ (CONST{} as u8)", c1 + 1, c2 + 1)
                }
                "<<" => {
                    mask_values[i] = ((consts[c1] << 3) as u8) ^ (consts[c2] as u8);
                    format!("(((CONST{} << 3) as u8) ^ (CONST{} as u8))", c1 + 1, c2 + 1)
                }
                _ => unreachable!(),
            };

            mask_exprs[i] = expr;
        }

        // Force all consts to be referenced at least once
        for i in 0..8 {
            if !used_consts.contains(&i) {
                let target = rand::random::<usize>() % 4;
                mask_exprs[target] = format!(
                    "({}) ^ (((CONST{} as u8) & 0x{:02X}))",
                    mask_exprs[target],
                    i + 1,
                    rand::random::<u8>()
                );
                used_consts.insert(i);
            }
        }

        // Path to static_assets/key.rs
        let static_assets_dir = self
            .crate_manifest_path
            .parent()
            .unwrap()
            .join("src")
            .join("static_assets");
        fs::create_dir_all(&static_assets_dir)?;

        let key_path = static_assets_dir.join("key.rs");
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&key_path)?;

        writeln!(f, "// Auto-generated by Perro compiler")?;
        writeln!(f, "// This file reconstructs and masks AES key at runtime")?;

        // Write masked parts
        for (i, part) in parts.iter().enumerate() {
            write!(f, "const PART{}: [u8; 8] = [", i + 1)?;
            for (j, b) in part.iter().enumerate() {
                if j > 0 {
                    write!(f, ", ")?;
                }
                let masked = b ^ mask_values[i];
                write!(f, "0x{:02X}", masked)?;
            }
            writeln!(f, "];")?;
        }

        // Write constants (all 8)
        for (i, c) in consts.iter().enumerate() {
            writeln!(
                f,
                "#[allow(dead_code)] const CONST{}: u32 = 0x{:08X};",
                i + 1,
                c
            )?;
        }

        // Write reconstruction function
        writeln!(f, "pub fn get_aes_key() -> [u8; 32] {{")?;
        writeln!(f, "    let mut key = [0u8; 32];")?;
        writeln!(f, "    for i in 0..8 {{")?;
        writeln!(f, "        key[i]      = PART1[i] ^ ({});", mask_exprs[0])?;
        writeln!(f, "        key[i + 8]  = PART2[i] ^ ({});", mask_exprs[1])?;
        writeln!(f, "        key[i + 16] = PART3[i] ^ ({});", mask_exprs[2])?;
        writeln!(f, "        key[i + 24] = PART4[i] ^ ({});", mask_exprs[3])?;
        writeln!(f, "    }}")?;
        writeln!(f, "    key")?;
        writeln!(f, "}}")?;

        Ok(())
    }

    fn codegen_assets(&self, project_crate_root: &Path) -> anyhow::Result<()> {
        // Ensure static_assets directory exists within the project crate
        let static_assets_dir = project_crate_root.join("src").join("static_assets");
        fs::create_dir_all(&static_assets_dir)?;

        println!("🎬 Generating static scene definitions...");
        self.codegen_scenes_file(&static_assets_dir)?;

        println!("📋 Generating static FUR UI definitions...");
        self.codegen_fur_file(&static_assets_dir)?;

        println!("🖼️ Generating static texture definitions...");
        self.codegen_textures_file(&static_assets_dir)?;

        println!("🔷 Generating static mesh definitions...");
        self.codegen_meshes_file(&static_assets_dir)?;

        println!("📝 Generating static Project manifest...");
        self.codegen_manifest_file(&static_assets_dir)?;

        println!("📦 Generating static_assets mod.rs...");
        self.codegen_static_assets_mod(&static_assets_dir)?;

        Ok(())
    }

    fn codegen_static_assets_mod(&self, static_assets_dir: &Path) -> anyhow::Result<()> {
        let mod_file_path = static_assets_dir.join("mod.rs");
        let mut mod_file = File::create(&mod_file_path)?;

        writeln!(mod_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(mod_file, "pub mod key;")?;
        writeln!(mod_file, "pub mod manifest;")?;
        writeln!(mod_file, "pub mod scenes;")?;
        writeln!(mod_file, "pub mod fur;")?;
        writeln!(mod_file, "pub mod textures;")?;
        writeln!(mod_file, "pub mod meshes;")?;

        mod_file.flush()?;
        Ok(())
    }

    fn codegen_main_file(&self, project_crate_root: &Path) -> anyhow::Result<()> {
        let src_dir = project_crate_root.join("src");
        let main_file_path = src_dir.join("main.rs");
        let mut main_file = File::create(&main_file_path)?;

        // Check if this is the editor project (by checking project name or path)
        let is_editor = {
            // Check project name
            if let Ok(project) = crate::manifest::Project::load(Some(&self.project_root)) {
                project.name() == "Perro Engine"
                    || self
                        .project_root
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n == "perro_editor")
                        .unwrap_or(false)
            } else {
                // Fallback: check if path contains "perro_editor"
                self.project_root.to_string_lossy().contains("perro_editor")
            }
        };

        // Add windows_subsystem attribute only for non-verbose project builds
        if !matches!(self.target, CompileTarget::VerboseProject) {
            writeln!(
                main_file,
                "#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = \"windows\")]"
            )?;
        }

        writeln!(main_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(main_file, "")?;
        writeln!(
            main_file,
            "// Embed assets.brk built by compiler/packer in release/export"
        )?;
        writeln!(
            main_file,
            "static ASSETS_BRK: &[u8] = include_bytes!(\"../../../assets.brk\");"
        )?;
        writeln!(main_file, "")?;
        writeln!(main_file, "// Embed icon in binary (Linux only)")?;
        writeln!(main_file, "#[cfg(target_os = \"linux\")]")?;
        writeln!(main_file, "mod embedded_icon {{")?;
        writeln!(
            main_file,
            "    // Include the generated embedded icon module"
        )?;
        writeln!(
            main_file,
            "    // OUT_DIR is set by Cargo and points to the build output directory"
        )?;
        writeln!(
            main_file,
            "    include!(concat!(env!(\"OUT_DIR\"), \"/embedded_icon.rs\"));"
        )?;
        writeln!(main_file, "}}")?;
        writeln!(main_file, "")?;

        if is_editor {
            writeln!(
                main_file,
                "use perro_core::runtime::{{run_game, run_dev_with_path, RuntimeData, StaticAssets}};"
            )?;
            writeln!(main_file, "use std::env;")?;
        } else {
            writeln!(
                main_file,
                "use perro_core::runtime::{{run_game, RuntimeData, StaticAssets}};"
            )?;
        }

        writeln!(main_file, "")?;
        writeln!(main_file, "mod static_assets;")?;
        writeln!(main_file, "")?;
        writeln!(main_file, "fn main() {{")?;

        if is_editor {
            writeln!(
                main_file,
                "    // Editor-specific: Check if --path argument is present - if so, run in dev mode"
            )?;
            writeln!(
                main_file,
                "    // This allows the editor to spawn itself to run a project in dev mode"
            )?;
            writeln!(
                main_file,
                "    let args: Vec<String> = env::args().collect();"
            )?;
            writeln!(
                main_file,
                "    if let Some(i) = args.iter().position(|a| a == \"--path\") {{"
            )?;
            writeln!(
                main_file,
                "        if let Some(path_arg) = args.get(i + 1) {{"
            )?;
            writeln!(
                main_file,
                "            // Resolve the path to an absolute PathBuf"
            )?;
            writeln!(
                main_file,
                "            let project_path = if std::path::Path::new(path_arg).is_absolute() {{"
            )?;
            writeln!(
                main_file,
                "                std::path::PathBuf::from(path_arg)"
            )?;
            writeln!(main_file, "            }} else {{")?;
            writeln!(
                main_file,
                "                match std::fs::canonicalize(path_arg) {{"
            )?;
            writeln!(main_file, "                    Ok(abs_path) => abs_path,")?;
            writeln!(main_file, "                    Err(_) => {{")?;
            writeln!(main_file, "                        env::current_dir()")?;
            writeln!(
                main_file,
                "                            .expect(\"Failed to get current directory\")"
            )?;
            writeln!(main_file, "                            .join(path_arg)")?;
            writeln!(main_file, "                                       }}")?;
            writeln!(main_file, "                }}")?;
            writeln!(main_file, "            }};")?;
            writeln!(main_file, "")?;
            writeln!(
                main_file,
                "            // Run in dev mode with the specified project path"
            )?;
            writeln!(
                main_file,
                "            // All other --arg val pairs will be parsed and added as runtime params"
            )?;
            writeln!(main_file, "            run_dev_with_path(project_path);")?;
            writeln!(main_file, "            return;")?;
            writeln!(main_file, "        }}")?;
            writeln!(main_file, "    }}")?;
            writeln!(main_file, "")?;
        }

        writeln!(
            main_file,
            "    // Parse command-line arguments into runtime parameters"
        )?;
        writeln!(
            main_file,
            "    // Format: --key value (all --arg val pairs become runtime parameters)"
        )?;
        writeln!(
            main_file,
            "    let args: Vec<String> = std::env::args().collect();"
        )?;
        writeln!(
            main_file,
            "    let mut runtime_params = std::collections::HashMap::new();"
        )?;
        writeln!(main_file, "    let mut key: Option<String> = None;")?;
        writeln!(main_file, "    for arg in args.iter().skip(1) {{")?;
        writeln!(main_file, "        if arg.starts_with(\"--\") {{")?;
        writeln!(
            main_file,
            "            let clean_key = arg.trim_start_matches(\"--\").to_string();"
        )?;
        writeln!(main_file, "            key = Some(clean_key);")?;
        writeln!(main_file, "        }} else if let Some(k) = key.take() {{")?;
        writeln!(
            main_file,
            "            runtime_params.insert(k, arg.clone());"
        )?;
        writeln!(main_file, "        }}")?;
        writeln!(main_file, "    }}")?;
        writeln!(main_file, "")?;
        writeln!(
            main_file,
            "    // The root script can check these parameters via api.project().get_runtime_param()"
        )?;
        writeln!(main_file, "    // and decide what to do based on them")?;
        writeln!(main_file, "    run_game(RuntimeData {{")?;
        writeln!(main_file, "        assets_brk: ASSETS_BRK,")?;
        writeln!(
            main_file,
            "        aes_key: static_assets::key::get_aes_key(),"
        )?;
        writeln!(main_file, "        static_assets: StaticAssets {{")?;
        writeln!(
            main_file,
            "            project: &static_assets::manifest::PERRO_PROJECT,"
        )?;
        writeln!(
            main_file,
            "            scenes: &static_assets::scenes::PERRO_SCENES,"
        )?;
        writeln!(
            main_file,
            "            fur: &static_assets::fur::PERRO_FUR,"
        )?;
        writeln!(
            main_file,
            "            textures: &static_assets::textures::PERRO_TEXTURES,"
        )?;
        writeln!(
            main_file,
            "            meshes: &static_assets::meshes::PERRO_MESHES,"
        )?;
        writeln!(main_file, "        }},")?;
        writeln!(
            main_file,
            "        script_registry: scripts::get_script_registry(),"
        )?;
        writeln!(
            main_file,
            "        global_registry_order: scripts::get_global_registry_order(),"
        )?;
        writeln!(
            main_file,
            "        global_registry_names: scripts::get_global_registry_names(),"
        )?;
        writeln!(main_file, "    }}, runtime_params);")?;
        writeln!(main_file, "}}")?;

        main_file.flush()?;
        Ok(())
    }

    fn codegen_build_rs(&self, project_crate_root: &Path) -> anyhow::Result<()> {
        let build_rs_path = project_crate_root.join("build.rs");
        let mut build_file = File::create(&build_rs_path)?;

        writeln!(build_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(build_file, "use std::fs::{{self, OpenOptions}};")?;
        writeln!(build_file, "use std::io::Write;")?;
        writeln!(build_file, "use std::path::{{Path, PathBuf}};")?;
        writeln!(build_file, "use toml::Value;")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "fn main() {{")?;
        writeln!(build_file, "    // Set up logging into build.log")?;
        writeln!(
            build_file,
            "    let manifest_dir = PathBuf::from(env!(\"CARGO_MANIFEST_DIR\"));"
        )?;
        writeln!(build_file, "    let project_root = manifest_dir")?;
        writeln!(build_file, "        .parent()")?;
        writeln!(build_file, "        .expect(\"Failed to get parent\")")?;
        writeln!(build_file, "        .parent()")?;
        writeln!(
            build_file,
            "        .expect(\"Failed to get grandparent\");"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let log_path = project_root.join(\"build.log\");"
        )?;
        writeln!(build_file, "    init_log(&log_path);")?;
        writeln!(
            build_file,
            "    log(&log_path, \"=== Build Script Started ===\");"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Read project.toml")?;
        writeln!(
            build_file,
            "    let project_toml_path = project_root.join(\"project.toml\");"
        )?;
        writeln!(
            build_file,
            "    log(&log_path, &format!(\"Reading {{}}\", project_toml_path.display()));"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let content = fs::read_to_string(&project_toml_path)"
        )?;
        writeln!(
            build_file,
            "        .expect(\"❌ Could not read project.toml\");"
        )?;
        writeln!(
            build_file,
            "    let config: Value = content.parse().expect(\"❌ Invalid project.toml format\");"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let project = config.get(\"project\").expect(\"❌ Missing [project] section\");"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let name = project")?;
        writeln!(build_file, "        .get(\"name\")")?;
        writeln!(build_file, "        .and_then(|v| v.as_str())")?;
        writeln!(build_file, "        .unwrap_or(\"Perro Game\");")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let version = project")?;
        writeln!(build_file, "        .get(\"version\")")?;
        writeln!(build_file, "        .and_then(|v| v.as_str())")?;
        writeln!(build_file, "        .unwrap_or(\"0.1.0\");")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let icon_path = project")?;
        writeln!(build_file, "        .get(\"icon\")")?;
        writeln!(build_file, "        .and_then(|v| v.as_str())")?;
        writeln!(build_file, "        .unwrap_or(\"res://icon.png\");")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    log(&log_path, &format!(\"Project: {{}}\", name));"
        )?;
        writeln!(
            build_file,
            "    log(&log_path, &format!(\"Version: {{}}\", version));"
        )?;
        writeln!(
            build_file,
            "    log(&log_path, &format!(\"Configured icon path: {{}}\", icon_path));"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let real_icon_path = resolve_res_path(project_root.to_path_buf(), icon_path);"
        )?;
        writeln!(
            build_file,
            "    log(&log_path, &format!(\"Resolved icon path: {{}}\", real_icon_path.display()));"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    // Always rerun if these files or env change"
        )?;
        writeln!(
            build_file,
            "    println!(\"cargo:rerun-if-changed={{}}\", project_toml_path.display());"
        )?;
        writeln!(
            build_file,
            "    println!(\"cargo:rerun-if-changed={{}}\", real_icon_path.display());"
        )?;
        writeln!(
            build_file,
            "    println!(\"cargo:rerun-if-env-changed=PERRO_BUILD_TIMESTAMP\");"
        )?;
        writeln!(build_file, "")?;

        // Windows-specific code
        writeln!(build_file, "    #[cfg(target_os = \"windows\")]")?;
        writeln!(build_file, "    {{")?;
        writeln!(
            build_file,
            "        let final_icon = ensure_ico(&real_icon_path, &project_root, &log_path);"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "        if final_icon.exists() {{")?;
        writeln!(
            build_file,
            "            if let Ok(metadata) = fs::metadata(&final_icon) {{"
        )?;
        writeln!(build_file, "                if metadata.len() == 0 {{")?;
        writeln!(
            build_file,
            "                    panic!(\"❌ Icon file is empty: {{}}\", final_icon.display());"
        )?;
        writeln!(build_file, "                }}")?;
        writeln!(build_file, "                log(")?;
        writeln!(build_file, "                    &log_path,")?;
        writeln!(
            build_file,
            "                    &format!(\"✔ Final ICO is valid ({{}} bytes)\", metadata.len()),"
        )?;
        writeln!(build_file, "                );")?;
        writeln!(build_file, "            }}")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "            // Parse semver (major.minor.patch)"
        )?;
        writeln!(
            build_file,
            "            let parts: Vec<&str> = version.split('.').collect();"
        )?;
        writeln!(
            build_file,
            "            let major = parts.get(0).unwrap_or(&\"0\").parse::<u16>().unwrap_or(0);"
        )?;
        writeln!(
            build_file,
            "            let minor = parts.get(1).unwrap_or(&\"0\").parse::<u16>().unwrap_or(0);"
        )?;
        writeln!(
            build_file,
            "            let patch = parts.get(2).unwrap_or(&\"0\").parse::<u16>().unwrap_or(0);"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "            // Build number: from env or fallback"
        )?;
        writeln!(
            build_file,
            "            let build_number: u32 = std::env::var(\"PERRO_BUILD_TIMESTAMP\")"
        )?;
        writeln!(build_file, "                .ok()")?;
        writeln!(
            build_file,
            "                .and_then(|s| s.parse::<u32>().ok())"
        )?;
        writeln!(build_file, "                .unwrap_or_else(|| {{")?;
        writeln!(
            build_file,
            "                    std::time::SystemTime::now()"
        )?;
        writeln!(
            build_file,
            "                        .duration_since(std::time::UNIX_EPOCH)"
        )?;
        writeln!(build_file, "                        .unwrap()")?;
        writeln!(build_file, "                        .as_secs() as u32")?;
        writeln!(build_file, "                }});")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "            let version_display =")?;
        writeln!(
            build_file,
            "                format!(\"{{}}.{{}}.{{}}.{{}}\", major, minor, patch, build_number);"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "            // Create .rc file")?;
        writeln!(
            build_file,
            "            let out_dir = std::env::var(\"OUT_DIR\").unwrap();"
        )?;
        writeln!(
            build_file,
            "            let rc_path = PathBuf::from(&out_dir).join(\"icon.rc\");"
        )?;
        writeln!(
            build_file,
            "            let icon_str = final_icon.to_str().unwrap().replace(\"\\\\\", \"\\\\\\\\\");"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "            let rc_content = format!(")?;
        writeln!(build_file, "    r#\"")?;
        writeln!(build_file, "APPICON_{{}} ICON \"{{}}\"")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "1 VERSIONINFO")?;
        writeln!(build_file, "FILEVERSION {{}},{{}},{{}},{{}}")?;
        writeln!(build_file, "PRODUCTVERSION {{}},{{}},{{}},{{}}")?;
        writeln!(build_file, "BEGIN")?;
        writeln!(build_file, "    BLOCK \"StringFileInfo\"")?;
        writeln!(build_file, "    BEGIN")?;
        writeln!(build_file, "        BLOCK \"040904E4\"")?;
        writeln!(build_file, "        BEGIN")?;
        writeln!(
            build_file,
            "            VALUE \"FileDescription\", \"{{}}\""
        )?;
        writeln!(build_file, "            VALUE \"FileVersion\", \"{{}}\"")?;
        writeln!(build_file, "            VALUE \"ProductName\", \"{{}}\"")?;
        writeln!(build_file, "            VALUE \"ProductVersion\", \"{{}}\"")?;
        writeln!(
            build_file,
            "            VALUE \"OriginalFilename\", \"{{}}.exe\""
        )?;
        writeln!(build_file, "            VALUE \"Engine\", \"Perro\"")?;
        writeln!(
            build_file,
            "            VALUE \"EngineWebsite\", \"https://perroengine.com\""
        )?;
        writeln!(build_file, "        END")?;
        writeln!(build_file, "    END")?;
        writeln!(build_file, "    BLOCK \"VarFileInfo\"")?;
        writeln!(build_file, "    BEGIN")?;
        writeln!(build_file, "        VALUE \"Translation\", 0x0409, 1252")?;
        writeln!(build_file, "    END")?;
        writeln!(build_file, "END")?;
        writeln!(build_file, "\"#,")?;
        writeln!(build_file, "        build_number,")?;
        writeln!(build_file, "        icon_str,")?;
        writeln!(build_file, "        major, minor, patch, build_number,")?;
        writeln!(build_file, "        major, minor, patch, build_number,")?;
        writeln!(build_file, "        name,")?;
        writeln!(build_file, "        version_display,")?;
        writeln!(build_file, "        name,")?;
        writeln!(build_file, "        version_display,")?;
        writeln!(build_file, "        name")?;
        writeln!(build_file, ");")?;
        writeln!(build_file, "")?;

        writeln!(
            build_file,
            "            fs::write(&rc_path, rc_content).expect(\"Failed to write .rc file\");"
        )?;
        writeln!(build_file, "            log(")?;
        writeln!(build_file, "                &log_path,")?;
        writeln!(
            build_file,
            "                &format!(\"✔ Wrote RC with version {{}} (icon ID={{}})\", version_display, build_number),"
        )?;
        writeln!(build_file, "            );")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "            embed_resource::compile(&rc_path, embed_resource::NONE);"
        )?;
        writeln!(
            build_file,
            "            log(&log_path, \"✔ Icon + version resource embedded successfully\");"
        )?;
        writeln!(build_file, "        }} else {{")?;
        writeln!(
            build_file,
            "            panic!(\"⚠ Icon not found at {{}}\", final_icon.display());"
        )?;
        writeln!(build_file, "        }}")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;

        // macOS-specific code
        writeln!(build_file, "    #[cfg(target_os = \"macos\")]")?;
        writeln!(build_file, "    {{")?;
        writeln!(
            build_file,
            "        setup_macos_bundle(&real_icon_path, &project_root, &log_path, name, version);"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;

        // Linux-specific code
        writeln!(build_file, "    #[cfg(target_os = \"linux\")]")?;
        writeln!(build_file, "    {{")?;
        writeln!(
            build_file,
            "        embed_linux_icon(&real_icon_path, &log_path);"
        )?;
        writeln!(
            build_file,
            "        setup_linux_desktop(&real_icon_path, &project_root, &log_path, name, version);"
        )?;
        writeln!(
            build_file,
            "        // Create AppImage (single file with embedded icon) after release builds"
        )?;
        writeln!(
            build_file,
            "        if std::env::var(\"PROFILE\").unwrap_or_default() == \"release\" {{"
        )?;
        writeln!(
            build_file,
            "            create_appimage(&real_icon_path, &project_root, &log_path, name, version);"
        )?;
        writeln!(build_file, "        }}")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;

        writeln!(
            build_file,
            "    log(&log_path, \"=== Build Script Finished ===\");"
        )?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        // Helper functions
        writeln!(build_file, "fn init_log(path: &Path) {{")?;
        writeln!(build_file, "    let _ = fs::remove_file(path);")?;
        writeln!(build_file, "    let mut f = OpenOptions::new()")?;
        writeln!(build_file, "        .create(true)")?;
        writeln!(build_file, "        .write(true)")?;
        writeln!(build_file, "        .truncate(true)")?;
        writeln!(build_file, "        .open(path)")?;
        writeln!(
            build_file,
            "        .expect(\"Failed to create build.log\");"
        )?;
        writeln!(build_file, "    writeln!(f, \"Perro Build Log\").unwrap();")?;
        writeln!(
            build_file,
            "    writeln!(f, \"================\").unwrap();"
        )?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        writeln!(build_file, "fn log(path: &Path, message: &str) {{")?;
        writeln!(build_file, "    println!(\"{{}}\", message);")?;
        writeln!(build_file, "    let mut f = OpenOptions::new()")?;
        writeln!(build_file, "        .create(true)")?;
        writeln!(build_file, "        .append(true)")?;
        writeln!(build_file, "        .open(path)")?;
        writeln!(build_file, "        .expect(\"Failed to open build.log\");")?;
        writeln!(build_file, "    writeln!(f, \"{{}}\", message).unwrap();")?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        // Embed default icon bytes
        // Try to find the default icon relative to perro_core
        let default_icon_path = std::env::current_exe()
            .ok()
            .and_then(|exe| {
                // Find perro_core directory by walking up from executable
                let mut dir = exe.parent()?;
                for _ in 0..15 {
                    let icon_path = dir.join("src").join("resources").join("default-icon.png");
                    if icon_path.exists() {
                        return Some(icon_path);
                    }
                    // Also check if we're in perro_core directory
                    if dir.join("Cargo.toml").exists() {
                        let icon_path = dir.join("src").join("resources").join("default-icon.png");
                        if icon_path.exists() {
                            return Some(icon_path);
                        }
                    }
                    dir = dir.parent()?;
                }
                None
            })
            .or_else(|| {
                // Fallback: try relative to project_root (for workspace setups)
                self.project_root
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| {
                        p.join("perro_core")
                            .join("src")
                            .join("resources")
                            .join("default-icon.png")
                    })
            });

        let icon_bytes_available = if let Some(icon_path) = default_icon_path {
            if icon_path.exists() {
                if let Ok(icon_bytes) = fs::read(&icon_path) {
                    writeln!(build_file, "// Default Perro icon embedded at compile time")?;
                    writeln!(build_file, "const DEFAULT_ICON_BYTES: &[u8] = &[")?;
                    for chunk in icon_bytes.chunks(16) {
                        let bytes_str: Vec<String> =
                            chunk.iter().map(|b| format!("0x{:02X}", b)).collect();
                        writeln!(build_file, "    {},", bytes_str.join(", "))?;
                    }
                    writeln!(build_file, "];")?;
                    writeln!(build_file, "")?;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if !icon_bytes_available {
            // If we can't find or read the default icon, create an empty array (build will fail gracefully)
            writeln!(
                build_file,
                "// Default icon not found - projects must provide their own icon"
            )?;
            writeln!(build_file, "const DEFAULT_ICON_BYTES: &[u8] = &[];")?;
            writeln!(build_file, "")?;
        }

        // Windows functions
        writeln!(build_file, "#[cfg(target_os = \"windows\")]")?;
        writeln!(
            build_file,
            "fn ensure_ico(path: &Path, project_root: &Path, log_path: &Path) -> PathBuf {{"
        )?;
        writeln!(build_file, "    if !path.exists() {{")?;
        writeln!(
            build_file,
            "        log(log_path, &format!(\"⚠ Icon file not found: {{}}, using default Perro icon\", path.display()));"
        )?;
        writeln!(build_file, "        // Use default icon if available")?;
        writeln!(build_file, "        if DEFAULT_ICON_BYTES.is_empty() {{")?;
        writeln!(
            build_file,
            "            panic!(\"❌ Icon file not found: {{}} and no default icon available\", path.display());"
        )?;
        writeln!(build_file, "        }}")?;
        writeln!(
            build_file,
            "        let default_icon_path = project_root.join(\"default-icon-temp.png\");"
        )?;
        writeln!(
            build_file,
            "        fs::write(&default_icon_path, DEFAULT_ICON_BYTES)"
        )?;
        writeln!(
            build_file,
            "            .expect(\"Failed to write default icon\");"
        )?;
        writeln!(
            build_file,
            "        let ico_path = project_root.join(\"icon.ico\");"
        )?;
        writeln!(
            build_file,
            "        convert_any_image_to_ico(&default_icon_path, &ico_path, log_path);"
        )?;
        writeln!(
            build_file,
            "        let _ = fs::remove_file(&default_icon_path); // Clean up temp file"
        )?;
        writeln!(build_file, "        return ico_path;")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let ext = path")?;
        writeln!(build_file, "        .extension()")?;
        writeln!(build_file, "        .and_then(|e| e.to_str())")?;
        writeln!(build_file, "        .unwrap_or(\"\")")?;
        writeln!(build_file, "        .to_lowercase();")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    if ext == \"ico\" {{")?;
        writeln!(
            build_file,
            "        log(log_path, \"Icon is already an ICO file, using directly.\");"
        )?;
        writeln!(build_file, "        return path.to_path_buf();")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let ico_path = project_root.join(\"icon.ico\");"
        )?;
        writeln!(build_file, "    log(")?;
        writeln!(build_file, "        log_path,")?;
        writeln!(
            build_file,
            "        &format!(\"Converting {{}} → {{}}\", path.display(), ico_path.display()),"
        )?;
        writeln!(build_file, "    );")?;
        writeln!(
            build_file,
            "    convert_any_image_to_ico(path, &ico_path, log_path);"
        )?;
        writeln!(build_file, "    ico_path")?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        writeln!(build_file, "#[cfg(target_os = \"windows\")]")?;
        writeln!(
            build_file,
            "fn convert_any_image_to_ico(input_path: &Path, ico_path: &Path, log_path: &Path) {{"
        )?;
        writeln!(
            build_file,
            "    use ico::{{IconDir, IconDirEntry, IconImage, ResourceType}};"
        )?;
        writeln!(build_file, "    use image::io::Reader as ImageReader;")?;
        writeln!(build_file, "    use std::fs::File;")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    if !input_path.exists() {{")?;
        writeln!(
            build_file,
            "        panic!(\"❌ Icon path does NOT exist: {{}}\", input_path.display());"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let img = ImageReader::open(input_path)")?;
        writeln!(build_file, "        .expect(\"Failed to open image\")")?;
        writeln!(build_file, "        .decode()")?;
        writeln!(build_file, "        .expect(\"Failed to decode image\");")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let sizes = [(16, \"icon_16x16.png\"), (32, \"icon_16x16@2x.png\"), (32, \"icon_32x32.png\"), (64, \"icon_32x32@2x.png\"), (128, \"icon_128x128.png\"), (256, \"icon_128x128@2x.png\"), (256, \"icon_256x256.png\"), (512, \"icon_256x256@2x.png\"), (512, \"icon_512x512.png\"), (1024, \"icon_512x512@2x.png\")];"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let mut icon_dir = IconDir::new(ResourceType::Icon);"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    for (size, _filename) in sizes {{")?;
        writeln!(
            build_file,
            "        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);"
        )?;
        writeln!(build_file, "        let rgba = resized.into_rgba8();")?;
        writeln!(build_file, "        let icon_image =")?;
        writeln!(
            build_file,
            "            IconImage::from_rgba_data(size as u32, size as u32, rgba.into_raw());"
        )?;
        writeln!(
            build_file,
            "        icon_dir.add_entry(IconDirEntry::encode(&icon_image).unwrap());"
        )?;
        writeln!(
            build_file,
            "        log(log_path, &format!(\"✔ Added {{}}x{{}} size to ICO\", size, size));"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let mut file = File::create(ico_path).expect(\"Failed to create ICO file\");"
        )?;
        writeln!(build_file, "    icon_dir")?;
        writeln!(build_file, "        .write(&mut file)")?;
        writeln!(build_file, "        .expect(\"Failed to write ICO file\");")?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"✔ ICO saved: {{}}\", ico_path.display()));"
        )?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        // macOS functions
        writeln!(build_file, "#[cfg(target_os = \"macos\")]")?;
        writeln!(
            build_file,
            "fn setup_macos_bundle(icon_path: &Path, project_root: &Path, log_path: &Path, name: &str, version: &str) {{"
        )?;
        writeln!(
            build_file,
            "    let actual_icon_path = if !icon_path.exists() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, &format!(\"⚠ Icon file not found: {{}}, using default Perro icon\", icon_path.display()));"
        )?;
        writeln!(
            build_file,
            "        let default_icon_path = project_root.join(\"default-icon-temp.png\");"
        )?;
        writeln!(
            build_file,
            "        fs::write(&default_icon_path, DEFAULT_ICON_BYTES)"
        )?;
        writeln!(
            build_file,
            "            .expect(\"Failed to write default icon\");"
        )?;
        writeln!(build_file, "        default_icon_path")?;
        writeln!(build_file, "    }} else {{")?;
        writeln!(build_file, "        icon_path.to_path_buf()")?;
        writeln!(build_file, "    }};")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let icns_path = project_root.join(\"icon.icns\");"
        )?;
        writeln!(
            build_file,
            "    convert_to_icns(&actual_icon_path, &icns_path, log_path);"
        )?;
        writeln!(
            build_file,
            "    if actual_icon_path.file_name().and_then(|n| n.to_str()) == Some(\"default-icon-temp.png\") {{"
        )?;
        writeln!(
            build_file,
            "        let _ = fs::remove_file(&actual_icon_path); // Clean up temp file"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let info_plist_path = project_root.join(\"Info.plist\");"
        )?;
        writeln!(build_file, "    let info_plist_content = format!(")?;
        writeln!(
            build_file,
            "        r#\"<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
        )?;
        writeln!(
            build_file,
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">"
        )?;
        writeln!(build_file, "<plist version=\"1.0\">")?;
        writeln!(build_file, "<dict>")?;
        writeln!(build_file, "    <key>CFBundleDisplayName</key>")?;
        writeln!(build_file, "    <string>{{}}</string>")?;
        writeln!(build_file, "    <key>CFBundleExecutable</key>")?;
        writeln!(build_file, "    <string>{{}}</string>")?;
        writeln!(build_file, "    <key>CFBundleIconFile</key>")?;
        writeln!(build_file, "    <string>icon.icns</string>")?;
        writeln!(build_file, "    <key>CFBundleIdentifier</key>")?;
        writeln!(build_file, "    <string>com.perroengine.{{}}</string>")?;
        writeln!(build_file, "    <key>CFBundleInfoDictionaryVersion</key>")?;
        writeln!(build_file, "    <string>6.0</string>")?;
        writeln!(build_file, "    <key>CFBundleName</key>")?;
        writeln!(build_file, "    <string>{{}}</string>")?;
        writeln!(build_file, "    <key>CFBundlePackageType</key>")?;
        writeln!(build_file, "    <string>APPL</string>")?;
        writeln!(build_file, "    <key>CFBundleShortVersionString</key>")?;
        writeln!(build_file, "    <string>{{}}</string>")?;
        writeln!(build_file, "    <key>CFBundleVersion</key>")?;
        writeln!(build_file, "    <string>{{}}</string>")?;
        writeln!(build_file, "    <key>NSHighResolutionCapable</key>")?;
        writeln!(build_file, "    <true/>")?;
        writeln!(build_file, "    <key>Engine</key>")?;
        writeln!(build_file, "    <string>Perro</string>")?;
        writeln!(build_file, "    <key>EngineWebsite</key>")?;
        writeln!(build_file, "    <string>https://perroengine.com</string>")?;
        writeln!(build_file, "</dict>")?;
        writeln!(build_file, "</plist>\"#,")?;
        writeln!(
            build_file,
            "        name, name, name, name, version, version"
        )?;
        writeln!(build_file, "    );")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    fs::write(&info_plist_path, info_plist_content).expect(\"Failed to write Info.plist\");"
        )?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"✔ Created macOS bundle files: {{}}, {{}}\", icns_path.display(), info_plist_path.display()));"
        )?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        writeln!(build_file, "#[cfg(target_os = \"macos\")]")?;
        writeln!(
            build_file,
            "fn convert_to_icns(input_path: &Path, icns_path: &Path, log_path: &Path) {{"
        )?;
        writeln!(build_file, "    use image::io::Reader as ImageReader;")?;
        writeln!(build_file, "    use std::process::Command;")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let temp_iconset = icns_path.with_extension(\"iconset\");"
        )?;
        writeln!(build_file, "    let _ = fs::create_dir_all(&temp_iconset);")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let img = ImageReader::open(input_path)")?;
        writeln!(build_file, "        .expect(\"Failed to open image\")")?;
        writeln!(build_file, "        .decode()")?;
        writeln!(build_file, "        .expect(\"Failed to decode image\");")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let sizes = [(16, \"icon_16x16.png\"), (32, \"icon_16x16@2x.png\"), (32, \"icon_32x32.png\"), (64, \"icon_32x32@2x.png\"), (128, \"icon_128x128.png\"), (256, \"icon_128x128@2x.png\"), (256, \"icon_256x256.png\"), (512, \"icon_256x256@2x.png\"), (512, \"icon_512x512.png\"), (1024, \"icon_512x512@2x.png\")];"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    for (size, filename) in sizes {{")?;
        writeln!(
            build_file,
            "        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);"
        )?;
        writeln!(
            build_file,
            "        let out_path = temp_iconset.join(filename);"
        )?;
        writeln!(
            build_file,
            "        resized.save(&out_path).expect(\"Failed to write PNG in iconset\");"
        )?;
        writeln!(
            build_file,
            "        log(log_path, &format!(\"✔ Added {{}}x{{}} to iconset\", size, size));"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let status = Command::new(\"iconutil\")")?;
        writeln!(
            build_file,
            "        .args([\"-c\", \"icns\", temp_iconset.to_str().expect(\"iconset path\"), \"-o\", icns_path.to_str().expect(\"icns path\")])"
        )?;
        writeln!(build_file, "        .status()")?;
        writeln!(
            build_file,
            "        .expect(\"iconutil not found (macOS only)\");"
        )?;
        writeln!(
            build_file,
            "    if !status.success() {{ panic!(\"iconutil failed\"); }}"
        )?;
        writeln!(build_file, "    let _ = fs::remove_dir_all(&temp_iconset);")?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"✔ ICNS saved: {{}}\", icns_path.display()));"
        )?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        // Linux functions
        writeln!(build_file, "#[cfg(target_os = \"linux\")]")?;
        writeln!(
            build_file,
            "fn embed_linux_icon(icon_path: &Path, log_path: &Path) {{"
        )?;
        writeln!(build_file, "    if !icon_path.exists() {{")?;
        writeln!(
            build_file,
            "        log(log_path, &format!(\"⚠ Icon file not found: {{}}, skipping icon embedding\", icon_path.display()));"
        )?;
        writeln!(build_file, "        return;")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let out_dir = std::env::var(\"OUT_DIR\").unwrap();"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    // Copy icon to OUT_DIR so we can include it"
        )?;
        writeln!(
            build_file,
            "    let icon_in_out_dir = PathBuf::from(&out_dir).join(\"icon.png\");"
        )?;
        writeln!(
            build_file,
            "    fs::copy(icon_path, &icon_in_out_dir).expect(\"Failed to copy icon to OUT_DIR\");"
        )?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"✔ Copied icon to OUT_DIR: {{}}\", icon_in_out_dir.display()));"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    // Generate module that embeds the icon using include_bytes!"
        )?;
        writeln!(
            build_file,
            "    // This will be included in the binary's data section"
        )?;
        writeln!(
            build_file,
            "    let embedded_icon_module = PathBuf::from(&out_dir).join(\"embedded_icon.rs\");"
        )?;
        writeln!(build_file, "    let module_content = format!(")?;
        writeln!(
            build_file,
            "        r#\"// Auto-generated embedded icon module"
        )?;
        writeln!(
            build_file,
            "// Icon is embedded in the binary at compile time"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "/// Embedded application icon (PNG bytes)")?;
        writeln!(
            build_file,
            "/// This icon is embedded directly in the binary's data section"
        )?;
        writeln!(
            build_file,
            "pub static EMBEDDED_ICON: &[u8] = include_bytes!(\"icon.png\");"
        )?;
        writeln!(build_file, "\"#")?;
        writeln!(build_file, "    );")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    fs::write(&embedded_icon_module, module_content).expect(\"Failed to write embedded_icon.rs\");"
        )?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"✔ Generated embedded icon module: {{}}\", embedded_icon_module.display()));"
        )?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "#[cfg(target_os = \"linux\")]")?;
        writeln!(
            build_file,
            "fn setup_linux_desktop(icon_path: &Path, project_root: &Path, log_path: &Path, name: &str, version: &str) {{"
        )?;
        writeln!(
            build_file,
            "    let actual_icon_path = if !icon_path.exists() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, &format!(\"⚠ Icon file not found: {{}}, using default Perro icon\", icon_path.display()));"
        )?;
        writeln!(
            build_file,
            "        let default_icon_path = project_root.join(\"default-icon-temp.png\");"
        )?;
        writeln!(
            build_file,
            "        fs::write(&default_icon_path, DEFAULT_ICON_BYTES)"
        )?;
        writeln!(
            build_file,
            "            .expect(\"Failed to write default icon\");"
        )?;
        writeln!(build_file, "        default_icon_path")?;
        writeln!(build_file, "    }} else {{")?;
        writeln!(build_file, "        icon_path.to_path_buf()")?;
        writeln!(build_file, "    }};")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let icon_name = format!(\"{{}}\", name.to_lowercase().replace(\" \", \"_\"));"
        )?;
        writeln!(
            build_file,
            "    let icon_dest = project_root.join(format!(\"{{}}.png\", icon_name));"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Copy icon to destination")?;
        writeln!(
            build_file,
            "    if fs::copy(&actual_icon_path, &icon_dest).is_err() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, \"⚠ Failed to copy icon to destination\");"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Create desktop file")?;
        writeln!(build_file, "    let desktop_content = format!(")?;
        writeln!(build_file, "        r#\"[Desktop Entry]")?;
        writeln!(build_file, "Name={{}}")?;
        writeln!(build_file, "Exec={{}}")?;
        writeln!(build_file, "Icon={{}}")?;
        writeln!(build_file, "Type=Application")?;
        writeln!(build_file, "Categories=Game;")?;
        writeln!(build_file, "Version={{}}")?;
        writeln!(build_file, "StartupNotify=true")?;
        writeln!(build_file, "Engine=Perro")?;
        writeln!(build_file, "EngineWebsite=https://perroengine.com")?;
        writeln!(build_file, "\"#,")?;
        writeln!(build_file, "        name, binary_name, icon_name, version")?;
        writeln!(build_file, "    );")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    fs::write(appdir.join(\"usr/share/applications\").join(format!(\"{{}}.desktop\", icon_name)), &desktop_content).expect(\"Failed to write desktop file\");"
        )?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"✔ Created Linux desktop files: {{}}, {{}}\", icon_dest.display(), desktop_path.display()));"
        )?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "#[cfg(target_os = \"linux\")]")?;
        writeln!(
            build_file,
            "fn create_appimage(icon_path: &Path, project_root: &Path, log_path: &Path, name: &str, version: &str) {{"
        )?;
        writeln!(build_file, "    use std::process::Command;")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    // Only create AppImage if appimagetool is available"
        )?;
        writeln!(
            build_file,
            "    if Command::new(\"appimagetool\").arg(\"--version\").output().is_err() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, \"⚠ appimagetool not found, skipping AppImage creation\");"
        )?;
        writeln!(
            build_file,
            "        log(log_path, \"  Install with: cargo install cargo-appimage or download from https://github.com/AppImage/AppImageKit\");"
        )?;
        writeln!(build_file, "        return;")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Get binary name from CARGO_BIN_NAME")?;
        writeln!(
            build_file,
            "    let binary_name = std::env::var(\"CARGO_BIN_NAME\")"
        )?;
        writeln!(
            build_file,
            "        .unwrap_or_else(|_| name.to_lowercase().replace(\" \", \"_\"));"
        )?;
        writeln!(
            build_file,
            "    let icon_name = name.to_lowercase().replace(\" \", \"_\");"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Determine build directory")?;
        writeln!(
            build_file,
            "    let profile = std::env::var(\"PROFILE\").unwrap_or_else(|_| \"debug\".to_string());"
        )?;
        writeln!(
            build_file,
            "    let target_dir = std::env::var(\"CARGO_TARGET_DIR\")"
        )?;
        writeln!(
            build_file,
            "        .unwrap_or_else(|_| project_root.join(\"..\").join(\"target\").to_string_lossy().to_string());"
        )?;
        writeln!(
            build_file,
            "    let build_dir = PathBuf::from(&target_dir).join(&profile);"
        )?;
        writeln!(build_file, "    let binary = build_dir.join(&binary_name);")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    if !binary.exists() {{")?;
        writeln!(
            build_file,
            "        log(log_path, &format!(\"⚠ Binary not found at {{}}, skipping AppImage\", binary.display()));"
        )?;
        writeln!(build_file, "        return;")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Create AppDir structure")?;
        writeln!(build_file, "    let appdir = build_dir.join(\"AppDir\");")?;
        writeln!(build_file, "    let _ = fs::remove_dir_all(&appdir);")?;
        writeln!(
            build_file,
            "    fs::create_dir_all(appdir.join(\"usr/bin\")).ok();"
        )?;
        writeln!(
            build_file,
            "    fs::create_dir_all(appdir.join(\"usr/share/applications\")).ok();"
        )?;
        writeln!(
            build_file,
            "    fs::create_dir_all(appdir.join(\"usr/share/icons/hicolor/256x256/apps\")).ok();"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Copy binary")?;
        writeln!(
            build_file,
            "    if fs::copy(&binary, appdir.join(\"usr/bin\").join(&binary_name)).is_err() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, \"⚠ Failed to copy binary to AppDir\");"
        )?;
        writeln!(build_file, "        return;")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Copy icon as .DirIcon and to hicolor")?;
        writeln!(
            build_file,
            "    if fs::copy(icon_path, appdir.join(\".DirIcon\")).is_err() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, \"⚠ Failed to copy icon as .DirIcon\");"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(
            build_file,
            "    if fs::copy(icon_path, appdir.join(\"usr/share/icons/hicolor/256x256/apps\").join(format!(\"{{}}.png\", icon_name))).is_err() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, \"⚠ Failed to copy icon to hicolor\");"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Create desktop file")?;
        writeln!(build_file, "    let desktop_content = format!(")?;
        writeln!(build_file, "        r#\"[Desktop Entry]")?;
        writeln!(build_file, "Name={{}}")?;
        writeln!(build_file, "Exec={{}}")?;
        writeln!(build_file, "Icon={{}}")?;
        writeln!(build_file, "Type=Application")?;
        writeln!(build_file, "Categories=Game;")?;
        writeln!(build_file, "Version={{}}")?;
        writeln!(build_file, "StartupNotify=true")?;
        writeln!(build_file, "Engine=Perro")?;
        writeln!(build_file, "EngineWebsite=https://perroengine.com")?;
        writeln!(build_file, "\"#,")?;
        writeln!(build_file, "        name, binary_name, icon_name, version")?;
        writeln!(build_file, "    );")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    if fs::write(appdir.join(\"usr/share/applications\").join(format!(\"{{}}.desktop\", icon_name)), &desktop_content).is_err() {{"
        )?;
        writeln!(
            build_file,
            "        log(log_path, \"⚠ Failed to write desktop file\");"
        )?;
        writeln!(build_file, "        return;")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    // Create AppImage")?;
        writeln!(
            build_file,
            "    let appimage_name = format!(\"{{}}-{{}}-x86_64.AppImage\", binary_name, version);"
        )?;
        writeln!(
            build_file,
            "    let appimage_path = build_dir.join(&appimage_name);"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"📦 Creating AppImage: {{}}\", appimage_path.display()));"
        )?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    let output = Command::new(\"appimagetool\")"
        )?;
        writeln!(build_file, "        .arg(&appdir)")?;
        writeln!(build_file, "        .arg(&appimage_path)")?;
        writeln!(build_file, "        .output();")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    match output {{")?;
        writeln!(build_file, "        Ok(result) => {{")?;
        writeln!(build_file, "            if result.status.success() {{")?;
        writeln!(build_file, "                // Make executable")?;
        writeln!(
            build_file,
            "                use std::os::unix::fs::PermissionsExt;"
        )?;
        writeln!(
            build_file,
            "                if let Ok(mut perms) = fs::metadata(&appimage_path).map(|m| m.permissions()) {{"
        )?;
        writeln!(build_file, "                    perms.set_mode(0o755);")?;
        writeln!(
            build_file,
            "                    fs::set_permissions(&appimage_path, perms).ok();"
        )?;
        writeln!(build_file, "                }}")?;
        writeln!(
            build_file,
            "                log(log_path, &format!(\"✔ AppImage created: {{}}\", appimage_path.display()));"
        )?;
        writeln!(build_file, "            }} else {{")?;
        writeln!(
            build_file,
            "                let stderr = String::from_utf8_lossy(&result.stderr);"
        )?;
        writeln!(
            build_file,
            "                log(log_path, &format!(\"⚠ AppImage creation failed: {{}}\", stderr));"
        )?;
        writeln!(build_file, "            }}")?;
        writeln!(build_file, "        }}")?;
        writeln!(build_file, "        Err(e) => {{")?;
        writeln!(
            build_file,
            "            log(log_path, &format!(\"⚠ Failed to run appimagetool: {{}}\", e));"
        )?;
        writeln!(build_file, "        }}")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        // Common function
        writeln!(
            build_file,
            "fn resolve_res_path(project_root: PathBuf, res_path: &str) -> PathBuf {{"
        )?;
        writeln!(
            build_file,
            "    if let Some(stripped) = res_path.strip_prefix(\"res://\") {{"
        )?;
        writeln!(
            build_file,
            "        project_root.join(\"res\").join(stripped)"
        )?;
        writeln!(build_file, "    }} else {{")?;
        writeln!(build_file, "        project_root.join(res_path)")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "}}")?;

        build_file.flush()?;
        Ok(())
    }

    fn codegen_scenes_file(&self, static_assets_dir: &Path) -> anyhow::Result<()> {
        use regex::Regex;
        use std::fmt::Write as _;

        /// Emit MetadataValue as Rust (no serde_json in generated project).
        fn emit_metadata_value_as_rust(v: &serde_json::Value) -> String {
            match v {
                serde_json::Value::Null => "MetadataValue::null()".to_string(),
                serde_json::Value::Bool(b) => format!("MetadataValue::bool({})", b),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        format!("MetadataValue::number_i64({}i64)", i)
                    } else if let Some(u) = n.as_u64() {
                        format!("MetadataValue::number_u64({}u64)", u)
                    } else if let Some(f) = n.as_f64() {
                        format!("MetadataValue::number_f64({})", f)
                    } else {
                        "MetadataValue::null()".to_string()
                    }
                }
                serde_json::Value::String(s) => {
                    let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
                    format!("MetadataValue::String(Cow::Borrowed(\"{}\"))", esc)
                }
                serde_json::Value::Array(arr) => {
                    let parts: Vec<String> = arr.iter().map(emit_metadata_value_as_rust).collect();
                    format!("MetadataValue::array(vec![{}])", parts.join(", "))
                }
                serde_json::Value::Object(obj) => {
                    let parts: Vec<String> = obj
                        .iter()
                        .map(|(k, v)| {
                            let k_esc = k.replace('\\', "\\\\").replace('"', "\\\"");
                            let val_rust = emit_metadata_value_as_rust(v);
                            format!("(Cow::Borrowed(\"{}\"), {})", k_esc, val_rust)
                        })
                        .collect();
                    format!(
                        "MetadataValue::object(std::collections::HashMap::from([{}]))",
                        parts.join(", ")
                    )
                }
            }
        }

        /// Emit ScriptExpVarValue constructor calls (no serde_json in generated project).
        fn emit_script_exp_var_value_as_rust(v: &serde_json::Value) -> String {
            match v {
                serde_json::Value::Null => "ScriptExpVarValue::null()".to_string(),
                serde_json::Value::Bool(b) => format!("ScriptExpVarValue::bool({})", b),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        format!("ScriptExpVarValue::number_i64({}i64)", i)
                    } else if let Some(u) = n.as_u64() {
                        format!("ScriptExpVarValue::number_u64({}u64)", u)
                    } else if let Some(f) = n.as_f64() {
                        format!("ScriptExpVarValue::number_f64({})", f)
                    } else {
                        format!("ScriptExpVarValue::number_f64({})", n)
                    }
                }
                serde_json::Value::String(s) => {
                    let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
                    format!("ScriptExpVarValue::string(Cow::Borrowed(\"{}\"))", esc)
                }
                serde_json::Value::Array(arr) => {
                    let parts: Vec<String> =
                        arr.iter().map(emit_script_exp_var_value_as_rust).collect();
                    format!("ScriptExpVarValue::array(vec![{}])", parts.join(", "))
                }
                serde_json::Value::Object(obj) => {
                    let parts: Vec<String> = obj
                        .iter()
                        .map(|(k, v)| {
                            let k_str: &str = k.as_ref();
                            let k_esc = k_str.replace('\\', "\\\\").replace('"', "\\\"");
                            // In JSON, NodeRef is stored as {"@node": scene_key}
                            if let Some(o) = v.as_object() {
                                if o.len() == 1 {
                                    if let Some(n) = o.get("@node").and_then(|n| n.as_u64()) {
                                        let idx = n as u32;
                                        return format!(
                                            "(Cow::Borrowed(\"{}\"), ScriptExpVarValue::NodeRef(NodeID::from_u32({}u32)))",
                                            k_esc, idx
                                        );
                                    }
                                }
                            }
                            let val_rust = emit_script_exp_var_value_as_rust(v);
                            format!("(Cow::Borrowed(\"{}\"), {})", k_esc, val_rust)
                        })
                        .collect();
                    format!("ScriptExpVarValue::object(vec![{}])", parts.join(", "))
                }
            }
        }

        let scenes_output_path = static_assets_dir.join("scenes.rs");
        let mut scenes_file = File::create(&scenes_output_path)?;

        // --- File header ---
        writeln!(scenes_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(scenes_file, "#![allow(clippy::all)]")?;
        writeln!(scenes_file, "#![allow(unused)]")?;
        writeln!(scenes_file, "use perro_core::NodeID;")?;
        writeln!(scenes_file, "use phf::{{phf_map, Map}};")?;
        writeln!(scenes_file, "use perro_core::scene::SceneData;")?;
        writeln!(
            scenes_file,
            "use perro_core::nodes::node::{{MetadataValue, ScriptExpVarValue}};"
        )?;
        writeln!(scenes_file, "use perro_core::structs::*;")?;
        writeln!(scenes_file, "use perro_core::structs2d::Shape2D;")?;
        writeln!(scenes_file, "use perro_core::node_registry::*;")?;
        writeln!(scenes_file, "use perro_core::nodes::*;")?;
        writeln!(scenes_file, "use perro_core::ui_node::UINode;")?;
        writeln!(scenes_file, "use cow_map::cow_map;")?;
        writeln!(
            scenes_file,
            "use std::{{borrow::Cow, collections::{{HashMap, HashSet}}}};"
        )?;
        writeln!(scenes_file, "\n// --- GENERATED SCENE DEFINITIONS ---")?;

        let res_dir = self.project_root.join("res");
        if !res_dir.exists() {
            eprintln!(
                "WARNING: `res` directory not found at {}. No scenes will be compiled.",
                res_dir.display()
            );
            writeln!(
                scenes_file,
                "\n/// A map of scene paths to their statically compiled SceneData blueprints."
            )?;
            writeln!(
                scenes_file,
                "pub static PERRO_SCENES: Map<&'static str, &'static SceneData> = phf_map! {{}};"
            )?;
            scenes_file.flush()?;
            return Ok(());
        }

        let mut processed_scene_paths: HashSet<String> = HashSet::new();
        let mut scene_queue: VecDeque<String> = VecDeque::new();
        let mut static_scene_definitions_code = String::new();
        let mut phf_entries = String::new();

        // --- Walk `res/` for *.scn files ---
        for entry in WalkDir::new(&res_dir) {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "scn") {
                println!("cargo:rerun-if-changed={}", path.display());
                let relative_path = path.strip_prefix(&res_dir)?.to_string_lossy().to_string();
                let res_path = format!("res://{}", relative_path.replace('\\', "/"));
                if processed_scene_paths.insert(res_path.clone()) {
                    scene_queue.push_back(res_path);
                }
            }
        }

        // --- Generate static definitions ---
        while let Some(current_res_path) = scene_queue.pop_front() {
            let local_path = current_res_path.strip_prefix("res://").unwrap();
            let full_fs_path = res_dir.join(local_path);
            if !full_fs_path.exists() {
                eprintln!("Skipping missing {}", full_fs_path.display());
                continue;
            }

            let mut scene_data: SceneData = SceneData::load(&current_res_path)?;
            SceneData::fix_relationships(&mut scene_data);

            // Fix node IDs to use deterministic IDs based on scene keys (NodeID::from_u32(key))
            use crate::ids::NodeID;
            use crate::node_registry::SceneNode;
            let key_to_old_id: std::collections::HashMap<u32, NodeID> = scene_data
                .key_to_node_id()
                .iter()
                .map(|(&k, &id)| (k, id))
                .collect();
            let mut new_key_to_id: std::collections::HashMap<u32, NodeID> =
                std::collections::HashMap::new();

            // CowMap has no iter_mut/values_mut; build owned copy for mutation
            let mut nodes_owned: std::collections::HashMap<u32, SceneNode> = scene_data
                .nodes
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect();

            for (&key, node) in nodes_owned.iter_mut() {
                let new_id = NodeID::from_u32(key);
                node.set_id(new_id);
                new_key_to_id.insert(key, new_id);
            }

            for node in nodes_owned.values_mut() {
                if let Some(parent) = node.get_parent() {
                    let parent_key_opt = key_to_old_id
                        .iter()
                        .find(|&(_, &old_id)| old_id == parent.id)
                        .map(|(&key, _)| key);
                    if let Some(parent_key) = parent_key_opt {
                        if let Some(&new_parent_id) = new_key_to_id.get(&parent_key) {
                            let parent_type = crate::nodes::node::ParentType::new(
                                new_parent_id,
                                parent.node_type,
                            );
                            node.set_parent(Some(parent_type));
                        }
                    }
                }
                let children = node.get_children().to_vec();
                node.clear_children();
                let mut seen_children = HashSet::new();
                for child_id in children {
                    let child_key_opt = key_to_old_id
                        .iter()
                        .find(|&(_, &old_id)| old_id == child_id)
                        .map(|(&key, _)| key);
                    if let Some(child_key) = child_key_opt {
                        if let Some(&new_child_id) = new_key_to_id.get(&child_key) {
                            if seen_children.insert(new_child_id) {
                                node.add_child(new_child_id);
                            }
                        }
                    }
                }
            }

            let static_scene_name = format!(
                "SCENE_{}",
                Self::sanitize_res_path_to_ident(&current_res_path)
            );
            let root_id_str = scene_data.root_key.to_string();

            let mut static_children_slices = String::new();
            let mut nodes_cow_entries = String::new();
            let mut key_to_id_entries = String::new();
            // Emit in deterministic order: root first, then remaining keys sorted (matches scene_key_order).
            let root_key = scene_data.root_key;
            let mut key_order: Vec<u32> = nodes_owned
                .keys()
                .copied()
                .filter(|&k| k != root_key)
                .collect();
            key_order.sort_unstable();
            key_order.insert(0, root_key);
            for key in key_order {
                let node = &nodes_owned[&key];
                let mut node_str = format!("{:#?}", node);

                // --- children: emit static slice and Cow::Borrowed for codegen (no Lazy/vec at runtime) ---
                let children = node.get_children();
                let children_replacement = if children.is_empty() {
                    "children: None".to_string()
                } else {
                    let slice_ident = format!("{}_CHILDREN_{}", static_scene_name, key);
                    let slice_elems: Vec<String> = children
                        .iter()
                        .map(|id| format!("NodeID::from_u32({})", id.index()))
                        .collect();
                    let slice_lit = format!("&[{}]", slice_elems.join(", "));
                    static_children_slices.push_str(&format!(
                        "static {}: &[NodeID] = {};\n",
                        slice_ident, slice_lit
                    ));
                    format!("children: Some(Cow::Borrowed({}))", slice_ident)
                };
                if let Some(children_start) = node_str.find("children: ") {
                    let value_start = children_start + "children: ".len();
                    let rest = &node_str[value_start..];
                    let value_len: usize = if rest.starts_with("None") {
                        4
                    } else if rest.starts_with("Some(") {
                        let mut depth = 1u32;
                        let mut j = 5; // after "Some("
                        let bytes = rest.as_bytes();
                        while j < bytes.len() {
                            match bytes[j] {
                                b'(' => depth += 1,
                                b')' => {
                                    depth = depth.saturating_sub(1);
                                    if depth == 0 {
                                        j += 1;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        j
                    } else {
                        0
                    };
                    if value_len > 0 {
                        let value_end = value_start + value_len;
                        node_str = format!(
                            "{}{}{}",
                            &node_str[..children_start],
                            children_replacement,
                            &node_str[value_end..]
                        );
                    }
                }

                // --- script_exp_vars: emit cow_map!(NAME: ScriptExpVarValue => "key" => value, ...) ---
                if let Some(raw_vars) = node.get_script_exp_vars_raw() {
                    use crate::nodes::node::ScriptExpVarValue;
                    {
                        let entries: Vec<String> = raw_vars
                            .iter()
                            .map(|(k, v): (&&str, &ScriptExpVarValue)| {
                                let k_esc = (*k).replace('\\', "\\\\").replace('"', "\\\"");
                                match v {
                                    ScriptExpVarValue::NodeRef(id) => {
                                        let idx = id.index();
                                        format!(
                                            "\"{}\" => ScriptExpVarValue::NodeRef(NodeID::from_u32({}u32))",
                                            k_esc, idx
                                        )
                                    }
                                    _ => {
                                        let val_rust = emit_script_exp_var_value_as_rust(&v.to_json_value());
                                        format!("\"{}\" => {}", k_esc, val_rust)
                                    }
                                }
                            })
                            .collect();
                        let script_vars_name =
                            format!("{}_NODE_{}_SCRIPT_VARS", static_scene_name, key);
                        let replacement = format!(
                            "Some(cow_map!({}: ScriptExpVarValue =>\n            {}))",
                            script_vars_name,
                            entries.join(",\n            ")
                        );
                        if let Some(start) = node_str.find("script_exp_vars: ") {
                            let value_start = start + "script_exp_vars: ".len();
                            let rest = &node_str[value_start..];
                            let value_len: usize = if rest.trim_start().starts_with("None") {
                                rest.find("None").unwrap_or(0) + 4
                            } else if rest.trim_start().starts_with("Some(") {
                                let some_start = rest.find("Some(").unwrap_or(0);
                                let mut depth = 1u32;
                                let mut j = some_start + 5;
                                while j < rest.len() {
                                    match rest.as_bytes()[j] {
                                        b'(' => depth += 1,
                                        b')' => {
                                            depth -= 1;
                                            if depth == 0 {
                                                j += 1;
                                                break;
                                            }
                                        }
                                        _ => {}
                                    }
                                    j += 1;
                                }
                                j
                            } else {
                                0
                            };
                            if value_len > 0 {
                                let value_end = value_start + value_len;
                                node_str = format!(
                                    "{}{}{}",
                                    &node_str[..value_start],
                                    replacement,
                                    &node_str[value_end..]
                                );
                            }
                        }
                    }
                }

                // --- metadata: emit cow_map!(NAME: MetadataValue => "key" => value, ...) (no serde_json) ---
                if let Some(meta) = node.get_metadata_raw() {
                    use crate::nodes::node::MetadataValue;
                    if !meta.is_empty() {
                        let entries: Vec<String> = meta
                            .iter()
                            .map(|(k, v): (&&str, &MetadataValue)| {
                                let k_esc = (*k).replace('\\', "\\\\").replace('"', "\\\"");
                                let val_rust = emit_metadata_value_as_rust(&v.to_json_value());
                                format!("\"{}\" => {}", k_esc, val_rust)
                            })
                            .collect();
                        let meta_name = format!("{}_NODE_{}_METADATA", static_scene_name, key);
                        let replacement = format!(
                            "Some(cow_map!({}: MetadataValue =>\n            {}))",
                            meta_name,
                            entries.join(",\n            ")
                        );
                        if let Some(start) = node_str.find("metadata: ") {
                            let value_start = start + "metadata: ".len();
                            let rest = &node_str[value_start..];
                            let value_len: usize = if rest.trim_start().starts_with("None") {
                                rest.find("None").unwrap_or(0) + 4
                            } else if rest.trim_start().starts_with("Some(") {
                                let some_start = rest.find("Some(").unwrap_or(0);
                                let mut depth = 1u32;
                                let mut j = some_start + 5;
                                while j < rest.len() {
                                    match rest.as_bytes()[j] {
                                        b'(' => depth += 1,
                                        b')' => {
                                            depth -= 1;
                                            if depth == 0 {
                                                j += 1;
                                                break;
                                            }
                                        }
                                        b'B' if rest[j..].starts_with("Borrowed(") => {
                                            depth += 1;
                                            j += 9; // skip "Borrowed(" so we don't double-count the '('
                                            continue;
                                        }
                                        b'O' if rest[j..].starts_with("Owned(") => {
                                            depth += 1;
                                            j += 6; // skip "Owned(" so we don't double-count the '('
                                            continue;
                                        }
                                        _ => {}
                                    }
                                    j += 1;
                                }
                                j
                            } else {
                                0
                            };
                            if value_len > 0 {
                                let value_end = value_start + value_len;
                                node_str = format!(
                                    "{}{}{}",
                                    &node_str[..value_start],
                                    replacement,
                                    &node_str[value_end..]
                                );
                            }
                        }
                    }
                }

                // (Legacy: we used to wrap raw UUID strings in uuid!("...") here. Node/UI IDs
                // are now NodeID/UIElementID and are fixed up in the ID type step below.)

                // Fix ID type constructor calls. Debug format is IdType(index:generation) which is
                // not valid Rust. Parse index (gen is always 0 at compile time) and emit IdType::from_u32(index).
                let id_types = [
                    "NodeID",
                    "TextureID",
                    "MaterialID",
                    "MeshID",
                    "LightID",
                    "UIElementID",
                ];
                for id_type in &id_types {
                    // Match Debug format IdType(123:0) -> IdType::from_u32(123)
                    let debug_pattern = format!(r"{}\((\d+):\d+\)", id_type);
                    let debug_id_regex = Regex::new(&debug_pattern)?;
                    node_str = debug_id_regex
                        .replace_all(&node_str, &format!("{}::from_u32($1)", id_type))
                        .to_string();
                    // Match single-arg IdType(123) (legacy) -> IdType::from_u32(123)
                    let pattern = format!(r"{}\((\d+)\)", id_type);
                    let id_regex = Regex::new(&pattern)?;
                    node_str = id_regex
                        .replace_all(&node_str, &format!("{}::from_u32($1)", id_type))
                        .to_string();
                }

                // Normalize whitespace and string conversions
                node_str = node_str.replace("Some(\n", "Some(");
                let string_some_regex = Regex::new(r#"Some\(\s*"([^"]+)"\s*,?\s*\)"#)?;
                node_str = string_some_regex
                    .replace_all(&node_str, "Some(Cow::Borrowed(\"$1\"))")
                    .to_string();
                let string_field_regex = Regex::new(r#":\s*"([^"]+)"\s*,"#)?;
                node_str = string_field_regex
                    .replace_all(&node_str, ": Cow::Borrowed(\"$1\"),")
                    .to_string();

                node_str = node_str.replace(": []", ": vec![]");
                // HashSet fields are Option at compile time (None = no allocation)
                node_str =
                    node_str.replace("previous_collisions: {},", "previous_collisions: None,");
             
               
                // Handle other HashMap fields
                node_str = node_str.replace(": {},", ": HashMap::new(),");

                // Fix enum variants that need qualification
                // Shape2D variants (for ShapeInstance2D)
                let shape_type_rectangle_regex =
                    Regex::new(r"shape_type:\s*Some\s*\(\s*Rectangle\s*\{")?;
                let shape_type_circle_regex = Regex::new(r"shape_type:\s*Some\s*\(\s*Circle\s*\{")?;
                let shape_type_square_regex = Regex::new(r"shape_type:\s*Some\s*\(\s*Square\s*\{")?;
                let shape_type_triangle_regex =
                    Regex::new(r"shape_type:\s*Some\s*\(\s*Triangle\s*\{")?;
                node_str = shape_type_rectangle_regex
                    .replace_all(&node_str, "shape_type: Some(Shape2D::Rectangle {")
                    .to_string();
                node_str = shape_type_circle_regex
                    .replace_all(&node_str, "shape_type: Some(Shape2D::Circle {")
                    .to_string();
                node_str = shape_type_square_regex
                    .replace_all(&node_str, "shape_type: Some(Shape2D::Square {")
                    .to_string();
                node_str = shape_type_triangle_regex
                    .replace_all(&node_str, "shape_type: Some(Shape2D::Triangle {")
                    .to_string();

                // Shape2D variants (for CollisionShape2D - now uses Shape2D instead of ColliderShape)
                let shape_rectangle_regex = Regex::new(r"shape:\s*Some\s*\(\s*Rectangle\s*\{")?;
                let shape_circle_regex = Regex::new(r"shape:\s*Some\s*\(\s*Circle\s*\{")?;
                let shape_square_regex = Regex::new(r"shape:\s*Some\s*\(\s*Square\s*\{")?;
                let shape_triangle_regex = Regex::new(r"shape:\s*Some\s*\(\s*Triangle\s*\{")?;
                node_str = shape_rectangle_regex
                    .replace_all(&node_str, "shape: Some(Shape2D::Rectangle {")
                    .to_string();
                node_str = shape_circle_regex
                    .replace_all(&node_str, "shape: Some(Shape2D::Circle {")
                    .to_string();
                node_str = shape_square_regex
                    .replace_all(&node_str, "shape: Some(Shape2D::Square {")
                    .to_string();
                node_str = shape_triangle_regex
                    .replace_all(&node_str, "shape: Some(Shape2D::Triangle {")
                    .to_string();

                // Fix ty and node_type fields: wrap node type names with NodeType::
                // Use engine registry to only wrap actual node types
                let node_type_names: HashSet<String> = ENGINE_REGISTRY
                    .node_defs
                    .keys()
                    .map(|nt| format!("{:?}", nt))
                    .collect();

                // Fix ty: field
                let ty_field_regex = Regex::new(r"ty:\s*(\w+),")?;
                node_str = ty_field_regex
                    .replace_all(&node_str, |caps: &regex::Captures| {
                        let type_name = caps.get(1).unwrap().as_str();
                        if node_type_names.contains(type_name) {
                            format!("ty: NodeType::{type_name},")
                        } else {
                            caps.get(0).unwrap().as_str().to_string()
                        }
                    })
                    .to_string();

                // Fix node_type: field
                let node_type_field_regex = Regex::new(r"node_type:\s*(\w+),")?;
                node_str = node_type_field_regex
                    .replace_all(&node_str, |caps: &regex::Captures| {
                        let type_name = caps.get(1).unwrap().as_str();
                        if node_type_names.contains(type_name) {
                            format!("node_type: NodeType::{type_name},")
                        } else {
                            caps.get(0).unwrap().as_str().to_string()
                        }
                    })
                    .to_string();

                // --- root_ids: safe bracket correction (children use static Cow::Borrowed above) ---
                let regex_root_ids = Regex::new(r"root_ids:\s*Some\s*\(\s*\[")?;
                node_str = regex_root_ids
                    .replace_all(&node_str, "root_ids: Some(vec![")
                    .to_string();

                let regex_root_ids_empty = Regex::new(r"root_ids:\s*Some\s*\(\s*\[\s*\]\s*\)")?;
                node_str = regex_root_ids_empty
                    .replace_all(&node_str, "root_ids: Some(vec![])")
                    .to_string();

                // --- Extract SceneNode variant (for cow_map! nodes and key_to_node_id) ---
                writeln!(
                    &mut key_to_id_entries,
                    "    {}u32 => NodeID::from_u32({}u32),",
                    key, key
                )?;
                if let Some(open_paren) = node_str.find('(') {
                    if let Some(variant_pos) = node_str.find("SceneNode::") {
                        let variant_start = variant_pos + "SceneNode::".len();
                        let variant_end = open_paren;
                        let variant_name = node_str[variant_start..variant_end].trim();

                        let inner_start = open_paren + 1;
                        let inner = node_str[inner_start..]
                            .trim_end()
                            .trim_end_matches(')')
                            .trim();

                        writeln!(
                            &mut nodes_cow_entries,
                            "    {}u32 => SceneNode::{}({}),",
                            key, variant_name, inner
                        )?;
                    } else {
                        writeln!(
                            &mut nodes_cow_entries,
                            "    {}u32 => SceneNode::{},",
                            key,
                            node_str.trim()
                        )?;
                    }
                }
            }

            let nodes_map_name = format!("{}_NODES", static_scene_name);
            let keys_map_name = format!("{}_KEYS", static_scene_name);

            static_scene_definitions_code.push_str(&static_children_slices);
            static_scene_definitions_code.push_str(&format!(
                "
/// Auto-generated static scene for {path}
static {name}: SceneData = SceneData::from_nodes(
    {root_index},
    cow_map!({nodes_map_name}: u32, SceneNode =>
{nodes_entries}
    ),
    cow_map!({keys_map_name}: u32, NodeID =>
{keys_entries}
    )
);
",
                path = current_res_path,
                name = static_scene_name,
                root_index = root_id_str,
                nodes_map_name = nodes_map_name,
                keys_map_name = keys_map_name,
                nodes_entries = nodes_cow_entries,
                keys_entries = key_to_id_entries
            ));

            phf_entries.push_str(&format!(
                "    \"{}\" => &{},\n",
                current_res_path, static_scene_name
            ));
        }

        // --- Write all scene definitions ---
        writeln!(scenes_file, "{}", static_scene_definitions_code)?;

        // --- Write PERRO_SCENES map ---
        writeln!(
            scenes_file,
            "\n/// A map of scene paths to their statically compiled SceneData blueprints."
        )?;
        writeln!(
            scenes_file,
            "pub static PERRO_SCENES: Map<&'static str, &'static SceneData> = phf_map! {{"
        )?;
        write!(scenes_file, "{}", phf_entries)?;
        writeln!(scenes_file, "}};")?;

        scenes_file.flush()?;
        drop(scenes_file);

        // Run rustfmt on generated file for consistent formatting
        if std::process::Command::new("rustfmt")
            .arg(scenes_output_path)
            .status()
            .is_ok()
        {
            // rustfmt ran; ignore exit code (e.g. if file has allow(clippy::all) etc.)
        }

        Ok(())
    }

    fn sanitize_res_path_to_ident(res_path: &str) -> String {
        use std::path::Path;

        // Normalize path separators
        let mut cleaned = res_path.replace('\\', "/");

        // Strip "res://" prefix
        if cleaned.starts_with("res://") {
            cleaned = cleaned.trim_start_matches("res://").to_string();
        }

        // Parse the path to extract parent directory and filename
        let path_obj = Path::new(&cleaned);

        let base_name = path_obj
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let parent_str = path_obj
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .replace('/', "_")
            .replace('-', "_");

        // Build identifier: parent_dir_filename (if parent exists)
        let mut identifier = String::new();
        if !parent_str.is_empty() {
            identifier.push_str(&parent_str);
            identifier.push('_');
        }
        identifier.push_str(&base_name);

        // Sanitize: uppercase and filter to alphanumeric + underscore
        identifier
            .to_uppercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
    }

    fn codegen_fur_file(&self, static_assets_dir: &Path) -> anyhow::Result<()> {
        let fur_output_path = static_assets_dir.join("fur.rs");
        let mut fur_file = File::create(&fur_output_path)?;

        // --- File header ---
        writeln!(fur_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(fur_file, "#![allow(clippy::all)]")?;
        writeln!(fur_file, "#![allow(unused)]")?;
        writeln!(fur_file, "#![allow(non_upper_case_globals)]")?;
        writeln!(
            fur_file,
            "use perro_core::ui::fur_ast::{{FurElement, FurNode}};"
        )?;
        writeln!(fur_file, "use std::borrow::Cow;")?;
        writeln!(fur_file, "use cow_map::cow_map;")?;
        writeln!(fur_file, "use phf::{{phf_map, Map}};")?;
        writeln!(fur_file, "\n// --- GENERATED FUR DEFINITIONS ---")?;
        writeln!(fur_file, "static FUR_EMPTY_NODES: &[FurNode] = &[];")?;

        let res_dir = self.project_root.join("res");
        if !res_dir.exists() {
            eprintln!(
                "WARNING: `res` directory not found at {}. No FUR files will be compiled.",
                res_dir.display()
            );
            writeln!(
                fur_file,
                "\n/// A map of FUR file paths to their statically compiled UI element trees."
            )?;
            writeln!(
                fur_file,
                "pub static PERRO_FUR: Map<&'static str, &'static [FurElement]> = phf_map! {{}};"
            )?;
            fur_file.flush()?;
            return Ok(());
        }

        let mut processed_fur_paths: HashSet<String> = HashSet::new();
        let mut fur_queue: VecDeque<String> = VecDeque::new();
        let mut static_fur_definitions_code = String::new();
        let mut phf_entries = String::new();

        // --- Walk `res/` for *.fur files ---
        for entry in WalkDir::new(&res_dir) {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "fur") {
                println!("cargo:rerun-if-changed={}", path.display());
                let relative_path = path.strip_prefix(&res_dir)?.to_string_lossy().to_string();
                let res_path = format!("res://{}", relative_path.replace('\\', "/"));
                if processed_fur_paths.insert(res_path.clone()) {
                    fur_queue.push_back(res_path);
                }
            }
        }

        // --- Generate static definitions ---
        while let Some(current_res_path) = fur_queue.pop_front() {
            let ast = parse_fur_file(&current_res_path).map_err(|e| {
                anyhow::anyhow!("Failed to parse FUR file {}: {}", current_res_path, e)
            })?;

            let fur_elements: Vec<FurElement> = ast
                .into_iter()
                .filter_map(|f| match f {
                    FurNode::Element(el) => Some(el),
                    _ => None,
                })
                .collect();

            if fur_elements.is_empty() {
                eprintln!("⚠️  No elements found in {}, skipping", current_res_path);
                continue;
            }

            let static_fur_name = format!(
                "FUR_{}",
                Self::sanitize_res_path_to_ident(&current_res_path)
            );

            let mut all_statics = String::new();
            let mut root_element_codes = Vec::new();
            for (idx, element) in fur_elements.iter().enumerate() {
                let prefix = format!("{}_{}", static_fur_name, idx);
                let (statics, element_code) = self.codegen_fur_element_static(element, &prefix)?;
                for s in &statics {
                    all_statics.push_str(s);
                    all_statics.push('\n');
                }
                root_element_codes.push(element_code);
            }

            let root_elems_joined = root_element_codes.join(",\n        ");
            static_fur_definitions_code.push_str(&format!(
                r#"
/// Auto-generated static FUR elements for {path}
{statics}
pub static {name}: &[FurElement] = &[
        {elements}
];
"#,
                path = current_res_path,
                name = static_fur_name,
                statics = all_statics,
                elements = root_elems_joined
            ));

            phf_entries.push_str(&format!(
                "    \"{}\" => {},\n",
                current_res_path, static_fur_name
            ));
        }

        // --- Write all FUR definitions ---
        writeln!(fur_file, "{}", static_fur_definitions_code)?;

        // --- Write PERRO_FUR map ---
        writeln!(
            fur_file,
            "\n/// A map of FUR file paths to their statically compiled UI element trees."
        )?;
        writeln!(
            fur_file,
            "pub static PERRO_FUR: Map<&'static str, &'static [FurElement]> = phf_map! {{"
        )?;
        write!(fur_file, "{}", phf_entries)?;
        writeln!(fur_file, "}};")?;

        fur_file.flush()?;

        Ok(())
    }

    /// Returns (statics to emit, element code string). Statics are in dependency order (children first).
    fn codegen_fur_element_static(
        &self,
        element: &FurElement,
        prefix: &str,
    ) -> anyhow::Result<(Vec<String>, String)> {
        let mut statics = Vec::new();
        let mut child_codes = Vec::new();
        for (i, child) in element.children.iter().enumerate() {
            match child {
                FurNode::Element(child_el) => {
                    let child_prefix = format!("{}_c{}", prefix, i);
                    let (child_statics, child_code) =
                        self.codegen_fur_element_static(child_el, &child_prefix)?;
                    statics.extend(child_statics);
                    child_codes.push(format!("FurNode::Element({})", child_code));
                }
                FurNode::Text(text) => {
                    let esc = text.replace('\\', "\\\\").replace('"', "\\\"");
                    child_codes.push(format!("FurNode::Text(Cow::Borrowed(\"{}\"))", esc));
                }
            }
        }

        let (slice_name, children_static) = if child_codes.is_empty() {
            ("FUR_EMPTY_NODES".to_string(), None)
        } else {
            let name = format!("{}_CHILDREN", prefix.replace('.', "_").replace('-', "_"));
            let s = format!(
                "static {}: &[FurNode] = &[\n        {}];",
                name,
                child_codes.join(",\n        ")
            );
            (name, Some(s))
        };
        if let Some(s) = children_static {
            statics.push(s);
        }

        let attrs_name = format!("{}_ATTRS", prefix.replace('.', "_").replace('-', "_"));
        let attrs_code = if element.attributes.is_empty() {
            format!(
                "cow_map!({}: &'static str, Cow<'static, str> => )",
                attrs_name
            )
        } else {
            let entries: Vec<String> = element
                .attributes
                .iter()
                .map(|(k, v)| {
                    let esc = v.replace('\\', "\\\\").replace('"', "\\\"");
                    format!("\"{}\" => Cow::Borrowed(\"{}\")", k, esc)
                })
                .collect();
            format!(
                "cow_map!({}: &'static str, Cow<'static, str> =>\n            {})",
                attrs_name,
                entries.join(",\n            ")
            )
        };

        let tag_esc = element.tag_name.replace('\\', "\\\\").replace('"', "\\\"");
        let id_esc = element.id.replace('\\', "\\\\").replace('"', "\\\"");
        let element_code = format!(
            "FurElement {{\n        tag_name: Cow::Borrowed(\"{}\"),\n        id: Cow::Borrowed(\"{}\"),\n        attributes: {},\n        children: Cow::Borrowed({}),\n        self_closing: {},\n    }}",
            tag_esc, id_esc, attrs_code, slice_name, element.self_closing
        );

        Ok((statics, element_code))
    }

    fn codegen_manifest_file(&self, static_assets_dir: &Path) -> anyhow::Result<()> {
        let manifest_output_path = static_assets_dir.join("manifest.rs");
        let mut manifest_file = File::create(&manifest_output_path)?;

        // Load the project manifest from project.toml
        let project_toml_path = self.project_root.join("project.toml");
        let project = crate::manifest::Project::load_from_file(&project_toml_path)
            .map_err(|e| anyhow::anyhow!("Failed to load project manifest: {}", e))?;

        // --- File header ---
        writeln!(manifest_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(manifest_file, "#![allow(clippy::all)]")?;
        writeln!(manifest_file, "#![allow(unused)]")?;
        writeln!(manifest_file, "use cow_map::cow_map;")?;
        writeln!(manifest_file, "use std::borrow::Cow;")?;
        writeln!(manifest_file, "use perro_core::manifest::Project;")?;
        writeln!(manifest_file, "\n// --- GENERATED PROJECT MANIFEST ---")?;

        // Generate static metadata cow_map (immutable at runtime; no allocation)
        // Key type &'static str so CowMap get/contains_key/remove work (PhfBorrow).
        let metadata_map_name = "PERRO_METADATA";
        writeln!(
            manifest_file,
            "\nstatic {}: perro_core::manifest::MetadataMap = cow_map!({}: &'static str, Cow<'static, str> =>",
            metadata_map_name, metadata_map_name
        )?;
        if !project.metadata().is_empty() {
            for (key, value) in project.metadata().iter() {
                let v = value.replace('"', "\\\"");
                writeln!(
                    manifest_file,
                    "    \"{}\" => Cow::Borrowed(\"{}\"),",
                    key.replace('\\', "\\\\"),
                    v
                )?;
            }
        }
        writeln!(manifest_file, ");")?;

        // Generate static input actions PHF map
        // Use raw string values from project.toml (not parsed InputSource enum)
        let input_map_name = "PERRO_INPUT_ACTIONS";
        let input_actions = project.get_input_actions();
        if !input_actions.is_empty() {
            writeln!(
                manifest_file,
                "\nstatic {}: phf::Map<&'static str, &'static [&'static str]> = phf::phf_map! {{",
                input_map_name
            )?;
            for (action_name, sources) in input_actions {
                writeln!(manifest_file, "    \"{}\" => &[", action_name)?;
                for source_str in sources {
                    writeln!(manifest_file, "        \"{}\",", source_str)?;
                }
                writeln!(manifest_file, "    ],")?;
            }
            writeln!(manifest_file, "}};")?;
        }

        // Generate non-Lazy static (const fn new_static, all CowMaps/refs)
        writeln!(manifest_file, "\n/// Statically compiled project manifest")?;
        writeln!(
            manifest_file,
            "pub static PERRO_PROJECT: Project = Project::new_static("
        )?;
        writeln!(
            manifest_file,
            "    \"{}\",",
            project.name().replace('\\', "\\\\").replace('"', "\\\"")
        )?;
        writeln!(
            manifest_file,
            "    \"{}\",",
            project.version().replace('\\', "\\\\").replace('"', "\\\"")
        )?;
        writeln!(
            manifest_file,
            "    \"{}\",",
            project
                .main_scene()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        )?;
        if let Some(icon) = project.icon() {
            writeln!(
                manifest_file,
                "    Some(\"{}\"),",
                icon.replace('\\', "\\\\").replace('"', "\\\"")
            )?;
        } else {
            writeln!(manifest_file, "    None,")?;
        }
        writeln!(manifest_file, "    {}f32,", project.fps_cap())?;
        writeln!(manifest_file, "    {}f32,", project.xps())?;
        writeln!(manifest_file, "    {}f32,", project.virtual_width())?;
        writeln!(manifest_file, "    {}f32,", project.virtual_height())?;
        writeln!(
            manifest_file,
            "    \"{}\",",
            project.msaa().replace('\\', "\\\\").replace('"', "\\\"")
        )?;
        writeln!(manifest_file, "    &{},", metadata_map_name)?;
        if !input_actions.is_empty() {
            writeln!(manifest_file, "    &{},", input_map_name)?;
        } else {
            writeln!(manifest_file, "    &phf::phf_map! {{}},")?;
        }
        writeln!(
            manifest_file,
            "    &perro_core::manifest::EMPTY_RUNTIME_PARAMS,"
        )?;
        writeln!(manifest_file, ");")?;

        manifest_file.flush()?;

        Ok(())
    }

    fn codegen_textures_file(&self, static_assets_dir: &Path) -> anyhow::Result<()> {
        let textures_output_path = static_assets_dir.join("textures.rs");
        let mut textures_file = File::create(&textures_output_path)?;

        // --- File header ---
        writeln!(textures_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(textures_file, "#![allow(clippy::all)]")?;
        writeln!(textures_file, "#![allow(unused)]")?;
        writeln!(textures_file, "use phf::{{phf_map, Map}};")?;
        writeln!(
            textures_file,
            "use perro_core::structs2d::texture::StaticTextureData;"
        )?;
        writeln!(
            textures_file,
            "\n// --- .ptex in embedded_assets/textures/ ---"
        )?;

        let res_dir = self.project_root.join("res");
        if !res_dir.exists() {
            eprintln!(
                "WARNING: `res` directory not found at {}. No textures will be compiled.",
                res_dir.display()
            );
            writeln!(textures_file, "\n/// Map of texture paths to .ptex data.")?;
            writeln!(
                textures_file,
                "pub static PERRO_TEXTURES: Map<&'static str, &'static StaticTextureData> = phf_map! {{}};"
            )?;
            textures_file.flush()?;
            return Ok(());
        }

        let project_crate_root = static_assets_dir
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Could not determine project crate root"))?;
        let embedded_assets_root = project_crate_root.join("embedded_assets");
        if embedded_assets_root.exists() {
            fs::remove_dir_all(&embedded_assets_root)?;
        }
        let embedded_assets_dir = embedded_assets_root.join("textures");
        fs::create_dir_all(&embedded_assets_dir)?;

        let mut processed_texture_paths: HashSet<String> = HashSet::new();
        let mut static_texture_definitions_code = String::new();
        let mut phf_entries = String::new();

        let image_extensions = ["png", "jpg", "jpeg", "bmp", "gif", "ico", "tga", "webp"];

        for entry in WalkDir::new(&res_dir) {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if image_extensions.contains(&ext.to_lowercase().as_str()) {
                        println!("cargo:rerun-if-changed={}", path.display());
                        let relative_path =
                            path.strip_prefix(&res_dir)?.to_string_lossy().to_string();
                        let res_path = format!("res://{}", relative_path.replace('\\', "/"));

                        if processed_texture_paths.insert(res_path.clone()) {
                            let img_bytes = std::fs::read(path).map_err(|e| {
                                anyhow::anyhow!("Failed to read image {}: {}", path.display(), e)
                            })?;

                            let img = image::load_from_memory(&img_bytes).map_err(|e| {
                                anyhow::anyhow!("Failed to decode image {}: {}", path.display(), e)
                            })?;

                            let rgba = img.to_rgba8();
                            let (width, height) = img.dimensions();
                            let raw = rgba.as_raw();

                            let compressed = zstd::stream::encode_all(
                                std::io::Cursor::new(raw),
                                ASSET_COMPRESSION.texture,
                            )
                            .map_err(|e| {
                                anyhow::anyhow!("Zstd compress texture {}: {}", path.display(), e)
                            })?;

                            println!(
                                "🖼️ Pre-decode + .ptex texture: {} ({}x{})",
                                res_path, width, height
                            );

                            let ext_upper = ext.to_uppercase();
                            let static_texture_name = Self::sanitize_res_path_to_ident(&res_path);
                            // TEXTURE_ prefix so identifiers never start with a number (valid Rust)
                            let static_texture_name_with_ext =
                                format!("TEXTURE_{}_{}", static_texture_name, ext_upper);

                            // Write to embedded_assets/textures/ only (never res/): .ptex = Perro texture (Zstd-compressed RGBA)
                            let ptex_file_name = format!("{}.ptex", static_texture_name_with_ext);
                            let ptex_file_path = embedded_assets_dir.join(&ptex_file_name);
                            std::fs::write(&ptex_file_path, &compressed).map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to write .ptex texture {}: {}",
                                    ptex_file_path.display(),
                                    e
                                )
                            })?;

                            let include_path =
                                format!("../../embedded_assets/textures/{}", ptex_file_name);
                            static_texture_definitions_code.push_str(&format!(
                                r#"
static {bytes_name}: &[u8] = include_bytes!("{include_path}");

static {name}: StaticTextureData = StaticTextureData {{
    width: {width},
    height: {height},
    image_bytes: {bytes_name},
}};
"#,
                                name = static_texture_name_with_ext,
                                bytes_name = format!("{}_BYTES", static_texture_name_with_ext),
                                include_path = include_path,
                                width = width,
                                height = height,
                            ));

                            phf_entries.push_str(&format!(
                                "    \"{}\" => &{},\n",
                                res_path.replace('\\', "\\\\").replace('"', "\\\""),
                                static_texture_name_with_ext
                            ));
                        }
                    }
                }
            }
        }

        writeln!(textures_file, "{}", static_texture_definitions_code)?;
        writeln!(textures_file, "\n/// Map of texture paths to .ptex data.")?;
        writeln!(
            textures_file,
            "pub static PERRO_TEXTURES: Map<&'static str, &'static StaticTextureData> = phf_map! {{"
        )?;
        write!(textures_file, "{}", phf_entries)?;
        writeln!(textures_file, "}};")?;

        textures_file.flush()?;

        Ok(())
    }

    fn codegen_meshes_file(&self, static_assets_dir: &Path) -> anyhow::Result<()> {
        let meshes_output_path = static_assets_dir.join("meshes.rs");
        let mut meshes_file = File::create(&meshes_output_path)?;

        writeln!(meshes_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(meshes_file, "#![allow(clippy::all)]")?;
        writeln!(meshes_file, "#![allow(unused)]")?;
        writeln!(meshes_file, "use phf::{{phf_map, Map}};")?;
        writeln!(
            meshes_file,
            "use perro_core::structs::structs3d::StaticMeshData;"
        )?;
        writeln!(
            meshes_file,
            "\n// --- .pmesh in embedded_assets/meshes/ (one per mesh, key = model_path or model_path:index) ---"
        )?;

        let res_dir = self.project_root.join("res");
        if !res_dir.exists() {
            writeln!(
                meshes_file,
                "\n/// Map of mesh paths (model.glb or model.glb:index) to .pmesh data."
            )?;
            writeln!(
                meshes_file,
                "pub static PERRO_MESHES: Map<&'static str, &'static StaticMeshData> = phf_map! {{}};"
            )?;
            meshes_file.flush()?;
            return Ok(());
        }

        let project_crate_root = static_assets_dir
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Could not determine project crate root"))?;
        let embedded_assets_dir = project_crate_root.join("embedded_assets").join("meshes");
        fs::create_dir_all(&embedded_assets_dir)?;

        let mut static_mesh_definitions_code = String::new();
        let mut phf_entries = String::new();

        let mesh_extensions = ["gltf", "glb"];

        for entry in WalkDir::new(&res_dir) {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if mesh_extensions.contains(&ext.to_lowercase().as_str()) {
                        println!("cargo:rerun-if-changed={}", path.display());
                        let relative_path =
                            path.strip_prefix(&res_dir)?.to_string_lossy().to_string();
                        let res_path = format!("res://{}", relative_path.replace('\\', "/"));

                        let bytes = std::fs::read(path).map_err(|e| {
                            anyhow::anyhow!("Failed to read model {}: {}", path.display(), e)
                        })?;

                        let meshes = crate::rendering::mesh_loader::load_gltf_model_all_meshes(
                            &bytes,
                        )
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Failed to parse GLTF model {} (need GLB with embedded bin)",
                                path.display()
                            )
                        })?;

                        let num_meshes = meshes.len();
                        let base_static_name = Self::sanitize_res_path_to_ident(&res_path);
                        let ext_upper = ext.to_uppercase();

                        for (idx, (_mesh_name, vertices, indices)) in meshes.into_iter().enumerate()
                        {
                            // Index-based naming; MESH_ prefix so identifiers never start with a number (valid Rust).
                            let static_mesh_name_with_ext = if num_meshes == 1 {
                                format!("MESH_{}_{}", base_static_name, ext_upper)
                            } else {
                                format!("MESH_{}_{}_{}", base_static_name, idx, ext_upper)
                            };

                            let (bounds_center, bounds_radius) =
                                crate::rendering::renderer_3d::Renderer3D::compute_bounds(
                                    &vertices,
                                );

                            let vertex_bytes = bytemuck::cast_slice::<_, u8>(&vertices);
                            let index_bytes = bytemuck::cast_slice::<_, u8>(&indices);
                            let combined: Vec<u8> = vertex_bytes
                                .iter()
                                .chain(index_bytes.iter())
                                .copied()
                                .collect();
                            let display_key = if num_meshes == 1 {
                                res_path.clone()
                            } else {
                                format!("{}:{}", res_path, idx)
                            };
                            let compressed = zstd::stream::encode_all(
                                std::io::Cursor::new(&combined),
                                ASSET_COMPRESSION.mesh,
                            )
                            .map_err(|e| {
                                anyhow::anyhow!("Zstd compress mesh {}: {}", display_key, e)
                            })?;

                            println!(
                                "🔷 Pre-decode + .pmesh mesh: {} ({} verts, {} indices)",
                                display_key,
                                vertices.len(),
                                indices.len()
                            );

                            let pmesh_file_name = format!("{}.pmesh", static_mesh_name_with_ext);
                            let pmesh_file_path = embedded_assets_dir.join(&pmesh_file_name);
                            std::fs::write(&pmesh_file_path, &compressed).map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to write .pmesh {}: {}",
                                    pmesh_file_path.display(),
                                    e
                                )
                            })?;

                            let include_path =
                                format!("../../embedded_assets/meshes/{}", pmesh_file_name);
                            // Emit float literals (0.0 not 0) so bounds_center/bounds_radius are [f32; 3] and f32
                            let bc0 = format!("{:?}", bounds_center.x);
                            let bc1 = format!("{:?}", bounds_center.y);
                            let bc2 = format!("{:?}", bounds_center.z);
                            let br = format!("{:?}", bounds_radius);
                            static_mesh_definitions_code.push_str(&format!(
                                r#"
static {bytes_name}: &[u8] = include_bytes!("{include_path}");

static {name}: StaticMeshData = StaticMeshData {{
    vertex_count: {vertex_count},
    index_count: {index_count},
    bounds_center: [{bc0}, {bc1}, {bc2}],
    bounds_radius: {br},
    mesh_bytes: {bytes_name},
}};
"#,
                                name = static_mesh_name_with_ext,
                                bytes_name = format!("{}_BYTES", static_mesh_name_with_ext),
                                include_path = include_path,
                                vertex_count = vertices.len(),
                                index_count = indices.len(),
                                bc0 = bc0,
                                bc1 = bc1,
                                bc2 = bc2,
                                br = br,
                            ));

                            // path:index so runtime normalize (model.glb -> model.glb:0) finds one key
                            phf_entries.push_str(&format!(
                                "    \"{}:{}\" => &{},\n",
                                res_path.replace('\\', "\\\\").replace('"', "\\\""),
                                idx,
                                static_mesh_name_with_ext
                            ));
                        }
                    }
                }
            }
        }

        writeln!(meshes_file, "{}", static_mesh_definitions_code)?;
        writeln!(
            meshes_file,
            "\n/// Map of mesh paths (model.glb or model.glb:index) to .pmesh data."
        )?;
        writeln!(
            meshes_file,
            "pub static PERRO_MESHES: Map<&'static str, &'static StaticMeshData> = phf_map! {{"
        )?;
        write!(meshes_file, "{}", phf_entries)?;
        writeln!(meshes_file, "}};")?;

        meshes_file.flush()?;

        Ok(())
    }
}
