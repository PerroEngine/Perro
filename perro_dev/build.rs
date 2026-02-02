// build.rs
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let log_path = PathBuf::from("build.log");
    init_log(&log_path);
    log(&log_path, "=== Simple Build Script Started ===");

    // Package name and version from Cargo.toml (available on all platforms)
    let name = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "perro_dev".to_string());
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());

    // ───────────────────────────────────────────────
    // Static config (local file)
    // ───────────────────────────────────────────────
    let icon_path = PathBuf::from("./icon.png"); // adjust as needed

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
            &name,
            version_display,
            &name,
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

    #[cfg(target_os = "linux")]
    {
        embed_linux_icon(&icon_path, &log_path);
        setup_linux_desktop(&icon_path, &log_path, &name, &version);
        // Create AppImage (single file with embedded icon) after release builds
        if std::env::var("PROFILE").unwrap_or_default() == "release" {
            create_appimage(&icon_path, &log_path, &name, &version);
        }
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

#[cfg(target_os = "linux")]
fn embed_linux_icon(icon_path: &Path, log_path: &Path) {
    if !icon_path.exists() {
        log(
            log_path,
            &format!(
                "⚠ Icon file not found: {}, skipping icon embedding",
                icon_path.display()
            ),
        );
        return;
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();

    // Copy icon to OUT_DIR so we can include it
    let icon_in_out_dir = PathBuf::from(&out_dir).join("icon.png");
    fs::copy(icon_path, &icon_in_out_dir).expect("Failed to copy icon to OUT_DIR");
    log(
        log_path,
        &format!("✔ Copied icon to OUT_DIR: {}", icon_in_out_dir.display()),
    );

    // Generate module that embeds the icon using include_bytes!
    // This will be included in the binary's data section
    let embedded_icon_module = PathBuf::from(&out_dir).join("embedded_icon.rs");
    let module_content = format!(
        r#"// Auto-generated embedded icon module
// Icon is embedded in the binary at compile time

/// Embedded application icon (PNG bytes)
/// This icon is embedded directly in the binary's data section
#[allow(dead_code)]
pub static EMBEDDED_ICON: &[u8] = include_bytes!("icon.png");
"#
    );

    fs::write(&embedded_icon_module, module_content).expect("Failed to write embedded_icon.rs");
    log(
        log_path,
        &format!(
            "✔ Generated embedded icon module: {}",
            embedded_icon_module.display()
        ),
    );

    // The module will be included via include! macro in main.rs
    println!(
        "cargo:rustc-env=EMBEDDED_ICON_MODULE={}",
        embedded_icon_module.display()
    );
}

#[cfg(target_os = "linux")]
fn setup_linux_desktop(icon_path: &Path, log_path: &Path, name: &str, version: &str) {
    if !icon_path.exists() {
        log(
            log_path,
            &format!(
                "⚠ Icon file not found: {}, skipping desktop setup",
                icon_path.display()
            ),
        );
        return;
    }

    let project_root = PathBuf::from(".");
    let icon_name = format!("{}", name.to_lowercase().replace(" ", "_"));
    let icon_dest = project_root.join(format!("{}.png", icon_name));
    let _ = fs::copy(icon_path, &icon_dest);
    log(
        log_path,
        &format!("✔ Copied icon to {}", icon_dest.display()),
    );

    // Get the binary name early so we can use it for icon installation
    // Note: On Linux, the binary name from Cargo.toml is "PerroDevRuntime"
    let binary_name =
        std::env::var("CARGO_BIN_NAME").unwrap_or_else(|_| "PerroDevRuntime".to_string());
    let binary_name_lower = binary_name.to_lowercase();

    // Install icon to all standard sizes in user's local icon directory
    // This allows file managers to display the icon for the executable
    // Install with both the app name and binary name so file managers can find it either way
    if let Ok(home) = std::env::var("HOME") {
        install_icon_to_system(&icon_path, &home, &icon_name, log_path);
        // Also install with binary name
        install_icon_to_system(&icon_path, &home, &binary_name_lower, log_path);
        // Also use xdg-icon-resource to properly register the icon
        install_icon_with_xdg(&icon_path, &home, &icon_name, log_path);
        install_icon_with_xdg(&icon_path, &home, &binary_name_lower, log_path);
    }

    let desktop_path = project_root.join(format!("{}.desktop", icon_name));
    // Use the binary name as the icon name so file managers can match it to the executable
    // They'll look in standard icon directories
    let desktop_content = format!(
        r#"[Desktop Entry]
Name={}
Exec={}
Icon={}
Type=Application
Categories=Game;
Version={}
StartupNotify=true
Engine=Perro
EngineWebsite=https://perroengine.com
"#,
        name, binary_name_lower, binary_name_lower, version
    );

    fs::write(&desktop_path, &desktop_content).expect("Failed to write .desktop file");
    log(
        log_path,
        &format!("✔ Created Linux desktop file: {}", desktop_path.display()),
    );

    // Also create a desktop file with the exact binary name for better file manager matching
    let binary_desktop_path = project_root.join(format!("{}.desktop", binary_name_lower));
    fs::write(&binary_desktop_path, &desktop_content).expect("Failed to write binary desktop file");
    log(
        log_path,
        &format!(
            "✔ Created binary desktop file: {}",
            binary_desktop_path.display()
        ),
    );

    // Also install desktop file to user's applications directory
    if let Ok(home) = std::env::var("HOME") {
        let apps_dir = PathBuf::from(&home).join(".local/share/applications");
        if fs::create_dir_all(&apps_dir).is_ok() {
            // Install both the icon-named and binary-named desktop files
            let system_desktop_path = apps_dir.join(format!("{}.desktop", icon_name));
            if fs::copy(&desktop_path, &system_desktop_path).is_ok() {
                log(
                    log_path,
                    &format!(
                        "✔ Installed desktop file to: {}",
                        system_desktop_path.display()
                    ),
                );
            }

            let binary_system_desktop_path =
                apps_dir.join(format!("{}.desktop", binary_name_lower));
            if fs::copy(&binary_desktop_path, &binary_system_desktop_path).is_ok() {
                log(
                    log_path,
                    &format!(
                        "✔ Installed binary desktop file to: {}",
                        binary_system_desktop_path.display()
                    ),
                );
            }

            // Update desktop database to register the desktop files
            update_desktop_database(&apps_dir, log_path);
        }
    }

    // Try to update icon cache (non-blocking, failures are OK)
    update_icon_cache(log_path);

    // Also create a desktop file next to the binary (in target directory)
    // This helps file managers associate the icon with the executable
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        let bin_dir = PathBuf::from(&target_dir).join(&profile);

        // Create desktop file with binary name so file managers can find it
        let bin_desktop_path = bin_dir.join(format!("{}.desktop", binary_name_lower));
        if fs::write(&bin_desktop_path, &desktop_content).is_ok() {
            log(
                log_path,
                &format!(
                    "✔ Created desktop file next to binary: {}",
                    bin_desktop_path.display()
                ),
            );
        }
    } else if let Ok(out_dir) = std::env::var("OUT_DIR") {
        // Fallback: try to find the target directory from OUT_DIR
        // OUT_DIR is typically: target/{profile}/build/{package}-{hash}/out
        // We need to go up to target/{profile}
        let out_path = PathBuf::from(&out_dir);
        if let Some(target_profile) = out_path.ancestors().nth(3) {
            let bin_desktop_path = target_profile.join(format!("{}.desktop", binary_name_lower));
            if fs::write(&bin_desktop_path, &desktop_content).is_ok() {
                log(
                    log_path,
                    &format!(
                        "✔ Created desktop file next to binary: {}",
                        bin_desktop_path.display()
                    ),
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn install_icon_to_system(icon_path: &Path, home: &str, icon_name: &str, log_path: &Path) {
    use image::io::Reader as ImageReader;

    // Load the original icon
    let img = match ImageReader::open(icon_path) {
        Ok(reader) => match reader.decode() {
            Ok(img) => img,
            Err(e) => {
                log(
                    log_path,
                    &format!(
                        "⚠ Failed to decode icon: {}, skipping system installation",
                        e
                    ),
                );
                return;
            }
        },
        Err(e) => {
            log(
                log_path,
                &format!("⚠ Failed to open icon: {}, skipping system installation", e),
            );
            return;
        }
    };

    // Standard icon sizes for hicolor theme
    let sizes = [16, 22, 24, 32, 48, 64, 128, 256, 512];

    for size in sizes.iter() {
        let icons_dir =
            PathBuf::from(home).join(format!(".local/share/icons/hicolor/{}x{}/apps", size, size));

        if fs::create_dir_all(&icons_dir).is_err() {
            continue;
        }

        let icon_file = icons_dir.join(format!("{}.png", icon_name));

        // Resize the image to the target size
        let resized = img.resize_exact(
            *size as u32,
            *size as u32,
            image::imageops::FilterType::Lanczos3,
        );

        // Save the resized icon
        if resized.save(&icon_file).is_ok() {
            log(
                log_path,
                &format!(
                    "✔ Installed {}x{} icon to: {}",
                    size,
                    size,
                    icon_file.display()
                ),
            );
        }
    }

    // Also install scalable SVG if we have one (optional, skip for now)
    // For now, we'll just use PNG at multiple sizes
}

#[cfg(target_os = "linux")]
fn install_icon_with_xdg(icon_path: &Path, home: &str, icon_name: &str, log_path: &Path) {
    use image::GenericImageView;
    use image::io::Reader as ImageReader;
    use std::process::Command;

    // Load the original icon to get dimensions
    let img = match ImageReader::open(icon_path) {
        Ok(reader) => match reader.decode() {
            Ok(img) => img,
            Err(_) => return,
        },
        Err(_) => return,
    };

    let (width, height) = img.dimensions();
    let size = (width.min(height)) as usize;

    // Use xdg-icon-resource to install the icon properly
    // This registers it in the icon theme system
    let hicolor_dir = PathBuf::from(home).join(".local/share/icons/hicolor");

    // Create a temporary icon file in the appropriate size directory
    let size_dir = hicolor_dir.join(format!("{}x{}", size, size)).join("apps");
    if fs::create_dir_all(&size_dir).is_ok() {
        let temp_icon = size_dir.join(format!("{}.png", icon_name));
        if fs::copy(icon_path, &temp_icon).is_ok() {
            // Use xdg-icon-resource to install it
            let output = Command::new("xdg-icon-resource")
                .arg("install")
                .arg("--novendor")
                .arg("--size")
                .arg(&size.to_string())
                .arg(&temp_icon)
                .arg(&icon_name)
                .output();

            match output {
                Ok(result) => {
                    if result.status.success() {
                        log(
                            log_path,
                            &format!("✔ Registered icon with xdg-icon-resource: {}", icon_name),
                        );
                    }
                }
                Err(e) => {
                    log(
                        log_path,
                        &format!(
                            "⚠ xdg-icon-resource failed: {} (this is OK, manual install still works)",
                            e
                        ),
                    );
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn update_desktop_database(apps_dir: &Path, log_path: &Path) {
    use std::process::Command;

    // Update the desktop database so file managers can find the desktop file
    let output = Command::new("update-desktop-database")
        .arg(apps_dir)
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                log(log_path, "✔ Updated desktop database");
            }
        }
        Err(_) => {
            // Silently fail - this is optional
        }
    }
}

#[cfg(target_os = "linux")]
fn update_icon_cache(log_path: &Path) {
    // Try to update the icon cache using gtk-update-icon-cache
    // This is non-blocking - if it fails, that's OK
    use std::process::Command;

    if let Ok(home) = std::env::var("HOME") {
        let hicolor_dir = PathBuf::from(&home).join(".local/share/icons/hicolor");

        if hicolor_dir.exists() {
            // Try gtk-update-icon-cache first (GTK 3)
            let output = Command::new("gtk-update-icon-cache")
                .arg("-f")
                .arg("-t")
                .arg(&hicolor_dir)
                .output();

            match output {
                Ok(result) => {
                    if result.status.success() {
                        log(log_path, "✔ Updated GTK icon cache");
                    } else {
                        // Try update-icon-caches (alternative)
                        let _ = Command::new("update-icon-caches")
                            .arg(&hicolor_dir)
                            .output();
                    }
                }
                Err(_) => {
                    // Try update-icon-caches as fallback
                    let _ = Command::new("update-icon-caches")
                        .arg(&hicolor_dir)
                        .output();
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn create_appimage(icon_path: &Path, log_path: &Path, name: &str, version: &str) {
    use std::process::Command;

    // Only create AppImage if appimagetool is available
    if Command::new("appimagetool")
        .arg("--version")
        .output()
        .is_err()
    {
        log(
            log_path,
            "⚠ appimagetool not found, skipping AppImage creation",
        );
        log(
            log_path,
            "  Install with: cargo install cargo-appimage or download from https://github.com/AppImage/AppImageKit",
        );
        return;
    }

    // Get binary name
    let binary_name =
        std::env::var("CARGO_BIN_NAME").unwrap_or_else(|_| "PerroDevRuntime".to_string());
    let icon_name = name.to_lowercase().replace(" ", "_");

    // Determine build directory
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        PathBuf::from("..")
            .join("target")
            .to_string_lossy()
            .to_string()
    });
    let build_dir = PathBuf::from(&target_dir).join(&profile);
    let binary = build_dir.join(&binary_name);

    if !binary.exists() {
        log(
            log_path,
            &format!(
                "⚠ Binary not found at {}, skipping AppImage",
                binary.display()
            ),
        );
        return;
    }

    // Create AppDir structure
    let appdir = build_dir.join("AppDir");
    let _ = fs::remove_dir_all(&appdir);
    fs::create_dir_all(appdir.join("usr/bin")).ok();
    fs::create_dir_all(appdir.join("usr/share/applications")).ok();
    fs::create_dir_all(appdir.join("usr/share/icons/hicolor/256x256/apps")).ok();

    // Copy binary
    if fs::copy(&binary, appdir.join("usr/bin").join(&binary_name)).is_err() {
        log(log_path, "⚠ Failed to copy binary to AppDir");
        return;
    }

    // Copy icon as .DirIcon and to hicolor
    if fs::copy(icon_path, appdir.join(".DirIcon")).is_err() {
        log(log_path, "⚠ Failed to copy icon as .DirIcon");
    }
    if fs::copy(
        icon_path,
        appdir
            .join("usr/share/icons/hicolor/256x256/apps")
            .join(format!("{}.png", icon_name)),
    )
    .is_err()
    {
        log(log_path, "⚠ Failed to copy icon to hicolor");
    }

    // Create desktop file
    let desktop_content = format!(
        r#"[Desktop Entry]
Name={}
Exec={}
Icon={}
Type=Application
Categories=Game;
Version={}
StartupNotify=true
Engine=Perro
EngineWebsite=https://perroengine.com
"#,
        name, binary_name, icon_name, version
    );

    if fs::write(
        appdir
            .join("usr/share/applications")
            .join(format!("{}.desktop", icon_name)),
        &desktop_content,
    )
    .is_err()
    {
        log(log_path, "⚠ Failed to write desktop file");
        return;
    }

    // Create AppImage
    let appimage_name = format!("{}-{}-x86_64.AppImage", binary_name, version);
    let appimage_path = build_dir.join(&appimage_name);

    log(
        log_path,
        &format!("📦 Creating AppImage: {}", appimage_path.display()),
    );

    let output = Command::new("appimagetool")
        .arg(&appdir)
        .arg(&appimage_path)
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                // Make executable
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mut perms) = fs::metadata(&appimage_path).map(|m| m.permissions()) {
                    perms.set_mode(0o755);
                    fs::set_permissions(&appimage_path, perms).ok();
                }
                log(
                    log_path,
                    &format!("✔ AppImage created: {}", appimage_path.display()),
                );
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                log(log_path, &format!("⚠ AppImage creation failed: {}", stderr));
            }
        }
        Err(e) => {
            log(log_path, &format!("⚠ Failed to run appimagetool: {}", e));
        }
    }
}
