use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
    *hash ^= 0xff;
    *hash = hash.wrapping_mul(FNV_PRIME);
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        files.push(path.to_path_buf());
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if child.file_name().is_some_and(|name| name == "target") {
            continue;
        }
        collect_files(&child, files);
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let engine_root = manifest_dir
        .ancestors()
        .nth(3)
        .expect("perro_runtime must stay under perro_source/runtime_project")
        .to_path_buf();

    let mut files = Vec::new();
    collect_files(&engine_root.join("perro_source"), &mut files);
    files.push(engine_root.join("Cargo.toml"));
    files.push(engine_root.join("Cargo.lock"));
    files.sort();
    files.dedup();

    let mut hash = FNV_OFFSET;
    hash_bytes(&mut hash, b"perro-script-abi-v2");
    for path in files {
        let extension = path.extension().and_then(|value| value.to_str());
        if !matches!(extension, Some("rs" | "toml" | "lock")) {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let relative = path.strip_prefix(&engine_root).unwrap_or(&path);
        hash_bytes(
            &mut hash,
            relative.to_string_lossy().replace('\\', "/").as_bytes(),
        );
        hash_bytes(
            &mut hash,
            &fs::read(&path).expect("read ABI fingerprint input"),
        );
    }

    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let rustc_version = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("run rustc -vV");
    assert!(rustc_version.status.success(), "rustc -vV failed");
    hash_bytes(&mut hash, &rustc_version.stdout);

    let mut build_env = env::vars()
        .filter(|(key, _)| {
            matches!(
                key.as_str(),
                "CARGO_CFG_TARGET_ABI"
                    | "CARGO_CFG_TARGET_ARCH"
                    | "CARGO_CFG_TARGET_ENDIAN"
                    | "CARGO_CFG_TARGET_ENV"
                    | "CARGO_CFG_TARGET_FAMILY"
                    | "CARGO_CFG_TARGET_FEATURE"
                    | "CARGO_CFG_TARGET_OS"
                    | "CARGO_CFG_TARGET_POINTER_WIDTH"
                    | "CARGO_CFG_TARGET_VENDOR"
                    | "CARGO_ENCODED_RUSTFLAGS"
                    | "CARGO_FEATURE_STEAMWORKS"
                    | "HOST"
                    | "TARGET"
            )
        })
        .collect::<Vec<_>>();
    build_env.sort();
    for (key, value) in build_env {
        hash_bytes(&mut hash, key.as_bytes());
        hash_bytes(&mut hash, value.as_bytes());
    }

    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("out dir")).join("script_abi_fingerprint.rs");
    fs::write(
        output,
        format!(
            "/// Build identity required by dynamic script libraries.\n\
             #[doc(hidden)]\n\
             pub const SCRIPT_ABI_BUILD_FINGERPRINT: u64 = 0x{hash:016x};\n"
        ),
    )
    .expect("write ABI fingerprint");
}
