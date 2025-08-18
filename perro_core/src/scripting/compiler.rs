use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

pub enum BuildProfile {
    Dev,
    Release,
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

    /// Run `cargo build` on the perro_rust crate and report how long it took.
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
            Err(_) => true, // treat unreadable/missing file as "true"
        };

        if !should_compile {
            println!("Nothing to rebuild (should_compile == false)");
            return Ok(());
        }

        // ------------------------------------------------------------
        // 3. Run cargo build
        // ------------------------------------------------------------
        println!("Starting compilation of perro_rust crate…");
        let start = Instant::now();

        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--manifest-path")
            .arg(&self.crate_manifest_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if matches!(profile, BuildProfile::Release) {
            cmd.arg("--release");
        }

        let output = cmd
            .spawn()
            .and_then(|child| child.wait_with_output())
            .map_err(|e| format!("Failed to spawn cargo build: {e}"))?;

        let elapsed = start.elapsed();

        // ------------------------------------------------------------
        // 4. Handle result, update flag file
        // ------------------------------------------------------------
        if output.status.success() {
            println!("Compilation successful! ({:.2?})", elapsed);
            // best-effort write; ignore errors on purpose
            let _ = fs::write(&flag_path, "false\n");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Compilation failed after {:.2?}:\n{}", elapsed, stderr);
            Err(stderr.to_string())
        }
    }
}