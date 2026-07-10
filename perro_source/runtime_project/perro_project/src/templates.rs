fn write_if_missing(path: PathBuf, contents: &str) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, contents)
}

fn write_if_changed(path: &Path, contents: &str) -> std::io::Result<()> {
    if file_text_matches(path, contents)? {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let _guard = WriteLock::acquire(path)?;
    if file_text_matches(path, contents)? {
        return Ok(());
    }
    fs::write(path, contents)
}

fn file_text_matches(path: &Path, contents: &str) -> std::io::Result<bool> {
    match fs::read_to_string(path) {
        Ok(existing) => Ok(existing == contents),
        Err(err)
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidData
            ) =>
        {
            Ok(false)
        }
        Err(err) => Err(err),
    }
}

struct WriteLock {
    path: PathBuf,
}

impl WriteLock {
    fn acquire(path: &Path) -> std::io::Result<Self> {
        Self::acquire_with_timeout(path, std::time::Duration::from_secs(10))
    }

    fn acquire_with_timeout(path: &Path, timeout: std::time::Duration) -> std::io::Result<Self> {
        let lock_path = path.with_extension("write-lock");
        let started = std::time::Instant::now();
        loop {
            match fs::create_dir(&lock_path) {
                Ok(()) => return Ok(Self { path: lock_path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let elapsed = started.elapsed();
                    if elapsed >= timeout {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            format!("timed out waiting for write lock `{}`", lock_path.display()),
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10).min(timeout - elapsed));
                }
                Err(err) => return Err(err),
            }
        }
    }
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

#[cfg(test)]
mod write_lock_tests {
    use super::{WriteLock, default_project_build_rs};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn existing_write_lock_times_out() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "perro_write_lock_timeout_{}_{}",
            std::process::id(),
            stamp
        ));
        std::fs::create_dir_all(&root).expect("temp root");
        let target = root.join("generated.rs");
        let lock = target.with_extension("write-lock");
        std::fs::create_dir(&lock).expect("held lock");

        let started = Instant::now();
        let err = WriteLock::acquire_with_timeout(&target, Duration::from_millis(25))
            .err()
            .expect("timeout error");

        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(1));
        std::fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn generated_build_script_contains_icon_path_guards() {
        let script = default_project_build_rs();

        assert!(script.contains("component == \".\" || component == \"..\""));
        assert!(script.contains("source.starts_with(&res_root)"));
        assert!(script.contains("resolve_res_icon_path(&project_root, &icon_res)?"));
    }
}

fn crate_name_from_project_name(project_name: &str) -> String {
    let mut out = String::with_capacity(project_name.len() + 8);
    for c in project_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    let mut normalized = if trimmed.is_empty() {
        "perro_project".to_string()
    } else {
        trimmed.to_string()
    };
    if normalized
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
    {
        normalized.insert(0, '_');
    }
    normalized
}

fn default_main_scene() -> String {
    r#"$root = @main

[main]

[Node3D]
    position = (0, 0, 0)
[/Node3D]
[/main]

[camera]
parent = $root

[Camera3D]
    active = true
    [Node3D]
        position = (0, 0, 8)
    [/Node3D]
[/Camera3D]
[/camera]

[ambient]
parent = $root

[AmbientLight3D]
    color = (1.0, 1.0, 1.0)
    intensity = 0.8
[/AmbientLight3D]
[/ambient]
"#
    .to_string()
}

fn default_project_readme_md(project_name: &str) -> String {
    format!(
        r#"# {project_name}

Welcome to your Perro project. This README is a quick map of how things fit together.

Run `perro check` to sync scripts and get rust-analyzer working.

## Project Layout
- `project.toml` is the project config (main scene, icon, graphics defaults).
- `deps.toml` is optional. Add `[dependencies]` here for extra Rust crates used by scripts.
- `res/` holds your assets, scripts, and scenes. `res://` paths resolve into this folder.
- `res/main.scn` is the default scene because `project.toml` points to it by default.
- `.perro/` contains generated Rust crates (project, scripts, dev runner). You generally don’t touch these.
  - `project/` is the static project crate produced by `perro build`. It bakes assets and links scripts into the final executable.
  - `scripts/` is generated from any `.rs` file under `res/` plus Perro’s internal glue. It gets overwritten on build, so don’t edit it directly.
  - `dev_runner/` is built and run by `perro dev`. It loads the scripts dynamic library in dev mode.
  - Output from `perro build` goes to `.output/` for convenience so you do not have to dig through `target/`.

## Common Commands
- `perro new` creates a project (you just ran this).
- `perro dev` builds scripts and runs the dev runner.
- `perro check` builds scripts only.
- `perro build` builds the full static bundle.
- `perro format` runs rustfmt for all `.rs` scripts under `res/`.
- `perro new_script` creates a new script template in `res/` (use `--res` for subfolders).
- `perro new_scene` creates a new scene template in `res/` (use `--res` and `--template 2D|3D`).
- `perro new_animation` creates a new `.panim` animation clip template (defaults to `res/animations`).
- If you run these inside the project root, you do not need `--path`.

## Scenes And Scripts
- Scenes are `.scn` files under `res/`.
- Script files are Rust files under `res/` (any `.rs` file under `res/`).
- You attach scripts to nodes in scenes using a `script` field with a `res://` path.
- Example:
```text
[Player]
    script = "res://scripts/player.rs"
    [Node2D]
            position = (0, 0)
    [/Node2D]
[/Player]
```
- Use `res://` paths to reference files in res/
- Use `user://` when you want user data, either to read or write. On Windows this resolves to:
  `C:\Users\<You>\AppData\Local\<ProjectName>\data\...`
- On web target, `user://...` maps to browser `localStorage` with project-scoped keys.
- Use `perro_web::storage` if you need `sessionStorage` or cookie-backed values.
- You cannot write to res in release

## Documentation
The comprehensive docs live in the main Perro repository on GitHub: `https://github.com/PerroEngine/Perro/blob/main/docs/index.md`
"#
    )
}

pub fn default_script_example_rs() -> String {
    r#"use perro_api::prelude::*;

// Script is authored against a node type. This default template uses Node2D.
type SelfNodeType = Node2D;

// State is data-only. Keeping state separate from behavior makes cross-calls memory safe
// and helps the runtime handle recursion/re-entrancy without borrowing issues.

// Custom structs/enums used in #[State] or methods! typed params/returns should derive Variant.
// Without Variant, runtime variant conversion for those types will not compile.
#[derive(Clone, Copy, Variant)]
struct OrbitGoal {
    axis: Vector3,
}

impl Default for OrbitGoal {
    fn default() -> Self {
        Self {
            axis: Vector3::new(0.0, 1.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, Variant)]
struct MotionSample {
    velocity: Vector3,
    drift: Vector3,
}

impl Default for MotionSample {
    fn default() -> Self {
        Self {
            velocity: Vector3::ZERO,
            drift: Vector3::new(0.0, 0.0, 0.25),
        }
    }
}

// Define state struct with #[State] and use #[default = _] for default values on initialization.
#[State]
struct ExampleState {
    #[default = 5]
    count: i32,

    #[default = OrbitGoal::default()]
    orbit_goal: OrbitGoal,

    #[default = MotionSample::default()]
    motion_sample: MotionSample,
}

const SPEED: f32 = 5.0;

lifecycle!({
    // Lifecycle methods are engine entry points. They are called by the runtime.
    // `ctx` is the main interface into the engine core to access runtime data/scripts and nodes.
    // `res` is resource access (meshes/materials/textures) available at runtime.
    // `ipt` is immutable input state for the current frame (keys pressed/released/down).
    // `self` is the NodeID handle of the node this script is attached to.

    // init is called when the script instance is created. This can be used for one-time setup. State is initialized
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        // with_state! gives read-only state access and returns data from the closure.
        // with_state_mut! gives mutable state access; it can mutate and optionally return data.
        let count = with_state!(ctx.run, ExampleState, ctx.id, |state| {
            state.count
        });
        log_info!(count);
    }

    // on_all_init is called after all scripts have had on_init called. This can be used for setup that requires other scripts to be initialized.
    fn on_all_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    // on_update is called every frame. This is where most behavior logic goes.
    fn on_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        let dt = delta_time!(ctx.run);
        let _is_space_down = ctx.ipt.Keys().down(KeyCode::Space);

        // Regular Rust method calls are for internal methods.
        self.bump_count(ctx);

        // with_node! gives read-only typed node access and returns data from the closure.
        // with_node_mut! gives mutable typed node access; it can mutate and optionally return data.
        // Here we mutate the attached node via `self`.
        with_node_mut!(ctx.run, SelfNodeType, ctx.id, |node| {
            node.position.x += dt * SPEED;
        });

        // You can also pass another NodeID with another node type if that id maps
        // to that type at runtime.
        // Example:
        // with_node_mut!(ctx, MeshInstance3D, enemy, |mesh| { mesh.scale.x += 1.0; });
        //
        // For common hierarchy/identity operations, prefer dedicated helper macros:
        // let name = get_node_name!(ctx, node).unwrap_or_default();
        // let parent = get_node_parent_id!(ctx, node).unwrap_or(NodeID::nil());
        // let children = get_node_children_ids!(ctx, node).unwrap_or_default();
        // let _renamed = set_node_name!(ctx, node, "Player");
        // let _ok = reparent!(ctx, NodeID::new(10), node);
        // let _moved = reparent_multi!(ctx, NodeID::new(10), [NodeID::new(11), NodeID::new(12)]);
        //
        // Script attachment helpers:
        // let _attached = script_attach!(ctx, node, "res://scripts/other.rs");
        // let _detached = script_detach!(ctx, node);
        // `script_attach!` takes a target node id + script path.
        // `script_detach!` takes a node/script id and removes the attached script instance.
        //
        // call_method! can invoke methods through the script interface by member id.
        // Here we call our own script through self for demonstration.
        call_method!(ctx, node, func!("test"), params![7123_i32, "bodsasb"]);
        set_var!(ctx, node, var!("count"), 77_i32.into());
        let remote_count = get_var!(ctx, node, var!("count"));
        log_info!(remote_count);
        // For local/internal behavior and local state, prefer direct methods plus
        // with_state!/with_state_mut! (for example self.bump_count(...)).
        // Read-only helpers (`with_state!`, `with_node!`) are for non-mutable access.
        // Mutable helpers (`with_state_mut!`, `with_node_mut!`) can mutate and
        // can return a value if you need one; ignoring the return is also fine.
        // That is simpler and more performant than call_method!/get_var!/set_var!.

        // Typical NodeID lookup is runtime-dependent. NodeID is a handle, not the node value.
        // if let Some(enemy) = find_node!(ctx, "enemy") {
        //     // Cross-script call on another script instance:
        //     call_method!(ctx, enemy, func!("test"), params![1_i32, "ping"]);
        //
        //     // Mutate enemy node directly if you know its runtime node type:
        //     with_node_mut!(ctx, MeshInstance3D, enemy, |enemy| {
        //         enemy.scale.x += 0.1;
        //     });
        //
        //     // If type is uncertain, check metadata/type first, then branch/match.
        // }
    }

    // on_fixed_update is called on a fixed timestep, independent of frame rate. This is useful for physics and other deterministic updates.
    fn on_fixed_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    // on_removal is called when the script instance is removed from a node or the node is removed from the scene. This can be used for cleanup.
    fn on_removal(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});

methods!({
    // methods! defines callable behavior methods (local or cross-script via call_method!)...
    fn bump_count(&self, ctx: &mut ScriptContext<'_, API>) {
        //  Use `with_state_mut!` for mutable access to state
        with_state_mut!(ctx.run, ExampleState, ctx.id, |state| {
            state.count += 1;
        });
    }

    fn test(&self, ctx: &mut ScriptContext<'_, API>, param1: i32, msg: &str) {
        log_info!(param1);
        log_info!(msg);
        self.bump_count(ctx);
    }
});
"#
    .to_string()
}

pub fn default_script_empty_rs() -> String {
    r#"use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[State]
struct EmptyState {}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    fn on_all_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    fn on_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    fn on_fixed_update(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}

    fn on_removal(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});

methods!({
    fn default_method(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {}
});
"#
    .to_string()
}

fn default_gitignore() -> String {
    "target/\n.perro/\n.output/\n".to_string()
}

fn default_deps_toml() -> String {
    r#"# Optional script crate dependencies.
# On `perro check`, `perro dev`, and `perro build`, these are merged into:
#   .perro/scripts/Cargo.toml -> [dependencies]
#
# Keep `perro_api` + `perro_runtime` managed by the engine; they are injected automatically.
#
# Example:
# serde = { version = "1", features = ["derive"] }
# rand = "0.9"
[dependencies]
"#
    .to_string()
}

fn default_project_crate_toml(crate_name: &str) -> String {
    format!(
        r#"[workspace]

[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2024"
build = "build.rs"

[lib]
name = "main"
crate-type = ["cdylib", "rlib"]

[dependencies]
perro_app = "0.1.0"
perro_api = "0.1.0"
perro_scene = "0.1.0"
perro_render_bridge = "0.1.0"
perro_animation = "0.1.0"
perro_structs = "0.1.0"
perro_input_api = "0.1.0"
scripts = {{ path = "../scripts" }}

[features]
profile = ["perro_app/profile"]
steamworks = ["perro_app/steamworks", "perro_api/steamworks", "perro_runtime/steamworks", "scripts/steamworks"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "=0.2.126"
console_error_panic_hook = "0.1.7"
getrandom = {{ version = "0.3.4", features = ["wasm_js"] }}
getrandom_js = {{ package = "getrandom", version = "0.2.17", features = ["js"] }}

[package.metadata.android]
package = "com.perro.__PROJECT_CRATE__"
build_targets = ["aarch64-linux-android"]
label = "__PROJECT_NAME__"
min_sdk_version = 26
target_sdk_version = 35

[target.'cfg(target_os = "windows")'.build-dependencies]
winresource = "0.1.20"
perro_api = "0.1.0"
toml = "0.8.23"
image = {{ version = "0.25.9", default-features = false, features = ["png", "jpeg", "gif", "bmp", "tga", "webp", "ico"] }}
resvg = "0.47.0"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "none"
incremental = true
debug = false
debug-assertions = false
overflow-checks = false

[profile.release.package.{crate_name}]
strip = "symbols"
 "# 
    )
    .replace("__PROJECT_CRATE__", crate_name)
    .replace("__PROJECT_NAME__", crate_name)
}

fn default_project_build_rs() -> String {
    r#"#[cfg(target_os = "windows")]
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
        let Some(relative) = icon.strip_prefix("res://") else {
            return Err(format!("project.icon must start with `res://`, got `{icon}`"));
        };
        if relative.is_empty()
            || relative.contains(['\\', ':'])
            || relative.chars().any(char::is_control)
            || relative
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
        {
            return Err(format!(
                "project.icon must stay inside `res://` and use normal path components, got `{icon}`"
            ));
        }
        Ok(icon)
    }

    fn resolve_res_icon_path(
        project_root: &Path,
        icon_res_path: &str,
    ) -> Result<PathBuf, String> {
        let rel = icon_res_path
            .trim_start_matches("res://")
            .trim_start_matches('/');
        let res_root = project_root
            .join("res")
            .canonicalize()
            .map_err(|e| format!("failed to resolve project res root: {e}"))?;
        let source = rel
            .split('/')
            .fold(res_root.clone(), |path, component| path.join(component));
        if !source.exists() {
            return Ok(source);
        }
        let source = source
            .canonicalize()
            .map_err(|e| format!("failed to resolve project icon `{}`: {e}", source.display()))?;
        if !source.starts_with(&res_root) {
            return Err(format!(
                "project.icon escapes project res root: {}",
                source.display()
            ));
        }
        Ok(source)
    }

    fn builtin_icon_source_path(out_dir: &Path) -> Result<PathBuf, String> {
        let out = out_dir.join("perro_builtin_logo.svg");
        fs::write(&out, perro_api::builtin_assets::PERRO_LOGO_SVG)
            .map_err(|e| format!("failed to write builtin perro icon `{}`: {e}", out.display()))?;
        Ok(out)
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
    let mut icon_source = resolve_res_icon_path(&project_root, &icon_res)?;
    let out_dir = PathBuf::from(
        env::var("OUT_DIR").map_err(|e| format!("OUT_DIR missing: {e}"))?,
    );

    println!("cargo:rerun-if-changed={}", project_toml.display());
    println!("cargo:rerun-if-changed={}", icon_source.display());

    if !icon_source.exists() {
        icon_source = builtin_icon_source_path(&out_dir)?;
    }

    let ext = icon_source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let icon_for_resource = if ext == "ico" {
        icon_source
    } else {
        convert_icon_to_ico(&icon_source, &out_dir)?
    };

    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon_for_resource.to_string_lossy().as_ref());
    apply_windows_metadata(&mut res, &project_toml)?;
    res.compile()
        .map_err(|e| format!("failed to compile windows resource icon: {e}"))?;
    Ok(())
}
"#
    .to_string()
}

fn default_scripts_crate_toml() -> String {
    r#"[workspace]

[package]
name = "scripts"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
perro_api = "0.1.0"
perro_runtime = "0.1.0"

[features]
dynamic-scripts = []
steamworks = ["perro_api/steamworks", "perro_runtime/steamworks"]

[profile.dev]
opt-level = 0
incremental = true
codegen-units = 256
lto = false
debug = false
strip = "none"
overflow-checks = false
panic = "abort"

[profile.dev.package."*"]
opt-level = 2
incremental = true
codegen-units = 64
debug = false
strip = "none"
overflow-checks = false

[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ["cfg(rust_analyzer)"] }
"#
    .to_string()
}

fn default_scripts_cargo_config_toml() -> String {
    r#"[build]
target-dir = "../../target"
"#
    .to_string()
}

fn default_project_cargo_config_toml() -> String {
    r#"[build]
target-dir = "../../target"
"#
    .to_string()
}

fn default_dev_runner_crate_toml() -> String {
    r#"[workspace]

[package]
name = "perro_dev_runner"
version = "0.1.0"
edition = "2024"
build = "build.rs"

[dependencies]
perro_app = "0.1.0"
perro_project = "0.1.0"

[features]
timings = ["perro_app/fps"]
profile = ["perro_app/profile"]
ui_profile = ["perro_app/ui_profile"]
mem_profile = ["perro_app/mem_profile"]
steamworks = ["perro_app/steamworks"]

[target.'cfg(target_os = "windows")'.build-dependencies]
winresource = "0.1.20"
perro_api = "0.1.0"
toml = "0.8.23"
image = { version = "0.25.9", default-features = false, features = ["png", "jpeg", "gif", "bmp", "tga", "webp", "ico"] }
resvg = "0.47.0"

[profile.dev]
opt-level = 1

[profile.dev.package.perro_runtime]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.perro_app]
opt-level = 3

[profile.dev.package.perro_graphics]
opt-level = 3

[profile.dev.package.perro_physics]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.rapier2d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.rapier3d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.parry2d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.dev.package.parry3d]
opt-level = 3
debug-assertions = false
overflow-checks = false

[profile.release]
debug = true
"#
    .to_string()
}

fn default_dev_runner_main_rs() -> String {
    r#"use perro_app::{entry, winit_runner::AppExitKind};
use perro_project::resolve_local_path;
use std::{env, path::PathBuf, process};

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn current_dir_fallback() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let local_root = current_dir_fallback();

    let root = parse_flag_value(&args, "--path")
        .map(|p| resolve_local_path(&p, &local_root))
        .unwrap_or_else(|| local_root.clone());

    let fallback_name =
        parse_flag_value(&args, "--name").unwrap_or_else(|| "Perro Project".to_string());

    eprintln!("perro dev runner: start {}", root.to_string_lossy());
    let run_result = entry::run_dev_project_from_path(&root, &fallback_name);

    match run_result {
        Ok(result) => match result.kind {
            AppExitKind::WindowClose => println!("perro exit: window close"),
            AppExitKind::EventLoopExit => println!("perro exit: event loop exit"),
        },
        Err(err) => {
            eprintln!("perro exit error at `{}`: {err}", root.to_string_lossy());
            process::exit(1);
        }
    }
}
"#
    .to_string()
}

fn rel_path(from: &Path, to: &Path) -> String {
    let from_components: Vec<_> = from.components().collect();
    let to_components: Vec<_> = to.components().collect();
    let common = from_components
        .iter()
        .zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut out = PathBuf::new();
    for _ in common..from_components.len() {
        out.push("..");
    }
    for c in &to_components[common..] {
        out.push(c.as_os_str());
    }
    let path = out.to_string_lossy().replace('\\', "/");
    path.strip_prefix("//?/").unwrap_or(&path).to_string()
}

fn default_project_main_rs(project_name: &str) -> String {
    r#"#[path = "static/mod.rs"]
mod static_assets;

static PERRO_ASSETS: &[u8] = include_bytes!("../embedded/assets.perro");

#[used]
#[unsafe(no_mangle)]
pub static PERRO_ENGINE_DETECT: [u8; 89] =
    *b"PERRO_ENGINE_DETECT:v1;engine=Perro Engine;format=.perro;site=https://www.perroengine.com";


fn keep_perro_engine_marker() {
    // SAFETY: Reads stay within static marker bounds and use valid static pointers.
    unsafe {
        std::hint::black_box(std::ptr::read_volatile(PERRO_ENGINE_DETECT.as_ptr()));
        std::hint::black_box(std::ptr::read_volatile(
            PERRO_ENGINE_DETECT.as_ptr().add(PERRO_ENGINE_DETECT.len() - 1),
        ));
    }
}

fn project_root() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for dir in exe_dir.ancestors() {
                if dir.join("project.toml").exists() {
                    return dir.to_path_buf();
                }
            }
            return exe_dir.to_path_buf();
        }
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if root.join("project.toml").exists() {
        return root.canonicalize().unwrap_or(root);
    }
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

  fn main() {
      keep_perro_engine_marker();
      let root = project_root();
      perro_app::entry::run_static_embedded_project(perro_app::entry::StaticEmbeddedProject {
          project: perro_app::entry::StaticEmbeddedProjectInfo {
              project_root: &root,
              project_name: "__PROJECT_NAME__",
              main_scene_hash: 7300106721993353294u64,
              icon_hash: 6859512821849760879u64,
              startup_splash_hash: 6859512821849760879u64,
              virtual_width: 1920,
              virtual_height: 1080,
          },
          routes: perro_app::entry::StaticEmbeddedRoutesConfig {
              routes: &[perro_app::entry::StaticEmbeddedRoute {
                  href: "/",
                  name: "main",
                  scene_hash: 7300106721993353294u64,
              }],
          },
          input: perro_app::entry::StaticEmbeddedInputMapConfig {
              actions: &[perro_app::entry::StaticEmbeddedInputAction {
                  name: "jump",
                  keys: &[perro_input_api::KeyCode::Space, perro_input_api::KeyCode::ArrowUp],
                  mouse: &[],
                  gamepad: &[],
                  joycon: &[],
              }],
          },
          graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {
              vsync: false,
              msaa: true,
              meshlets: false,
              dev_meshlets: false,
              release_meshlets: true,
              meshlet_debug_view: false,
              occlusion_culling: perro_app::entry::OcclusionCulling::Gpu,
              particle_sim_default: perro_app::entry::ParticleSimDefault::Cpu,
              ui_pixel_snapping: true,
          },
          runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {
              target_fixed_update: Some(60.0),
              frame_rate_cap: perro_app::entry::FrameRateCap::Unlimited,
              physics_gravity: -9.81,
              physics_coef: 1.0,
          },
          metadata: perro_app::entry::StaticEmbeddedMetadataConfig {
              description: None,
              company: None,
              version: None,
              copyright: None,
              trademark: None,
          },
          localization: perro_app::entry::StaticEmbeddedLocalizationConfig {
              default_locale: "en",
          },
          steam: perro_app::entry::StaticEmbeddedSteamConfig {
              enabled: false,
              app_id: None,
              input_mode: perro_runtime::SteamInputMode::Off,
          },
          assets: perro_app::entry::StaticEmbeddedAssetsConfig {
              perro_assets: PERRO_ASSETS,
              scene_lookup: static_assets::scenes::lookup_scene,
              localization_lookup: static_assets::localizations::lookup_localized_string,
              material_lookup: static_assets::materials::lookup_material,
              ui_style_lookup: static_assets::ui_styles::lookup_ui_style,
              tileset_lookup: static_assets::tilesets::lookup_tileset,
              particle_lookup: static_assets::particles::lookup_particle,
              animation_lookup: static_assets::animations::lookup_animation,
              animation_tree_lookup: static_assets::animation_trees::lookup_animation_tree,
              mesh_lookup: static_assets::meshes::lookup_mesh,
              collision_trimesh_lookup: static_assets::collision_trimeshes::lookup_collision_trimesh,
              navmesh_lookup: static_assets::navmeshes::lookup_navmesh,
              skeleton_lookup: static_assets::skeletons::lookup_skeleton,
              texture_lookup: static_assets::textures::lookup_texture,
              shader_lookup: static_assets::shaders::lookup_shader,
              audio_lookup: static_assets::audios::lookup_audio,
              static_script_registry: Some(scripts::SCRIPT_REGISTRY),
          },
      })
      .expect("failed to run embedded static project");
  }
"#
    .replace("__PROJECT_NAME__", project_name)
}

fn default_static_mod_rs() -> String {
    "#![allow(unused_imports)]\n\npub mod scenes;\npub mod materials;\npub mod ui_styles;\npub mod tilesets;\npub mod particles;\npub mod animations;\npub mod animation_trees;\npub mod meshes;\npub mod collision_trimeshes;\npub mod navmeshes;\npub mod skeletons;\npub mod textures;\npub mod shaders;\npub mod audios;\npub mod localizations;\n".to_string()
}

fn default_static_scenes_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_scene::Scene;

const EMPTY_SCENE_NODES: &[perro_scene::SceneNodeEntry] = &[];
const EMPTY_SCENE_KEY_NAMES: &[std::borrow::Cow<'static, str>] = &[];
const EMPTY_SCENE: Scene = Scene {
    nodes: std::borrow::Cow::Borrowed(EMPTY_SCENE_NODES),
    root: None,
    key_names: std::borrow::Cow::Borrowed(EMPTY_SCENE_KEY_NAMES),
};

pub const fn lookup_scene(_path_hash: u64) -> &'static Scene {
    &EMPTY_SCENE
}
"#
    .to_string()
}

fn default_static_materials_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_render_bridge::{Material3D, StandardMaterial3D};

const EMPTY_MATERIAL: Material3D = Material3D::Standard(StandardMaterial3D::const_default());

pub const fn lookup_material(_path_hash: u64) -> &'static Material3D {
    &EMPTY_MATERIAL
}
"#
    .to_string()
}

fn default_static_ui_styles_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_api::ui::UiStyle;

const EMPTY_UI_STYLE: UiStyle = UiStyle::panel();

pub const fn lookup_ui_style(_path_hash: u64) -> &'static UiStyle {
    &EMPTY_UI_STYLE
}
"#
    .to_string()
}

fn default_static_animation_trees_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_animation::AnimationTreeAsset;
use std::borrow::Cow;

const EMPTY_SLOTS: &[perro_animation::AnimationTreeSlot] = &[];
const EMPTY_NODES: &[perro_animation::AnimationTreeGraphNode] = &[];
static EMPTY_ANIMATION_TREE: AnimationTreeAsset = AnimationTreeAsset {
    name: Cow::Borrowed(""),
    slots: Cow::Borrowed(EMPTY_SLOTS),
    nodes: Cow::Borrowed(EMPTY_NODES),
    output: Cow::Borrowed(""),
};

pub const fn lookup_animation_tree(_path_hash: u64) -> &'static AnimationTreeAsset {
    &EMPTY_ANIMATION_TREE
}
"#
    .to_string()
}

fn default_static_localizations_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_localized_string(
    _locale: perro_api::resource_api::sub_apis::Locale,
    _key_hash: u64,
) -> &'static str {
    ""
}
"#
    .to_string()
}

fn default_static_tilesets_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_tileset(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_particles_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_render_bridge::{ParticlePath3D, ParticleProfile3D};

const EMPTY_PARTICLE: ParticleProfile3D = ParticleProfile3D {
    path: ParticlePath3D::None,
    expr_x_ops: None,
    expr_y_ops: None,
    expr_z_ops: None,
    lifetime_min: 0.6,
    lifetime_max: 1.4,
    speed_min: 1.0,
    speed_max: 3.0,
    spread_radians: core::f32::consts::FRAC_PI_3,
    size: 6.0,
    size_min: 0.65,
    size_max: 1.35,
    force: [0.0, 0.0, 0.0],
    color_start: [1.0, 1.0, 1.0, 1.0],
    color_end: [1.0, 0.4, 0.1, 0.0],
    emissive: [0.0, 0.0, 0.0],
    spin_angular_velocity: 0.0,
};

pub const fn lookup_particle(_path_hash: u64) -> &'static ParticleProfile3D {
    &EMPTY_PARTICLE
}
"#
    .to_string()
}

fn default_static_animations_rs() -> String {
    r#"#![allow(unused_imports)]

use perro_animation::AnimationClip;

const EMPTY_ANIMATION_CLIP: AnimationClip = AnimationClip {
    name: std::borrow::Cow::Borrowed(""),
    fps: 0.0,
    total_frames: 0,
    objects: std::borrow::Cow::Borrowed(&[]),
    object_tracks: std::borrow::Cow::Borrowed(&[]),
    frame_events: std::borrow::Cow::Borrowed(&[]),
};

pub const fn lookup_animation(_path_hash: u64) -> &'static AnimationClip {
    &EMPTY_ANIMATION_CLIP
}
"#
    .to_string()
}

fn default_static_textures_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_texture(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_shaders_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_shader(_path_hash: u64) -> &'static str {
    ""
}
"#
    .to_string()
}

fn default_static_meshes_rs() -> String {
    r#"#![allow(unused_imports)]
#![allow(dead_code)]

pub const fn lookup_mesh(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_collision_trimeshes_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_collision_trimesh(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_navmeshes_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_navmesh(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_skeletons_rs() -> String {
    r#"#![allow(unused_imports)]
#![allow(dead_code)]

pub const fn lookup_skeleton(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_static_audios_rs() -> String {
    r#"#![allow(unused_imports)]

pub const fn lookup_audio(_path_hash: u64) -> &'static [u8] {
    b""
}
"#
    .to_string()
}

fn default_scripts_lib_rs() -> String {
    r#"use perro_runtime::RuntimeScriptApi;
use perro_api::scripting::ScriptConstructor;

pub static SCRIPT_REGISTRY: &[(u64, ScriptConstructor<RuntimeScriptApi>)] = &[];

#[cfg(feature = "dynamic-scripts")]
#[unsafe(no_mangle)]
pub extern "C" fn perro_scripts_init() {}
"#
    .to_string()
}
