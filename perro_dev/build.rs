// build.rs
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let log_path = PathBuf::from("build.log");
    init_log(&log_path);
    log(&log_path, "=== Simple Build Script Started ===");

    // ───────────────────────────────────────────────
    // Static config (local file)
    // ───────────────────────────────────────────────
    let icon_path = PathBuf::from("./icon.png"); // adjust as needed
    let version = "1.0.0";
    let name = "Perro Dev Runtime";

    log(&log_path, &format!("Icon path: {}", icon_path.display()));

    println!("cargo:rerun-if-changed={}", icon_path.display());

    #[cfg(target_os = "windows")]
    {
        let final_icon = ensure_ico(&icon_path, &log_path);

        let parts: Vec<&str> = version.split('.').collect();
        let major = parts.get(0).unwrap_or(&"0").parse::<u16>().unwrap_or(0);
        let minor = parts.get(1).unwrap_or(&"0").parse::<u16>().unwrap_or(0);
        let patch = parts.get(2).unwrap_or(&"0").parse::<u16>().unwrap_or(0);

        let build_number: u32 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;

        let version_display = format!("{}.{}.{}.{}", major, minor, patch, build_number);

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let rc_path = PathBuf::from(&out_dir).join("icon.rc");
        let icon_str = final_icon.to_str().unwrap().replace("\\", "\\\\");

        let rc_content = format!(
            r#"
APPICON ICON "{}"

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
            icon_str,
            major,
            minor,
            patch,
            build_number,
            major,
            minor,
            patch,
            build_number,
            name,
            version_display,
            name,
            version_display
        );

        fs::write(&rc_path, rc_content).expect("Failed to write .rc file");
        log(
            &log_path,
            &format!(
                "✔ Wrote RC with version {} (icon={})",
                version_display,
                final_icon.display()
            ),
        );

        embed_resource::compile(&rc_path, embed_resource::NONE);
        log(&log_path, "✔ Embedded icon + version info successfully");
    }

    log(&log_path, "=== Simple Build Script Finished ===");
}

// ───────────────────────────────────────────────
// Helpers
// ───────────────────────────────────────────────

fn init_log(path: &Path) {
    let _ = fs::remove_file(path);
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .expect("Failed to create build.log");
    writeln!(f, "Simple Build Log").unwrap();
    writeln!(f, "=================").unwrap();
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
fn ensure_ico(path: &Path, log_path: &Path) -> PathBuf {
    if !path.exists() {
        panic!("❌ Icon file not found: {}", path.display());
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "ico" {
        log(log_path, "Icon is already an ICO file.");
        return path.to_path_buf();
    }

    let ico_path = PathBuf::from("icon.ico");
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

    let img = ImageReader::open(input_path)
        .expect("Failed to open image")
        .decode()
        .expect("Failed to decode image");

    let sizes = [16, 32, 48, 256];
    let mut icon_dir = IconDir::new(ResourceType::Icon);

    for size in sizes {
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.into_rgba8();
        let icon_image = IconImage::from_rgba_data(size as u32, size as u32, rgba.into_raw());
        icon_dir.add_entry(IconDirEntry::encode(&icon_image).unwrap());
        log(log_path, &format!("✔ Added {}x{} size", size, size));
    }

    let mut file = File::create(ico_path).expect("Failed to create ICO file");
    icon_dir.write(&mut file).expect("Failed to write ICO file");
    log(log_path, &format!("✔ Saved ICO to {}", ico_path.display()));
}
