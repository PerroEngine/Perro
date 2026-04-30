#[cfg(target_os = "windows")]
fn main() {
    if let Err(err) = embed_windows_icon() {
        println!("cargo:warning=perro icon embedding skipped: {err}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn embed_windows_icon() -> Result<(), String> {
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };
    use toml::Value;

    fn load_icon_res_path(project_toml: &Path) -> Result<String, String> {
        let src = fs::read_to_string(project_toml)
            .map_err(|e| format!("failed to read {}: {e}", project_toml.display()))?;
        let value: Value = src
            .parse::<Value>()
            .map_err(|e| format!("failed to parse {}: {e}", project_toml.display()))?;
        let icon = value
            .get("project")
            .and_then(Value::as_table)
            .and_then(|project| project.get("icon"))
            .and_then(Value::as_str)
            .unwrap_or("res://icon.png")
            .trim()
            .to_string();
        if !icon.starts_with("res://") {
            return Err(format!("project.icon must start with `res://`, got `{icon}`"));
        }
        Ok(icon)
    }

    fn resolve_res_icon_path(project_root: &Path, icon_res_path: &str) -> PathBuf {
        let rel = icon_res_path
            .trim_start_matches("res://")
            .trim_start_matches('/');
        project_root.join("res").join(rel)
    }

    fn convert_icon_to_ico(source: &Path, out_dir: &Path) -> Result<PathBuf, String> {
        let mut image = image::open(source)
            .map_err(|e| format!("failed to decode icon image `{}`: {e}", source.display()))?;
        let (w, h) = (image.width(), image.height());
        if w > 256 || h > 256 {
            image = image.resize(256, 256, image::imageops::FilterType::Lanczos3);
        }
        let out = out_dir.join("perro_project_icon.ico");
        image
            .save_with_format(&out, image::ImageFormat::Ico)
            .map_err(|e| format!("failed to convert `{}` to ico: {e}", source.display()))?;
        Ok(out)
    }

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").map_err(|e| format!("CARGO_MANIFEST_DIR missing: {e}"))?,
    );
    let project_root = manifest_dir
        .join("..")
        .join("..")
        .canonicalize()
        .map_err(|e| format!("failed to resolve project root from manifest dir: {e}"))?;
    let project_toml = project_root.join("project.toml");
    let icon_res = load_icon_res_path(&project_toml)?;
    let icon_source = resolve_res_icon_path(&project_root, &icon_res);

    println!("cargo:rerun-if-changed={}", project_toml.display());
    println!("cargo:rerun-if-changed={}", icon_source.display());

    if !icon_source.exists() {
        return Err(format!("icon file not found: {}", icon_source.display()));
    }

    let ext = icon_source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let icon_for_resource = if ext == "ico" {
        icon_source
    } else {
        let out_dir = PathBuf::from(
            env::var("OUT_DIR").map_err(|e| format!("OUT_DIR missing: {e}"))?,
        );
        convert_icon_to_ico(&icon_source, &out_dir)?
    };

    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon_for_resource.to_string_lossy().as_ref());
    res.compile()
        .map_err(|e| format!("failed to compile windows resource icon: {e}"))?;
    Ok(())
}
