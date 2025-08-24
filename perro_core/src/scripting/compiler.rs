use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

pub enum BuildProfile {
    Dev,
    Release,
    Check, // just validate
}

/// Which crate are we compiling?
pub enum CompileTarget {
    Scripts, // .perro/scripts
    Project, // .perro/project
}

pub struct Compiler {
    /// Path to the Cargo.toml of the target crate
    pub crate_manifest_path: PathBuf,
    target: CompileTarget,
}

impl Compiler {
    pub fn new(project_root: &Path, target: CompileTarget) -> Self {
        let manifest = match target {
            CompileTarget::Scripts => project_root
                .join(".perro")
                .join("scripts")
                .join("Cargo.toml"),
            CompileTarget::Project => project_root
                .join(".perro")
                .join("project")
                .join("Cargo.toml"),
        };

        // Canonicalize to normalize separators and resolve symlinks
        let manifest = dunce::canonicalize(&manifest).unwrap_or(manifest);

        Self {
            crate_manifest_path: manifest,
            target,
        }
    }

    /// Pick the fastest available linker for the platform
    fn best_linker() -> &'static str {
        if cfg!(target_os = "linux") {
            "rust-lld"
        } else if cfg!(target_os = "windows") {
            match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
                Ok("gnu") => "gcc",       // MinGW toolchain
                Ok("msvc") => "lld-link", // MSVC toolchain
                _ => "cc",
            }
        } else if cfg!(target_os = "macos") {
            "clang" // safer than rust-lld Mach-O
        } else {
            "cc"
        }
    }

    /// Path to the `should_compile` flag file
    fn flag_path(&self) -> PathBuf {
        self.crate_manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("should_compile")
    }

    fn should_compile(&self) -> bool {
        match fs::read_to_string(self.flag_path()) {
            Ok(contents) => contents.trim().eq_ignore_ascii_case("true"),
            Err(_) => true, // default to true if missing
        }
    }

    fn set_should_compile(&self, value: bool) {
        let _ = fs::write(self.flag_path(), if value { "true\n" } else { "false\n" });
    }

    fn build_command(&self, profile: BuildProfile) -> Command {
        let mut cmd = Command::new("cargo");

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
            .env("RUSTFLAGS", format!("-C linker={}", Self::best_linker()))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // 🔑 Force profile based on target
        match self.target {
            CompileTarget::Scripts => {
                // Always hotreload profile for scripts
                cmd.arg("--profile").arg("hotreload");
            }
            CompileTarget::Project => {
                // Always release for project
                cmd.arg("--release");
            }
        }

        cmd
    }

    pub fn spawn(&self, profile: BuildProfile) -> Result<Child, String> {
        // Only skip if target is Scripts
        if matches!(self.target, CompileTarget::Scripts) && !self.should_compile() {
            println!("Nothing to rebuild (should_compile == false)");
            return Err("No rebuild needed".into());
        }

        println!("🚀 Spawning compiler for {:?}", self.target_name());
        self.build_command(profile)
            .spawn()
            .map_err(|e| format!("Failed to spawn cargo: {e}"))
    }

    pub fn compile(&self, profile: BuildProfile) -> Result<(), String> {
        // Only skip if target is Scripts
        if matches!(self.target, CompileTarget::Scripts) && !self.should_compile() {
            println!("Nothing to rebuild (should_compile == false)");
            return Ok(());
        }

        println!("Starting compilation of {:?} crate…", self.target_name());
        println!("Looking for manifest at: {}", self.crate_manifest_path.display());
        println!("Exists? {}", self.crate_manifest_path.exists());

        let start = Instant::now();

        let status = self
            .build_command(profile)
            .status()
            .map_err(|e| format!("Failed to run cargo: {e}"))?;

        let elapsed = start.elapsed();

        if status.success() {
            println!("✅ Compilation successful! (total {:.2?})", elapsed);

            // Only reset should_compile for scripts
            if matches!(self.target, CompileTarget::Scripts) {
                self.set_should_compile(false);
            }

            Ok(())
        } else {
            Err(format!("❌ Compilation failed after {:.2?}", elapsed))
        }
    }

    fn target_name(&self) -> &'static str {
        match self.target {
            CompileTarget::Scripts => "scripts",
            CompileTarget::Project => "project",
        }
    }
}