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
        "scripts.dylib"
    } else {
        "scripts.so"
    }
}

pub struct Compiler {
    pub crate_manifest_path: PathBuf,
    target: CompileTarget,
    toolchain_root: Option<PathBuf>,
    platform: Platform,
    toolchain_version: Option<String>,
    project_root: PathBuf,
    from_source: bool,
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
        };

        compiler.load_toolchain_config();
        compiler
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
        let version = self.toolchain_version.as_deref().unwrap_or("1.92.0");
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

        let version = self.toolchain_version.as_deref().unwrap_or("1.92.0");
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
                        let target_dir_abs = dunce::canonicalize(&target_dir)
                            .unwrap_or_else(|_| {
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

        eprintln!("⚠️  Could not find parent workspace root (searched from: {})", perro_dir.display());
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

        // Platform-specific library naming
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
        let mut cmd = if self.from_source {
            eprintln!("🔧 Using system cargo (source code mode)");
            Command::new("cargo")
        } else {
            // Try to use toolchain cargo, fallback to system
            if let Some(cargo_path) = self.get_cargo_path() {
                if cargo_path.exists() {
                    eprintln!("✅ Using toolchain cargo: {}", cargo_path.display());
                    Command::new(cargo_path)
                } else {
                    eprintln!("⚠️  Toolchain cargo not found, using system cargo");
                    Command::new("cargo")
                }
            } else {
                eprintln!("🔧 Using system cargo (no custom toolchain)");
                Command::new("cargo")
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
            .arg(num_cpus.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        
        // Set working directory to the manifest directory so relative paths resolve correctly
        if let Some(manifest_dir) = self.crate_manifest_path.parent() {
            cmd.current_dir(manifest_dir);
        }

        // Set CARGO_TARGET_DIR to control where cargo builds
        if let Some(target_dir) = self.get_cargo_target_dir() {
            // Canonicalize to ensure absolute path (required for CARGO_TARGET_DIR)
            let target_dir_abs = dunce::canonicalize(&target_dir)
                .unwrap_or_else(|_| target_dir.clone());
            
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
                let project_crate_root = project_manifest
                    .parent()
                    .expect("Project crate manifest has no parent");
                println!("⚙️ Generating static scene code for project crate...");
                let codegen_start = Instant::now();

                self.codegen_assets(project_crate_root)
                    .map_err(|e| format!("Asset codegen failed: {}", e))?;

                // Generate main.rs
                self.codegen_main_file(project_crate_root)
                    .map_err(|e| format!("Main.rs generation failed: {}", e))?;

                // Generate build.rs
                self.codegen_build_rs(project_crate_root)
                    .map_err(|e| format!("Build.rs generation failed: {}", e))?;

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
            let version = self.toolchain_version.as_deref().unwrap_or("1.92.0");
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

        mod_file.flush()?;
        Ok(())
    }

    fn codegen_main_file(&self, project_crate_root: &Path) -> anyhow::Result<()> {
        let src_dir = project_crate_root.join("src");
        let main_file_path = src_dir.join("main.rs");
        let mut main_file = File::create(&main_file_path)?;

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
        writeln!(
            main_file,
            "use perro_core::runtime::{{run_game, RuntimeData, StaticAssets}};"
        )?;
        writeln!(main_file, "")?;
        writeln!(main_file, "mod static_assets;")?;
        writeln!(main_file, "")?;
        writeln!(main_file, "fn main() {{")?;
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
        writeln!(main_file, "        }},")?;
        writeln!(
            main_file,
            "        script_registry: scripts::get_script_registry(),"
        )?;
        writeln!(main_file, "    }});")?;
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
        writeln!(build_file, "    build_number,")?;
        writeln!(build_file, "    icon_str,")?;
        writeln!(build_file, "    major, minor, patch, build_number,")?;
        writeln!(build_file, "    major, minor, patch, build_number,")?;
        writeln!(build_file, "    name,")?;
        writeln!(build_file, "    version_display,")?;
        writeln!(build_file, "    name,")?;
        writeln!(build_file, "    version_display,")?;
        writeln!(build_file, "    name")?;
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
            "        setup_linux_desktop(&real_icon_path, &project_root, &log_path, name, version);"
        )?;
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

        if let Some(icon_path) = default_icon_path {
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
                }
            }
        } else {
            // If we can't find the default icon, create an empty array (build will fail gracefully)
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
        writeln!(build_file, "    let sizes = [16, 32, 48, 256];")?;
        writeln!(
            build_file,
            "    let mut icon_dir = IconDir::new(ResourceType::Icon);"
        )?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    for size in sizes {{")?;
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
            "        let output_path = temp_iconset.join(filename);"
        )?;
        writeln!(
            build_file,
            "        resized.save(&output_path).expect(\"Failed to save icon size\");"
        )?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let output = Command::new(\"iconutil\")")?;
        writeln!(build_file, "        .args(&[\"-c\", \"icns\", \"-o\"])")?;
        writeln!(build_file, "        .arg(icns_path)")?;
        writeln!(build_file, "        .arg(&temp_iconset)")?;
        writeln!(build_file, "        .output();")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    match output {{")?;
        writeln!(
            build_file,
            "        Ok(result) if result.status.success() => {{"
        )?;
        writeln!(
            build_file,
            "            log(log_path, &format!(\"✔ Created ICNS: {{}}\", icns_path.display()));"
        )?;
        writeln!(build_file, "        }}")?;
        writeln!(build_file, "        _ => {{")?;
        writeln!(
            build_file,
            "            log(log_path, \"⚠ iconutil failed, fallback to PNG copy\");"
        )?;
        writeln!(
            build_file,
            "            fs::copy(input_path, icns_path.with_extension(\"png\")).ok();"
        )?;
        writeln!(build_file, "        }}")?;
        writeln!(build_file, "    }}")?;
        writeln!(build_file, "")?;
        writeln!(build_file, "    let _ = fs::remove_dir_all(&temp_iconset);")?;
        writeln!(build_file, "}}")?;
        writeln!(build_file, "")?;

        // Linux functions
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
            "    let icon_dest = project_root.join(format!(\"{{}}.png\", name.to_lowercase().replace(\" \", \"_\")));"
        )?;
        writeln!(
            build_file,
            "    let _ = fs::copy(&actual_icon_path, &icon_dest);"
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
            "    let desktop_path = project_root.join(format!(\"{{}}.desktop\", name.to_lowercase().replace(\" \", \"_\")));"
        )?;
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
        writeln!(
            build_file,
            "        name, name.to_lowercase().replace(\" \", \"_\"), icon_dest.display(), version"
        )?;
        writeln!(build_file, "    );")?;
        writeln!(build_file, "")?;
        writeln!(
            build_file,
            "    fs::write(&desktop_path, desktop_content).expect(\"Failed to write .desktop file\");"
        )?;
        writeln!(
            build_file,
            "    log(log_path, &format!(\"✔ Created Linux desktop files: {{}}, {{}}\", icon_dest.display(), desktop_path.display()));"
        )?;
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

        let scenes_output_path = static_assets_dir.join("scenes.rs");
        let mut scenes_file = File::create(&scenes_output_path)?;

        // --- File header ---
        writeln!(scenes_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(scenes_file, "#![allow(clippy::all)]")?;
        writeln!(scenes_file, "use once_cell::sync::Lazy;")?;
        writeln!(scenes_file, "use uuid::{{Uuid,uuid}};")?;
        writeln!(scenes_file, "use indexmap::IndexMap;")?;
        writeln!(scenes_file, "use perro_core::scene::SceneData;")?;
        writeln!(scenes_file, "use perro_core::structs::*;")?;
        writeln!(scenes_file, "use perro_core::node_registry::*;")?;
        writeln!(scenes_file, "use perro_core::nodes::*;")?;
        writeln!(scenes_file, "use perro_core::ui_node::UINode;")?;
        writeln!(scenes_file, "use perro_core::physics::ColliderShape;")?;
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
                "pub static PERRO_SCENES: Lazy<HashMap<&'static str, &'static SceneData>> = Lazy::new(|| {{"
            )?;
            writeln!(scenes_file, "    HashMap::new()")?;
            writeln!(scenes_file, "}});")?;
            scenes_file.flush()?;
            return Ok(());
        }

        let mut processed_scene_paths: HashSet<String> = HashSet::new();
        let mut scene_queue: VecDeque<String> = VecDeque::new();
        let mut static_scene_definitions_code = String::new();
        let mut map_insertions_code = String::new();

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

            let static_scene_name = Self::sanitize_res_path_to_ident(&current_res_path);
            let root_id_str = scene_data.root_id.to_string();

            let mut entries = String::new();
            for (uuid, node) in &scene_data.nodes {
                let mut node_str = format!("{:#?}", node);

                // --- UUID fixups ---
                let uuid_literal_regex = Regex::new(
                    r"\b([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b",
                )?;
                node_str = uuid_literal_regex
                    .replace_all(&node_str, "uuid!(\"$1\")")
                    .to_string();

                // Normalize whitespace and string conversions
                node_str = node_str.replace("Some(\n", "Some(");
                let string_some_regex = Regex::new(r#"Some\(\s*"([^"]+)"\s*,?\s*\)"#)?;
                node_str = string_some_regex
                    .replace_all(&node_str, "Some(Cow::Borrowed(\"$1\"))")
                    .to_string();

                let string_field_regex = Regex::new(r#":\s*"([^"]+)","#)?;
                node_str = string_field_regex
                    .replace_all(&node_str, ": Cow::Borrowed(\"$1\"),")
                    .to_string();

                node_str = node_str.replace(": []", ": vec![]");
                // Handle HashSet fields (like previous_collisions in Area2D)
                node_str = node_str.replace("previous_collisions: {},", "previous_collisions: HashSet::new(),");
                // Handle other HashMap fields
                node_str = node_str.replace(": {},", ": HashMap::new(),");
                
                // Fix enum variants that need qualification
                // ShapeType2D variants (for Shape2D)
                let shape_type_rectangle_regex = Regex::new(r"shape_type:\s*Some\s*\(\s*Rectangle\s*\{")?;
                let shape_type_circle_regex = Regex::new(r"shape_type:\s*Some\s*\(\s*Circle\s*\{")?;
                let shape_type_square_regex = Regex::new(r"shape_type:\s*Some\s*\(\s*Square\s*\{")?;
                let shape_type_triangle_regex = Regex::new(r"shape_type:\s*Some\s*\(\s*Triangle\s*\{")?;
                node_str = shape_type_rectangle_regex
                    .replace_all(&node_str, "shape_type: Some(ShapeType2D::Rectangle {")
                    .to_string();
                node_str = shape_type_circle_regex
                    .replace_all(&node_str, "shape_type: Some(ShapeType2D::Circle {")
                    .to_string();
                node_str = shape_type_square_regex
                    .replace_all(&node_str, "shape_type: Some(ShapeType2D::Square {")
                    .to_string();
                node_str = shape_type_triangle_regex
                    .replace_all(&node_str, "shape_type: Some(ShapeType2D::Triangle {")
                    .to_string();
                
                // ColliderShape variants (for CollisionShape2D)
                let collider_shape_rectangle_regex = Regex::new(r"shape:\s*Some\s*\(\s*Rectangle\s*\{")?;
                let collider_shape_circle_regex = Regex::new(r"shape:\s*Some\s*\(\s*Circle\s*\{")?;
                node_str = collider_shape_rectangle_regex
                    .replace_all(&node_str, "shape: Some(ColliderShape::Rectangle {")
                    .to_string();
                node_str = collider_shape_circle_regex
                    .replace_all(&node_str, "shape: Some(ColliderShape::Circle {")
                    .to_string();

                // --- Option<Vec<Uuid>>: safe bracket correction ---
                let regex_children = Regex::new(r"children:\s*Some\s*\(\s*\[")?;
                let regex_root_ids = Regex::new(r"root_ids:\s*Some\s*\(\s*\[")?;
                node_str = regex_children
                    .replace_all(&node_str, "children: Some(vec![")
                    .to_string();
                node_str = regex_root_ids
                    .replace_all(&node_str, "root_ids: Some(vec![")
                    .to_string();

                let regex_children_empty = Regex::new(r"children:\s*Some\s*\(\s*\[\s*\]\s*\)")?;
                let regex_root_ids_empty = Regex::new(r"root_ids:\s*Some\s*\(\s*\[\s*\]\s*\)")?;
                node_str = regex_children_empty
                    .replace_all(&node_str, "children: Some(vec![])")
                    .to_string();
                node_str = regex_root_ids_empty
                    .replace_all(&node_str, "root_ids: Some(vec![])")
                    .to_string();

                // --- Extract SceneNode variant ---
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
                            &mut entries,
                            "        (uuid!(\"{}\"), SceneNode::{}({})),",
                            uuid, variant_name, inner
                        )?;
                    } else {
                        writeln!(
                            &mut entries,
                            "        (uuid!(\"{}\"), SceneNode::{}),",
                            uuid,
                            node_str.trim()
                        )?;
                    }
                }
            }

            let indexmap_formatted = format!("IndexMap::from([\n{}\n    ])", entries);

            static_scene_definitions_code.push_str(&format!(
                "
/// Auto-generated static scene for {path}
static {name}: Lazy<SceneData> = Lazy::new(|| SceneData {{
    root_id: uuid!(\"{root_id}\"),
    nodes: {nodes},
}});
",
                path = current_res_path,
                name = static_scene_name,
                root_id = root_id_str,
                nodes = indexmap_formatted
            ));

            map_insertions_code.push_str(&format!(
                "    m.insert(\"{}\", &*{});\n",
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
            "pub static PERRO_SCENES: Lazy<HashMap<&'static str, &'static SceneData>> = Lazy::new(|| {{"
        )?;
        writeln!(scenes_file, "    let mut m = HashMap::new();")?;
        write!(scenes_file, "{}", map_insertions_code)?;
        writeln!(scenes_file, "    m")?;
        writeln!(scenes_file, "}});")?;

        scenes_file.flush()?;

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
        use std::fmt::Write as _;

        let fur_output_path = static_assets_dir.join("fur.rs");
        let mut fur_file = File::create(&fur_output_path)?;

        // --- File header ---
        writeln!(fur_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(fur_file, "#![allow(clippy::all)]")?;
        writeln!(fur_file, "use once_cell::sync::Lazy;")?;
        writeln!(fur_file, "use uuid::Uuid;")?;
        writeln!(fur_file, "use indexmap::IndexMap;")?;
        writeln!(
            fur_file,
            "use perro_core::ui::fur_ast::{{FurElement, FurNode}};"
        )?;
        writeln!(fur_file, "use std::collections::HashMap;")?;
        writeln!(fur_file, "use std::borrow::Cow;")?;
        writeln!(fur_file, "\n// --- GENERATED FUR DEFINITIONS ---")?;

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
                "pub static PERRO_FUR: Lazy<HashMap<&'static str, &'static [FurElement]>> = Lazy::new(|| {{"
            )?;
            writeln!(fur_file, "    HashMap::new()")?;
            writeln!(fur_file, "}});")?;
            fur_file.flush()?;
            return Ok(());
        }

        let mut processed_fur_paths: HashSet<String> = HashSet::new();
        let mut fur_queue: VecDeque<String> = VecDeque::new();
        let mut static_fur_definitions_code = String::new();
        let mut map_insertions_code = String::new();

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

            let static_fur_name = Self::sanitize_res_path_to_ident(&current_res_path);

            let mut elements_code = String::new();
            for element in &fur_elements {
                let element_code = self.codegen_fur_element(element, 1)?;
                writeln!(&mut elements_code, "        {},", element_code)?;
            }

            static_fur_definitions_code.push_str(&format!(
                r#"
/// Auto-generated static FUR elements for {path}
pub static {name}: Lazy<Vec<FurElement>> = Lazy::new(|| vec![
{elements}
]);
"#,
                path = current_res_path,
                name = static_fur_name,
                elements = elements_code
            ));

            map_insertions_code.push_str(&format!(
                "    m.insert(\"{}\", {}.as_slice());\n",
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
            "pub static PERRO_FUR: Lazy<HashMap<&'static str, &'static [FurElement]>> = Lazy::new(|| {{"
        )?;
        writeln!(fur_file, "    let mut m = HashMap::new();")?;
        write!(fur_file, "{}", map_insertions_code)?;
        writeln!(fur_file, "    m")?;
        writeln!(fur_file, "}});")?;

        fur_file.flush()?;

        Ok(())
    }

    fn codegen_fur_element(
        &self,
        element: &FurElement,
        indent_level: usize,
    ) -> anyhow::Result<String> {
        use std::fmt::Write as _;

        let indent = "    ".repeat(indent_level);
        let mut code = String::new();

        writeln!(&mut code, "{}FurElement {{", indent)?;
        writeln!(
            &mut code,
            "{}    tag_name: Cow::Borrowed(\"{}\"),",
            indent, element.tag_name
        )?;
        writeln!(
            &mut code,
            "{}    id: Cow::Borrowed(\"{}\"),",
            indent, element.id
        )?;

        // Generate attributes HashMap
        if element.attributes.is_empty() {
            writeln!(&mut code, "{}    attributes: HashMap::new(),", indent)?;
        } else {
            writeln!(&mut code, "{}    attributes: HashMap::from([", indent)?;
            for (key, value) in &element.attributes {
                writeln!(
                    &mut code,
                    "{}        (Cow::Borrowed(\"{}\"), Cow::Borrowed(\"{}\")),",
                    indent,
                    key,
                    value.replace("\"", "\\\"")
                )?;
            }
            writeln!(&mut code, "{}    ]),", indent)?;
        }

        // Generate children Vec<FurNode>
        if element.children.is_empty() {
            writeln!(&mut code, "{}    children: vec![],", indent)?;
        } else {
            writeln!(&mut code, "{}    children: vec![", indent)?;
            for child in &element.children {
                match child {
                    FurNode::Element(child_el) => {
                        let child_code = self.codegen_fur_element(child_el, indent_level + 2)?;
                        writeln!(
                            &mut code,
                            "{}        FurNode::Element({}),",
                            indent,
                            child_code.trim()
                        )?;
                    }
                    FurNode::Text(text) => {
                        writeln!(
                            &mut code,
                            "{}        FurNode::Text(Cow::Borrowed(\"{}\")),",
                            indent,
                            text.replace("\"", "\\\"")
                        )?;
                    }
                }
            }
            writeln!(&mut code, "{}    ],", indent)?;
        }

        writeln!(
            &mut code,
            "{}    self_closing: {},",
            indent, element.self_closing
        )?;
        write!(&mut code, "{}}}", indent)?;

        Ok(code)
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
        writeln!(manifest_file, "use once_cell::sync::Lazy;")?;
        writeln!(manifest_file, "use perro_core::manifest::Project;")?;
        writeln!(manifest_file, "\n// --- GENERATED PROJECT MANIFEST ---")?;

        // Generate static metadata PHF map
        let metadata_map_name = "PERRO_METADATA";
        if !project.metadata().is_empty() {
            writeln!(
                manifest_file,
                "\nstatic {}: phf::Map<&'static str, &'static str> = phf::phf_map! {{",
                metadata_map_name
            )?;
            for (key, value) in project.metadata() {
                writeln!(
                    manifest_file,
                    "    \"{}\" => \"{}\",",
                    key,
                    value.replace("\"", "\\\"")
                )?;
            }
            writeln!(manifest_file, "}};")?;
        }

        // Generate the Lazy Project
        writeln!(manifest_file, "\n/// Statically compiled project manifest")?;
        writeln!(
            manifest_file,
            "pub static PERRO_PROJECT: Lazy<Project> = Lazy::new(|| {{"
        )?;
        writeln!(manifest_file, "    Project::new_static(")?;
        writeln!(manifest_file, "        \"{}\".to_string(),", project.name())?;
        writeln!(
            manifest_file,
            "        \"{}\".to_string(),",
            project.version()
        )?;
        writeln!(
            manifest_file,
            "        \"{}\".to_string(),",
            project.main_scene()
        )?;

        // Handle optional icon
        if let Some(icon) = project.icon() {
            writeln!(manifest_file, "        Some(\"{}\".to_string()),", icon)?;
        } else {
            writeln!(manifest_file, "        None,")?;
        }

        writeln!(manifest_file, "        {}f32,", project.target_fps())?;
        writeln!(manifest_file, "        {}f32,", project.xps())?;

        // Handle optional root script
        if let Some(script) = project.root_script() {
            writeln!(manifest_file, "        Some(\"{}\".to_string()),", script)?;
        } else {
            writeln!(manifest_file, "        None,")?;
        }

        // Pass PHF map reference
        if !project.metadata().is_empty() {
            writeln!(manifest_file, "        &{},", metadata_map_name)?;
        } else {
            writeln!(manifest_file, "        &phf::phf_map! {{}},")?;
        }

        writeln!(manifest_file, "    )")?;
        writeln!(manifest_file, "}});")?;

        manifest_file.flush()?;

        Ok(())
    }

    fn codegen_textures_file(&self, static_assets_dir: &Path) -> anyhow::Result<()> {
        let textures_output_path = static_assets_dir.join("textures.rs");
        let mut textures_file = File::create(&textures_output_path)?;

        // --- File header ---
        writeln!(textures_file, "// Auto-generated by Perro Engine compiler")?;
        writeln!(textures_file, "#![allow(clippy::all)]")?;
        writeln!(textures_file, "use once_cell::sync::Lazy;")?;
        writeln!(textures_file, "use std::collections::HashMap;")?;
        writeln!(
            textures_file,
            "use perro_core::structs2d::texture::StaticTextureData;"
        )?;
        writeln!(textures_file, "\n// --- GENERATED TEXTURE DEFINITIONS ---")?;

        let res_dir = self.project_root.join("res");
        if !res_dir.exists() {
            eprintln!(
                "WARNING: `res` directory not found at {}. No textures will be compiled.",
                res_dir.display()
            );
            writeln!(
                textures_file,
                "\n/// A map of texture paths to their statically compiled pre-decoded RGBA8 data."
            )?;
            writeln!(
                textures_file,
                "pub static PERRO_TEXTURES: Lazy<HashMap<&'static str, &'static StaticTextureData>> = Lazy::new(|| {{"
            )?;
            writeln!(textures_file, "    HashMap::new()")?;
            writeln!(textures_file, "}});")?;
            textures_file.flush()?;
            return Ok(());
        }

        // Create embedded_assets directory in project root (outside src/)
        // static_assets_dir is project_crate_root/src/static_assets
        // So project_crate_root is static_assets_dir.parent().parent()
        let project_crate_root = static_assets_dir
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("Could not determine project crate root"))?;
        let embedded_assets_dir = project_crate_root.join("embedded_assets");
        
        // Clean embedded_assets directory at the start to prevent accumulation of old files
        if embedded_assets_dir.exists() {
            fs::remove_dir_all(&embedded_assets_dir)?;
        }
        fs::create_dir_all(&embedded_assets_dir)?;

        let mut processed_texture_paths: HashSet<String> = HashSet::new();
        let mut static_texture_definitions_code = String::new();
        let mut map_insertions_code = String::new();

        // Supported image formats
        let image_extensions = ["png", "jpg", "jpeg", "bmp", "gif", "ico", "tga", "webp"];

        // --- Walk `res/` for image files ---
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
                            // Load and decode image at compile time
                            let img_bytes = std::fs::read(path).map_err(|e| {
                                anyhow::anyhow!("Failed to read image {}: {}", path.display(), e)
                            })?;

                            let img = image::load_from_memory(&img_bytes).map_err(|e| {
                                anyhow::anyhow!("Failed to decode image {}: {}", path.display(), e)
                            })?;

                            // Convert to RGBA8 (same as ImageTexture::from_image does)
                            let rgba = img.to_rgba8();
                            let (width, height) = img.dimensions();

                            println!(
                                "🖼️ Pre-decoding texture: {} ({}x{})",
                                res_path, width, height
                            );

                            // Generate static texture data
                            // Append extension (uppercase) to avoid collisions (e.g., icon.png vs icon.jpg)
                            let ext_upper = ext.to_uppercase();
                            let static_texture_name = Self::sanitize_res_path_to_ident(&res_path);
                            let static_texture_name_with_ext = format!("{}_{}", static_texture_name, ext_upper);

                            // Write RGBA8 bytes to a binary file in embedded_assets/
                            // Use sanitized name with extension for the file to avoid filesystem collisions
                            let rgba_file_name = format!("{}.rgba", static_texture_name_with_ext);
                            let rgba_file_path = embedded_assets_dir.join(&rgba_file_name);
                            std::fs::write(&rgba_file_path, rgba.as_raw()).map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to write RGBA file {}: {}",
                                    rgba_file_path.display(),
                                    e
                                )
                            })?;

                            // Note: Cargo automatically tracks files included via include_bytes!,
                            // so we don't need to add rerun-if-changed for the rgba file.
                            // The source image is already tracked above.

                            // Generate code using include_bytes! macro
                            // Path is relative to textures.rs location (src/static_assets/)
                            // embedded_assets/ is at project root, so relative path is ../../embedded_assets/
                            let include_path = format!("../../embedded_assets/{}", rgba_file_name);
                            static_texture_definitions_code.push_str(&format!(
                                r#"
/// Auto-generated static texture bytes for {path}
/// Loaded from embedded binary file at compile time
static {bytes_name}: &[u8] = include_bytes!("{include_path}");

/// Auto-generated static texture data for {path}
static {name}: StaticTextureData = StaticTextureData {{
    width: {width},
    height: {height},
    rgba8_bytes: {bytes_name},
}};
"#,
                                path = res_path,
                                name = static_texture_name_with_ext,
                                bytes_name = format!("{}_BYTES", static_texture_name_with_ext),
                                include_path = include_path,
                                width = width,
                                height = height,
                            ));

                            map_insertions_code.push_str(&format!(
                                "    m.insert(\"{}\", &{});\n",
                                res_path, static_texture_name_with_ext
                            ));
                        }
                    }
                }
            }
        }

        // --- Write all texture definitions ---
        writeln!(textures_file, "{}", static_texture_definitions_code)?;

        // --- Write PERRO_TEXTURES map ---
        writeln!(
            textures_file,
            "\n/// A map of texture paths to their statically compiled pre-decoded RGBA8 data."
        )?;
        writeln!(
            textures_file,
            "pub static PERRO_TEXTURES: Lazy<HashMap<&'static str, &'static StaticTextureData>> = Lazy::new(|| {{"
        )?;
        writeln!(textures_file, "    let mut m = HashMap::new();")?;
        write!(textures_file, "{}", map_insertions_code)?;
        writeln!(textures_file, "    m")?;
        writeln!(textures_file, "}});")?;

        textures_file.flush()?;

        Ok(())
    }
}
