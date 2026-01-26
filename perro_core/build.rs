// build.rs
// Generates a compile-time constant with the current unix timestamp
// This is used to ensure script hashes change when perro_core is recompiled

fn main() {
    // Get the current unix timestamp at compile time
    let unix_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Write it to OUT_DIR so it can be included as a constant
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let timestamp_file = std::path::PathBuf::from(&out_dir).join("compile_time_timestamp.rs");
    
    let content = format!(
        r#"// Auto-generated compile-time timestamp
// This file is generated at build time and contains the unix timestamp
// when perro_core was compiled. This ensures script hashes change
// when the core is recompiled, even if scripts haven't changed.

/// Compile-time unix timestamp (seconds since epoch)
/// This is set when perro_core is compiled and remains constant
/// until the next recompilation.
pub const COMPILE_TIME_UNIX_TIMESTAMP: u64 = {};
"#,
        unix_timestamp
    );

    std::fs::write(&timestamp_file, content)
        .expect("Failed to write compile_time_timestamp.rs");
    
    println!("cargo:rerun-if-changed=build.rs");
}
