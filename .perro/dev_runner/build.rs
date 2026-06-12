#[cfg(target_os = "windows")]
fn main() {
    println!("cargo:rustc-check-cfg=cfg(perro_no_console)");
    if !target_supports_windows_resource() {
        return;
    }
    if let Err(err) = embed_windows_icon() {
        println!("cargo:warning=perro icon embedding skipped: {err}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("cargo:rustc-check-cfg=cfg(perro_no_console)");
}

#[cfg(target_os = "windows")]
fn target_supports_windows_resource() -> bool {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return false;
    }

    matches!(
        std::env::var("CARGO_CFG_TARGET_ENV").ok().as_deref(),
        Some("gnu" | "msvc")
    )
}

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
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mut image = if ext == "svg" {
            decode_svg_icon(source)?
        } else {
            image::open(source)
                .map_err(|e| format!("failed to decode icon image `{}`: {e}", source.display()))?
        };
        image = trim_icon_alpha(image);
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

    fn decode_svg_icon(source: &Path) -> Result<image::DynamicImage, String> {
        let bytes = fs::read(source)
            .map_err(|e| format!("failed to read icon image `{}`: {e}", source.display()))?;
        let options = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_data(&bytes, &options)
            .map_err(|e| format!("failed to decode icon image `{}`: {e}", source.display()))?;
        let tree_size = tree.size();
        let tree_width = tree_size.width().max(1.0);
        let tree_height = tree_size.height().max(1.0);
        let (width, height) = svg_icon_target_size(&bytes, tree_width, tree_height);
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| format!("failed to allocate svg icon pixmap `{}`", source.display()))?;
        let transform = resvg::tiny_skia::Transform::from_scale(
            width as f32 / tree_width,
            height as f32 / tree_height,
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for pixel in pixmap.pixels() {
            rgba.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
        }
        let image = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or_else(|| format!("failed to build svg icon image `{}`", source.display()))?;
        Ok(image::DynamicImage::ImageRgba8(image))
    }

    fn trim_icon_alpha(image: image::DynamicImage) -> image::DynamicImage {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let mut min_x = width;
        let mut min_y = height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut found = false;

        for (x, y, pixel) in rgba.enumerate_pixels() {
            if pixel[3] == 0 {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            found = true;
        }

        if !found {
            return image::DynamicImage::ImageRgba8(rgba);
        }
        if min_x == 0 && min_y == 0 && max_x + 1 == width && max_y + 1 == height {
            return image::DynamicImage::ImageRgba8(rgba);
        }

        let trim_width = max_x - min_x + 1;
        let trim_height = max_y - min_y + 1;
        image::DynamicImage::ImageRgba8(
            image::imageops::crop_imm(&rgba, min_x, min_y, trim_width, trim_height).to_image(),
        )
    }

    fn svg_icon_target_size(bytes: &[u8], tree_width: f32, tree_height: f32) -> (u32, u32) {
        const RASTER_SCALE: u32 = 4;
        let (width, height) = svg_declared_size(bytes).unwrap_or_else(|| {
            (
                tree_width.round().clamp(1.0, 256.0) as u32,
                tree_height.round().clamp(1.0, 256.0) as u32,
            )
        });
        (
            width.saturating_mul(RASTER_SCALE).max(1),
            height.saturating_mul(RASTER_SCALE).max(1),
        )
    }

    fn svg_declared_size(bytes: &[u8]) -> Option<(u32, u32)> {
        let src = std::str::from_utf8(bytes).ok()?;
        let tag = svg_start_tag(src)?;
        if let (Some(width), Some(height)) = (svg_attr_number(tag, "width"), svg_attr_number(tag, "height")) {
            return Some((width.min(256), height.min(256)));
        }
        if let Some((width, height)) = svg_viewbox_size(tag) {
            return Some((width.min(256), height.min(256)));
        }
        Some((256, 256))
    }

    fn svg_start_tag(src: &str) -> Option<&str> {
        let start = src.find("<svg")?;
        let rest = &src[start..];
        Some(&rest[..rest.find('>')?])
    }

    fn svg_attr_number(tag: &str, name: &str) -> Option<u32> {
        parse_svg_number(svg_attr_value(tag, name)?)
    }

    fn svg_attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
        let idx = tag.find(name)?;
        let value = tag[idx + name.len()..].trim_start().strip_prefix('=')?.trim_start();
        let quote = value.chars().next()?;
        if quote == '"' || quote == '\'' {
            let value = &value[quote.len_utf8()..];
            return Some(&value[..value.find(quote)?]);
        }
        Some(&value[..value.find(|ch: char| ch.is_ascii_whitespace() || ch == '>').unwrap_or(value.len())])
    }

    fn svg_viewbox_size(tag: &str) -> Option<(u32, u32)> {
        let value = svg_attr_value(tag, "viewBox").or_else(|| svg_attr_value(tag, "viewbox"))?;
        let nums: Vec<f32> = value
            .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<f32>().ok())
            .collect();
        if nums.len() < 4 {
            return None;
        }
        Some((size_component(nums[2])?, size_component(nums[3])?))
    }

    fn parse_svg_number(value: &str) -> Option<u32> {
        let trimmed = value.trim();
        let number_len = trimmed
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
            .unwrap_or(trimmed.len());
        if trimmed.get(number_len..)?.trim().starts_with('%') {
            return None;
        }
        size_component(trimmed.get(..number_len)?.parse::<f32>().ok()?)
    }

    fn size_component(value: f32) -> Option<u32> {
        (value.is_finite() && value > 0.0).then(|| value.round().max(1.0) as u32)
    }

    fn metadata_str(value: &Value, key: &str) -> Option<String> {
        value
            .get("metadata")
            .and_then(Value::as_table)
            .and_then(|metadata| metadata.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    }

    fn project_name(value: &Value) -> String {
        value
            .get("project")
            .and_then(Value::as_table)
            .and_then(|project| project.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("Perro Project")
            .to_string()
    }

    fn apply_windows_metadata(
        res: &mut winresource::WindowsResource,
        project_toml: &Path,
    ) -> Result<(), String> {
        let src = fs::read_to_string(project_toml)
            .map_err(|e| format!("failed to read {}: {e}", project_toml.display()))?;
        let value: Value = src
            .parse::<Value>()
            .map_err(|e| format!("failed to parse {}: {e}", project_toml.display()))?;
        let name = project_name(&value);
        let description = metadata_str(&value, "description").unwrap_or_else(|| name.clone());
        let version = metadata_str(&value, "version").unwrap_or_else(|| "0.1.0".to_string());

        res.set("FileDescription", &description);
        res.set("ProductName", &name);
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);
        res.set("OriginalFilename", &format!("{name}.exe"));
        res.set("Comments", "Made with Perro Engine");
        res.set("InternalName", &name);
        res.set("PerroEngine", "Perro Engine");
        if let Some(company) = metadata_str(&value, "company") {
            res.set("CompanyName", &company);
        }
        if let Some(copyright) = metadata_str(&value, "copyright") {
            res.set("LegalCopyright", &copyright);
        }
        if let Some(trademark) = metadata_str(&value, "trademark") {
            res.set("LegalTrademarks", &trademark);
        }
        Ok(())
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
    apply_windows_metadata(&mut res, &project_toml)?;
    res.compile()
        .map_err(|e| format!("failed to compile windows resource icon: {e}"))?;
    Ok(())
}
