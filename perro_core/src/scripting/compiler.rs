use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

pub enum BuildProfile {
    Dev,
    Release,
    Check, // new: just validate
}

pub struct Compiler {
    /// Path to the perro_rust Cargo.toml
    pub crate_manifest_path: PathBuf,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            crate_manifest_path: "./perro_rust/Cargo.toml".into(),
        }
    }

    fn best_linker() -> &'static str {
        if cfg!(target_os = "linux") {
            "rust-lld"
        } else if cfg!(target_os = "windows") {
            "rust-lld" // same as lld-link
        } else if cfg!(target_os = "macos") {
            "rust-lld" // Mach-O backend (experimental)
        } else {
            "cc"
        }
    }

    /// Run `cargo build` (or check) on the perro_rust crate and report how long it took.
    pub fn compile(&self, profile: BuildProfile) -> Result<(), String> {
        // ------------------------------------------------------------
        // 1. Determine the flag file: <crate>/should_compile
        // ------------------------------------------------------------
        let flag_path: PathBuf = self
            .crate_manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("should_compile");

        // ------------------------------------------------------------
        // 2. Read the flag; default to true if file is missing/broken
        // ------------------------------------------------------------
        let should_compile = match fs::read_to_string(&flag_path) {
            Ok(contents) => contents.trim().eq_ignore_ascii_case("true"),
            Err(_) => true,
        };

        if !should_compile {
            println!("Nothing to rebuild (should_compile == false)");
            return Ok(());
        }

        // ------------------------------------------------------------
        // 3. Run cargo build/check
        // ------------------------------------------------------------
        println!("Starting compilation of perro_rust crate…");
        let start = Instant::now();

        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let mut cmd = Command::new("cargo");

        match profile {
            BuildProfile::Check => {
                cmd.arg("check");
            }
            _ => {
                cmd.arg("build");
            }
        }

        cmd.arg("--manifest-path")
            .arg(&self.crate_manifest_path)
            .arg("-j")
            .arg(num_cpus.to_string())
            .env("RUSTFLAGS", format!("-C linker={}", Self::best_linker()))
            .stdout(Stdio::inherit()) // stream output live
            .stderr(Stdio::inherit());

        if matches!(profile, BuildProfile::Release) {
            cmd.arg("--release");
        } else if matches!(profile, BuildProfile::Dev) {
            cmd.arg("--profile").arg("hotreload");
        }

        let status = cmd
            .status()
            .map_err(|e| format!("Failed to spawn cargo build: {e}"))?;

        let elapsed = start.elapsed();

        // ------------------------------------------------------------
        // 4. Handle result, update flag file
        // ------------------------------------------------------------
        if status.success() {
            println!("✅ Compilation successful! ({:.2?})", elapsed);
            let _ = fs::write(&flag_path, "false\n");
            Ok(())
        } else {
            Err(format!("❌ Compilation failed after {:.2?}", elapsed))
        }
    }
}