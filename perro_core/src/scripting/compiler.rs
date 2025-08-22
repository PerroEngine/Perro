use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

pub enum BuildProfile {
    Dev,
    Release,
    Check, // just validate
}

pub struct Compiler {
    /// Path to the perro_rust Cargo.toml
    pub crate_manifest_path: PathBuf,
}

impl Compiler {
     pub fn new(project_root: &Path) -> Self {
        let manifest = project_root
            .join(".perro")
            .join("rust_scripts")
            .join("Cargo.toml");

        // Canonicalize to normalize separators and resolve symlinks
        let manifest = dunce::canonicalize(&manifest)
            .unwrap_or(manifest); // fall back if canonicalize fails

        Self {
            crate_manifest_path: manifest,
        }
    }

    /// Pick the fastest available linker for the platform
fn best_linker() -> &'static str {
    if cfg!(target_os = "linux") {
        "rust-lld"
    } else if cfg!(target_os = "windows") {
        // Detect GNU vs MSVC
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

    /// Check if we should compile (based on flag file)
    fn should_compile(&self) -> bool {
        match fs::read_to_string(self.flag_path()) {
            Ok(contents) => contents.trim().eq_ignore_ascii_case("true"),
            Err(_) => true, // default to true if missing
        }
    }

    /// Write the flag file
    fn set_should_compile(&self, value: bool) {
        let _ = fs::write(self.flag_path(), if value { "true\n" } else { "false\n" });
    }

    /// Build the cargo command for the given profile
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

        if matches!(profile, BuildProfile::Release) {
            cmd.arg("--release");
        } else if matches!(profile, BuildProfile::Dev) {
            cmd.arg("--profile").arg("hotreload");
        }

        cmd
    }

    /// Spawn the compiler (non-blocking)
    pub fn spawn(&self, profile: BuildProfile) -> Result<Child, String> {
        if !self.should_compile() {
            println!("Nothing to rebuild (should_compile == false)");
            return Err("No rebuild needed".into());
        }

        println!("🚀 Spawning compiler...");
        self.build_command(profile)
            .spawn()
            .map_err(|e| format!("Failed to spawn cargo: {e}"))
    }

    /// Compile and wait until finished (blocking)
    pub fn compile(&self, profile: BuildProfile) -> Result<(), String> {
        if !self.should_compile() {
            println!("Nothing to rebuild (should_compile == false)");
            return Ok(());
        }

        println!("Starting compilation of perro_rust crate…");
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
            self.set_should_compile(false);
            Ok(())
        } else {
            Err(format!("❌ Compilation failed after {:.2?}", elapsed))
        }
    }
}