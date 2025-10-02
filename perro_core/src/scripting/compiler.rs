use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use rand::RngCore;
use rand::seq::SliceRandom;

use crate::brk::build_brk;

pub enum BuildProfile {
    Dev,
    Release,
    Check, // just validate
}

pub enum CompileTarget {
    Scripts, // .perro/scripts
    Project, // .perro/project
}

pub struct Compiler {
    pub crate_manifest_path: PathBuf,
    target: CompileTarget,
}

impl Compiler {
    pub fn new(project_root: &Path, target: CompileTarget) -> Self {
        let manifest = match target {
            CompileTarget::Scripts => project_root.join(".perro/scripts/Cargo.toml"),
            CompileTarget::Project => project_root.join(".perro/project/Cargo.toml"),
        };

        let manifest = dunce::canonicalize(&manifest).unwrap_or(manifest);

        Self {
            crate_manifest_path: manifest,
            target,
        }
    }

    fn best_linker() -> &'static str {
        if cfg!(target_os = "linux") {
            "rust-lld"
        } else if cfg!(target_os = "windows") {
            match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
                Ok("gnu") => "gcc",
                Ok("msvc") => "lld-link",
                _ => "cc",
            }
        } else if cfg!(target_os = "macos") {
            "clang"
        } else {
            "cc"
        }
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

    match self.target {
        CompileTarget::Scripts => {
            cmd.arg("--profile").arg("hotreload");
        }
        CompileTarget::Project => {
            cmd.arg("--release");
            // Force build script to always run by setting env var that changes each time
            cmd.env("PERRO_BUILD_TIMESTAMP", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string());
        }
    }

    cmd
}

    pub fn compile(&self, profile: BuildProfile) -> Result<(), String> {
        if matches!(self.target, CompileTarget::Project) {
            // 🔑 Generate AES key
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);

            println!("🔑 Compile-time AES key: {:02X?}", key);

            // Write obfuscated key.rs
            self.write_key_file(&key).map_err(|e| e.to_string())?;

            // Run packer with the real key
            let project_root = self.crate_manifest_path
                .parent()  // .perro/project
                .unwrap()
                .parent()  // .perro
                .unwrap()
                .parent()  // actual project root
                .unwrap();

            let res_dir = project_root.join("res");
            let output = project_root.join("assets.brk");

            build_brk(&output, &res_dir, project_root, &key)
                .map_err(|e| e.to_string())?;
        }

        println!("🚀 Compiling {:?}", self.target_name());
        let start = Instant::now();
        let status = self
            .build_command(profile)
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
            CompileTarget::Project => "project",
        }
    }

    fn write_key_file(&self, key: &[u8; 32]) -> std::io::Result<()> {
        // Split into 4 parts of 8 bytes
        let mut parts: Vec<[u8; 8]> = Vec::new();
        for chunk in key.chunks(8) {
            let mut part = [0u8; 8];
            part.copy_from_slice(chunk);
            parts.push(part);
        }

        // Generate 8 random constants
        let consts: Vec<u32> = (0..8).map(|_| rand::random::<u32>()).collect();

        // Random operations
        let ops = ["^", "+", "-", ">>", "<<"];

        // Build mask expressions (runtime code) and mask values (compile-time)
        let mut mask_exprs: Vec<String> = Vec::new();
        let mut mask_values: Vec<u8> = Vec::new();

        for _ in 0..4 {
            let c1 = rand::random::<usize>() % 8;
            let c2 = rand::random::<usize>() % 8;
            let op = ops.choose(&mut rand::thread_rng()).unwrap();

            let expr = match *op {
                "^" => {
                    mask_values.push((consts[c1] as u8) ^ (consts[c2] as u8));
                    format!("((CONST{} as u8) ^ (CONST{} as u8))", c1 + 1, c2 + 1)
                }
                "+" => {
                    mask_values.push((consts[c1] as u8).wrapping_add(consts[c2] as u8));
                    format!("((CONST{} as u8).wrapping_add(CONST{} as u8))", c1 + 1, c2 + 1)
                }
                "-" => {
                    mask_values.push((consts[c1] as u8).wrapping_sub(consts[c2] as u8));
                    format!("((CONST{} as u8).wrapping_sub(CONST{} as u8))", c1 + 1, c2 + 1)
                }
                ">>" => {
                    mask_values.push(((consts[c1] >> 8) as u8) ^ (consts[c2] as u8));
                    format!("((CONST{} >> 8) as u8) ^ (CONST{} as u8)", c1 + 1, c2 + 1)
                }
                "<<" => {
                    mask_values.push(((consts[c1] << 3) as u8) ^ (consts[c2] as u8));
                    format!("(((CONST{} << 3) as u8) ^ (CONST{} as u8))", c1 + 1, c2 + 1)
                }
                _ => unreachable!(),
            };

            mask_exprs.push(expr);
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
                let masked = b ^ mask_values[i];
                write!(f, "0x{:02X}", masked)?;
            }
            writeln!(f, "];")?;
        }

        // Write constants
        for (i, c) in consts.iter().enumerate() {
            writeln!(f, "const CONST{}: u32 = 0x{:08X};", i + 1, c)?;
        }

        // Write get_aes_key with inlined mask reconstruction
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