use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::RngCore;
use rand::seq::SliceRandom;

use crate::asset_io::{resolve_path, ResolvedPath};
use crate::brk::build_brk;

#[derive(Debug, Clone)]
pub enum BuildProfile {
    Dev,
    Release,
    Check, // just validate
}

pub enum CompileTarget {
    Scripts, // .perro/scripts
    Project, // .perro/project
    VerboseProject
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
            CompileTarget::Project | CompileTarget::VerboseProject => project_root.join(".perro/project/Cargo.toml"),
        };

        let manifest = dunce::canonicalize(&manifest).unwrap_or(manifest);

        let mut compiler = Self {
            crate_manifest_path: manifest,
            target,
            toolchain_root: None,
            platform: Platform::current(),
            toolchain_version: None,
            project_root: project_root.to_path_buf(),
            from_source
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
                eprintln!("📋 Found toolchain version in project metadata: {}", toolchain_version);
                self.toolchain_version = Some(toolchain_version.to_string());
                
                if self.toolchain_root.is_none() {
                    match resolve_path("user://toolchains") {
                        ResolvedPath::Disk(path_buf) => {
                            self.toolchain_root = Some(path_buf);
                        }
                        ResolvedPath::Brk(_) => {
                            eprintln!("⚠️  user://toolchains resolved to BRK path, falling back to project-relative");
                            let toolchain_root = self.project_root.join(".perro").join("toolchains");
                            self.toolchain_root = Some(toolchain_root);
                        }
                    }
                }
            }
        }
    }

    fn get_toolchain_dir(&self) -> Option<PathBuf> {
        let version = self.toolchain_version.as_deref().unwrap_or("1.90.0");
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

    fn build_command(&self, profile: BuildProfile) -> Result<Command, String> {
        let mut cmd = if self.from_source {
            eprintln!("🔧 Using system cargo (debug mode)");
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

        match self.target {
            CompileTarget::Scripts => {
                cmd.arg("--profile").arg("hotreload");
            }
            CompileTarget::Project | CompileTarget::VerboseProject => {
                match profile {
                    BuildProfile::Dev => cmd.arg("--profile").arg("dev"),
                    BuildProfile::Release => cmd.arg("--release"),
                    BuildProfile::Check => &mut cmd,
                };
                
                cmd.env("PERRO_BUILD_TIMESTAMP", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string());
            }
        }

        Ok(cmd)
    }

    pub fn compile(&self, profile: BuildProfile) -> Result<(), String> {
        if matches!(self.target, CompileTarget::Project) {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);

            println!("🔑 Compile-time AES key: {:02X?}", key);

            // Time key file writing (usually very fast, but good for completeness)
            let key_write_start = Instant::now();
            self.write_key_file(&key).map_err(|e| e.to_string())?;
            let key_write_elapsed = key_write_start.elapsed();
            println!("✔ Key file written (total {:.2?})", key_write_elapsed);


            let res_dir = self.project_root.join("res");
            let output = self.project_root.join("assets.brk");

            // --- TIME THE BRK BUILD HERE ---
            println!("📦 Building BRK archive from {}...", res_dir.display());
            let brk_build_start = Instant::now();
            build_brk(&output, &res_dir, &self.project_root, &key)
                .map_err(|e| e.to_string())?;
            let brk_build_elapsed = brk_build_start.elapsed();
            println!("✅ BRK archive built (total {:.2?})", brk_build_elapsed);
            // --- END BRK TIMING ---
        }

        let toolchain_info = if self.from_source {
            "system (local development)".to_string()
        } else {
            let version = self.toolchain_version.as_deref().unwrap_or("1.83.0");
            let toolchain_name = self.platform.toolchain_name(version);
            
            self.get_toolchain_dir()
                .map(|p| format!("{} ({})", toolchain_name, p.display()))
                .unwrap_or_else(|| "system (fallback)".to_string())
        };

        println!("🚀 Compiling {:?} [{:?}] with toolchain: {}", 
            self.target_name(), 
            profile,
            toolchain_info
        );
        
        let start = Instant::now();
        let mut cmd = self.build_command(profile)?;
        let status = cmd
            .status()
            .map_err(|e| format!("Failed to run cargo: {e}"))?;
        let elapsed = start.elapsed();

        if status.success() {
            println!("✅ Compilation successful! (total {:.2?})", elapsed);
            Ok(())
        } else {
            Err(format!("❌ Compilation failed after {:.2?}", elapsed))
        }
    }

    fn target_name(&self) -> &'static str {
        match self.target {
            CompileTarget::Scripts => "scripts",
            CompileTarget::Project | CompileTarget::VerboseProject => "project",
        }
    }

    fn write_key_file(&self, key: &[u8; 32]) -> std::io::Result<()> {
        // Split into 4 parts of 8 bytes (fixed-size array instead of Vec)
        let mut parts: [[u8; 8]; 4] = [[0; 8]; 4];
        for (i, chunk) in key.chunks(8).enumerate() {
            parts[i].copy_from_slice(chunk);
        }

        // Generate 8 random constants (fixed-size array instead of Vec)
        let mut consts: [u32; 8] = [0; 8];
        for i in 0..8 {
            consts[i] = rand::random::<u32>();
        }

        // Random operations (unchanged, as ops is a static array)
        let ops = ["^", "+", "-", ">>", "<<"];

        // Build mask expressions (runtime code) and mask values (fixed-size arrays)
        // Note: mask_exprs still needs heap allocation for String content,
        // but the container itself is now fixed size.
        let mut mask_exprs: [String; 4] = [
            String::new(), String::new(), String::new(), String::new()
        ];
        let mut mask_values: [u8; 4] = [0; 4];

        for i in 0..4 { // Loop 4 times as there are 4 parts
            let c1 = rand::random::<usize>() % 8;
            let c2 = rand::random::<usize>() % 8;
            let op = ops.choose(&mut rand::thread_rng()).unwrap();

            let expr = match *op {
                "^" => {
                    mask_values[i] = (consts[c1] as u8) ^ (consts[c2] as u8);
                    format!("((CONST{} as u8) ^ (CONST{} as u8))", c1 + 1, c2 + 1)
                }
                "+" => {
                    mask_values[i] = (consts[c1] as u8).wrapping_add(consts[c2] as u8);
                    format!("((CONST{} as u8).wrapping_add(CONST{} as u8))", c1 + 1, c2 + 1)
                }
                "-" => {
                    mask_values[i] = (consts[c1] as u8).wrapping_sub(consts[c2] as u8);
                    format!("((CONST{} as u8).wrapping_sub(CONST{} as u8))", c1 + 1, c2 + 1)
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

            mask_exprs[i] = expr; // Assign to the fixed-size array
        }

        // Path to key.rs
        let key_path = self
            .crate_manifest_path
            .parent()
            .unwrap()
            .join("src")
            .join("key.rs");

        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&key_path)?;

        writeln!(f, "// Auto-generated by Perro compiler")?;

        // Write masked parts
        for (i, part) in parts.iter().enumerate() {
            write!(f, "const PART{}: [u8; 8] = [", i + 1)?;
            for (j, b) in part.iter().enumerate() {
                if j > 0 {
                    write!(f, ", ")?;
                }
                // Apply mask at compile time
                let masked = b ^ mask_values[i]; // mask_values access now matches part index
                write!(f, "0x{:02X}", masked)?;
            }
            writeln!(f, "];")?;
        }

        // Write constants
        for (i, c) in consts.iter().enumerate() {
            writeln!(f, "const CONST{}: u32 = 0x{:08X};", i + 1, c)?;
        }

        // Write get_aes key with inlined mask reconstruction
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
}