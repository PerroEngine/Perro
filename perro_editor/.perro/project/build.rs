use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use toml::Value;

fn main() {
    // Set up logging into build.log
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent()
        .expect("Failed to get parent")
        .parent()
        .expect("Failed to get grandparent");

    let log_path = project_root.join("build.log");
    init_log(&log_path);
    log(&log_path, "=== Build Script Started ===");

    // Read project.toml
    let project_toml_path = project_root.join("project.toml");
    log(&log_path, &format!("Reading {}", project_toml_path.display()));

    let content = fs::read_to_string(&project_toml_path)
        .expect("❌ Could not read project.toml");
    let config: Value = content.parse().expect("❌ Invalid project.toml format");

    let project = config.get("project").expect("❌ Missing [project] section");

    let name = project
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Perro Game");

    let version = project
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.1.0");

    let icon_path = project
        .get("icon")
        .and_then(|v| v.as_str())
        .unwrap_or("res://icon.png");

    log(&log_path, &format!("Project: {}", name));
    log(&log_path, &format!("Version: {}", version));
    log(&log_path, &format!("Configured icon path: {}", icon_path));

    let real_icon_path = resolve_res_path(project_root.to_path_buf(), icon_path);
    log(&log_path, &format!("Resolved icon path: {}", real_icon_path.display()));

    // Always rerun if these files or env change
    println!("cargo:rerun-if-changed={}", project_toml_path.display());
    println!("cargo:rerun-if-changed={}", real_icon_path.display());
    println!("cargo:rerun-if-env-changed=PERRO_BUILD_TIMESTAMP");

    #[cfg(target_os = "windows")]
    {
        let final_icon = ensure_ico(&real_icon_path, &project_root, &log_path);

        if final_icon.exists() {
            if let Ok(metadata) = fs::metadata(&final_icon) {
                if metadata.len() == 0 {
                    panic!("❌ Icon file is empty: {}", final_icon.display());
                }
                log(
                    &log_path,
                    &format!("✔ Final ICO is valid ({} bytes)", metadata.len()),
                );
            }

            // Parse semver (major.minor.patch)
            let parts: Vec<&str> = version.split('.').collect();
            let major = parts.get(0).unwrap_or(&"0").parse::<u16>().unwrap_or(0);
            let minor = parts.get(1).unwrap_or(&"0").parse::<u16>().unwrap_or(0);
            let patch = parts.get(2).unwrap_or(&"0").parse::<u16>().unwrap_or(0);

            // Build number: from env or fallback
            let build_number: u32 = std::env::var("PERRO_BUILD_TIMESTAMP")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as u32
                });

            let version_display =
                format!("{}.{}.{}.{}", major, minor, patch, build_number);

            // Create .rc file
            let out_dir = std::env::var("OUT_DIR").unwrap();
            let rc_path = PathBuf::from(&out_dir).join("icon.rc");
            let icon_str = final_icon.to_str().unwrap().replace("\\", "\\\\");

            // 👇 use unique resource ID for the icon
         let rc_content = format!(
    r#"
APPICON_{} ICON "{}"

1 VERSIONINFO
FILEVERSION {},{},{},{}
PRODUCTVERSION {},{},{},{}
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904E4"
        BEGIN
            VALUE "FileDescription", "{}"
            VALUE "FileVersion", "{}"
            VALUE "ProductName", "{}"
            VALUE "ProductVersion", "{}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1252
    END
END
"#,
    build_number,  
    icon_str,
    major, minor, patch, build_number,
    major, minor, patch, build_number,
    name,
    version_display,
    name,
    version_display
);

            fs::write(&rc_path, rc_content).expect("Failed to write .rc file");
            log(
                &log_path,
                &format!("✔ Wrote RC with version {} (icon ID={})", version_display, build_number),
            );

            embed_resource::compile(&rc_path, embed_resource::NONE);
            log(&log_path, "✔ Icon + version resource embedded successfully");
        } else {
            panic!("⚠ Icon not found at {}", final_icon.display());
        }
    }

    log(&log_path, "=== Build Script Finished ===");
}

fn init_log(path: &Path) {
    let _ = fs::remove_file(path);
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .expect("Failed to create build.log");
    writeln!(f, "Perro Build Log").unwrap();
    writeln!(f, "================").unwrap();
}

fn log(path: &Path, message: &str) {
    println!("{}", message);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("Failed to open build.log");
    writeln!(f, "{}", message).unwrap();
}

#[cfg(target_os = "windows")]
fn ensure_ico(path: &Path, project_root: &Path, log_path: &Path) -> PathBuf {
    if !path.exists() {
        panic!("❌ Icon file not found: {}", path.display());
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "ico" {
        log(log_path, "Icon is already an ICO file, using directly.");
        return path.to_path_buf();
    }

    let ico_path = project_root.join("icon.ico");
    log(
        log_path,
        &format!("Converting {} → {}", path.display(), ico_path.display()),
    );
    convert_any_image_to_ico(path, &ico_path, log_path);
    ico_path
}

#[cfg(target_os = "windows")]
fn convert_any_image_to_ico(input_path: &Path, ico_path: &Path, log_path: &Path) {
    use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
    use image::io::Reader as ImageReader;
    use std::fs::File;

    if !input_path.exists() {
        panic!("❌ Icon path does NOT exist: {}", input_path.display());
    }

    let img = ImageReader::open(input_path)
        .expect("Failed to open image")
        .decode()
        .expect("Failed to decode image");

    let sizes = [16, 32, 48, 256];
    let mut icon_dir = IconDir::new(ResourceType::Icon);

    for size in sizes {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.into_rgba8();
        let icon_image =
            IconImage::from_rgba_data(size as u32, size as u32, rgba.into_raw());
        icon_dir.add_entry(IconDirEntry::encode(&icon_image).unwrap());
        log(log_path, &format!("✔ Added {}x{} size to ICO", size, size));
    }

    let mut file = File::create(ico_path).expect("Failed to create ICO file");
    icon_dir
        .write(&mut file)
        .expect("Failed to write ICO file");
    log(log_path, &format!("✔ ICO saved: {}", ico_path.display()));
}

fn rename_binary(project_name: &str, log_path: &Path) {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(&out_dir)
        .ancestors()
        .nth(3) // OUT_DIR is deep inside target/{profile}/build/<hash>/out
        .unwrap()
        .to_path_buf();

    let profile = std::env::var("PROFILE").unwrap();
    let bin_name = std::env::var("CARGO_PKG_NAME").unwrap();
    
    // Path to the original binary
    let src_bin = target_dir
        .join(&profile)
        .join(if cfg!(windows) {
            format!("{}.exe", bin_name)
        } else {
            bin_name.clone()
        });

    // New name from project.toml
    let renamed_bin = target_dir
        .join(&profile)
        .join(if cfg!(windows) {
            format!("{}.exe", project_name)
        } else {
            project_name.to_string()
        });

    if src_bin.exists() {
        std::fs::copy(&src_bin, &renamed_bin).expect("Failed to rename binary");
        log(log_path, &format!("✔ Renamed binary to {}", renamed_bin.display()));
    } else {
        log(log_path, &format!("⚠ Could not find binary at {}", src_bin.display()));
    }
}


fn resolve_res_path(project_root: PathBuf, res_path: &str) -> PathBuf {
    if let Some(stripped) = res_path.strip_prefix("res://") {
        project_root.join("res").join(stripped)
    } else {
        project_root.join(res_path)
    }
}