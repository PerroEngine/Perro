#[derive(Clone, Copy, Debug)]
pub struct ProjectBuildOptions {
    pub profile: bool,
    pub console: bool,
    pub release: bool,
    pub target: ProjectBuildTarget,
    pub web_output_dir: WebOutputDir,
    pub android_sdk_root: Option<&'static str>,
    pub android_ndk_root: Option<&'static str>,
    pub headless: bool,
    pub native_target: Option<&'static str>,
    pub demo: bool,
    /// Discard every incremental pipeline cache (embedded blobs, manifests,
    /// archive stat sidecar) and re-encode all assets from source.
    pub fresh: bool,
}

impl ProjectBuildOptions {
    pub fn new(profile: bool, console: bool) -> Self {
        Self {
            profile,
            console,
            release: true,
            target: ProjectBuildTarget::Native,
            web_output_dir: WebOutputDir::Build,
            android_sdk_root: None,
            android_ndk_root: None,
            headless: false,
            native_target: None,
            demo: false,
            fresh: false,
        }
    }

    pub fn with_fresh(mut self, fresh: bool) -> Self {
        self.fresh = fresh;
        self
    }

    pub fn with_target(mut self, target: ProjectBuildTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_release(mut self, release: bool) -> Self {
        self.release = release;
        self
    }

    pub fn with_headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    pub fn with_native_target(mut self, target: Option<&'static str>) -> Self {
        self.native_target = target;
        self
    }

    pub fn with_demo(mut self, demo: bool) -> Self {
        self.demo = demo;
        self
    }

    pub fn with_web_output_dir(mut self, output_dir: WebOutputDir) -> Self {
        self.web_output_dir = output_dir;
        self
    }

    pub fn with_android_sdk_root(mut self, sdk_root: Option<&'static str>) -> Self {
        self.android_sdk_root = sdk_root;
        self
    }

    pub fn with_android_ndk_root(mut self, ndk_root: Option<&'static str>) -> Self {
        self.android_ndk_root = ndk_root;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectBuildTarget {
    Native,
    Web,
    Android,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebOutputDir {
    Build,
    Dev,
}

pub fn compile_project_bundle(
    project_root: &Path,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
    let cfg = perro_project::load_project_toml_with_demo(project_root, options.demo)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    validate_demo_entry_paths(&cfg)?;
    perro_project::ensure_build_crates_scaffold(project_root, &cfg.name)?;
    ensure_source_overrides(project_root)?;
    sync_android_project_manifest(project_root, &cfg, options)?;
    if options.fresh {
        reset_embedded_dir(project_root)?;
    }
    sweep_unknown_embedded_entries(project_root)?;
    let _path_filter = perro_io::walkdir::push_path_exclusions(cfg.demo.relative_patterns());
    let _demo_mode = perro_static_pipeline::push_demo_mode(options.demo);
    let _ = sync_scripts(project_root)?;
    generate_project_static_modules(project_root, &cfg)?;
    perro_static_pipeline::write_static_mod_rs(project_root)
        .map_err(|err| CompilerError::SceneParse(format!("static mod generation failed: {err}")))?;
    generate_embedded_entry_files_with_options(project_root, options)?;
    generate_perro_assets(project_root)?;
    build_project_crate(
        project_root,
        options,
        cfg.steam.enabled,
        cfg.metadata.version.as_deref(),
    )?;
    Ok(())
}

fn validate_demo_entry_paths(cfg: &perro_project::ProjectConfig) -> Result<(), CompilerError> {
    for (field, path) in [
        ("project.main_scene", cfg.main_scene.as_str()),
        ("project.icon", cfg.icon.as_str()),
        ("project.startup_splash", cfg.startup_splash.as_str()),
    ] {
        if cfg.demo.excludes(path) {
            return Err(CompilerError::SceneParse(format!(
                "`{field}` refs demo-excluded path `{path}`"
            )));
        }
    }
    Ok(())
}

pub fn compile_universal_macos_project_bundle(
    project_root: &Path,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
    if !cfg!(target_os = "macos") {
        return Err(CompilerError::SceneParse(
            "universal macOS builds require a macOS host".to_string(),
        ));
    }
    if options.target != ProjectBuildTarget::Native {
        return Err(CompilerError::SceneParse(
            "universal macOS builds require the native project target".to_string(),
        ));
    }

    const ARM_TARGET: &str = "aarch64-apple-darwin";
    const INTEL_TARGET: &str = "x86_64-apple-darwin";
    compile_project_bundle(project_root, options.with_native_target(Some(ARM_TARGET)))?;
    compile_project_bundle(project_root, options.with_native_target(Some(INTEL_TARGET)))?;
    export_universal_macos_binary(project_root, options.release, options.demo)?;
    Ok(())
}

fn generate_perro_assets(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    fs::create_dir_all(&embedded_dir)?;
    let output = embedded_dir.join("assets.perro");
    let res_dir = project_root.join("res");
    build_perro_assets_archive(&output, &res_dir, project_root, &[])?;
    Ok(())
}

/// `--fresh` escape hatch: wipe the embedded dir wholesale so every manifest,
/// blob, and stat sidecar is gone and the pipeline re-encodes from source.
fn reset_embedded_dir(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    if embedded_dir.exists() {
        fs::remove_dir_all(&embedded_dir)?;
    }
    Ok(())
}

// Generators own incremental cleanup inside their kind dirs (unchanged
// outputs keep their mtimes so cargo skips recompiling the project crate).
// This sweep only removes top-level entries no generator claims, e.g.
// leftovers from an older engine layout.
fn sweep_unknown_embedded_entries(project_root: &Path) -> Result<(), CompilerError> {
    const KNOWN: &[&str] = &[
        "scenes",
        "materials",
        "ui_styles",
        "tilesets",
        "particles",
        "animations",
        "animation_trees",
        "meshes",
        "collision_trimeshes",
        "navmeshes",
        "skeletons",
        "textures",
        "fonts",
        "shaders",
        "audios",
        "csvs",
        "localizations",
        "assets.perro",
        "assets.perro.stat",
    ];
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    fs::create_dir_all(&embedded_dir)?;
    for entry in fs::read_dir(&embedded_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let known = name.to_str().is_some_and(|name| KNOWN.contains(&name));
        if known {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn build_project_crate(
    project_root: &Path,
    options: ProjectBuildOptions,
    steam_enabled: bool,
    version: Option<&str>,
) -> Result<(), CompilerError> {
    let project_crate = project_root.join(".perro").join("project");
    let target_dir = project_root.join("target");
    let mut cmd = Command::new("cargo");
    cmd.env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&project_crate);
    if options.target == ProjectBuildTarget::Web {
        cmd.arg("build")
            .arg("--lib")
            .arg("--target")
            .arg("wasm32-unknown-unknown");
        cmd.env(
            "RUSTFLAGS",
            append_rustflag(
                env::var_os("RUSTFLAGS"),
                "--cfg getrandom_backend=\"wasm_js\"",
            ),
        );
    } else if options.target == ProjectBuildTarget::Android {
        cmd.arg("apk")
            .arg("build")
            .arg("--lib")
            .arg("--target")
            .arg("aarch64-linux-android");
    } else {
        cmd.arg("build");
        if let Some(target) = options.native_target {
            validate_native_target_triple(target)?;
            cmd.arg("--target").arg(target);
        }
    }
    if options.release {
        cmd.arg("--release");
    }
    if options.target == ProjectBuildTarget::Native && !options.console && !options.headless {
        cmd.env(
            "RUSTFLAGS",
            append_rustflag(env::var_os("RUSTFLAGS"), "--cfg perro_no_console"),
        );
    }
    if let Some(sdk_root) = options.android_sdk_root {
        cmd.env("ANDROID_SDK_ROOT", sdk_root)
            .env("ANDROID_HOME", sdk_root);
    }
    if let Some(ndk_root) = options.android_ndk_root {
        cmd.env("ANDROID_NDK_ROOT", ndk_root)
            .env("ANDROID_NDK_HOME", ndk_root)
            .env("NDK_HOME", ndk_root);
    }
    if options.demo {
        cmd.env("PERRO_DEMO", "1");
    }
    let mut features = Vec::new();
    if options.headless {
        cmd.arg("--no-default-features");
        features.push("headless");
    }
    if options.profile {
        features.push(if options.headless {
            "headless_profile"
        } else {
            "profile"
        });
    }
    if steam_enabled {
        features.push(if options.headless {
            "headless_steamworks"
        } else {
            "steamworks"
        });
    }
    if options.demo {
        features.push("perro-demo");
    }
    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }
    let android_apk = if options.target == ProjectBuildTarget::Android {
        let path = android_apk_artifact_path(project_root, &target_dir, options.release)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Some(path)
    } else {
        None
    };
    let status = cmd.status()?;

    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    match options.target {
        ProjectBuildTarget::Native => export_project_binary(
            project_root,
            &target_dir,
            options.release,
            steam_enabled,
            version,
            options.native_target,
            options.demo,
        )?,
        ProjectBuildTarget::Web => export_project_web_bundle(project_root, &target_dir, options)?,
        ProjectBuildTarget::Android => export_project_android_bundle(
            project_root,
            android_apk
                .as_deref()
                .expect("android build must resolve one apk path"),
            options.demo,
        )?,
    }
    Ok(())
}

fn validate_native_target_triple(target: &str) -> Result<(), CompilerError> {
    let valid = !target.is_empty()
        && !target.starts_with('-')
        && target.contains('-')
        && target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(CompilerError::SceneParse(format!(
            "invalid native Rust target triple `{target}`"
        )))
    }
}

fn append_rustflag(existing: Option<std::ffi::OsString>, flag: &str) -> std::ffi::OsString {
    let mut out = existing.unwrap_or_default();
    if !out.is_empty() {
        out.push(" ");
    }
    out.push(flag);
    out
}

fn export_project_binary(
    project_root: &Path,
    target_dir: &Path,
    release: bool,
    steam_enabled: bool,
    version: Option<&str>,
    native_target: Option<&str>,
    demo: bool,
) -> Result<(), CompilerError> {
    let package_bin_name = read_project_package_name(project_root)?;
    let output_bin_name = read_project_output_binary_name(project_root, &package_bin_name, demo)?;
    let profile_dir = if release { "release" } else { "debug" };
    let artifact_dir = native_artifact_dir(target_dir, profile_dir, native_target);
    let built_bin = artifact_dir.join(target_binary_name(&package_bin_name, native_target));
    if !built_bin.exists() {
        return Err(CompilerError::SceneParse(format!(
            "project binary not found after build: {}",
            built_bin.display()
        )));
    }

    let output_dir = project_root
        .join(".output")
        .join(native_output_folder_name(&output_bin_name, native_target));
    fs::create_dir_all(&output_dir)?;
    let copied_bin = output_dir.join(target_binary_name(&package_bin_name, native_target));
    let output_bin = output_dir.join(target_binary_name(
        &native_output_artifact_name(&output_bin_name, version, native_target),
        native_target,
    ));
    fs::copy(&built_bin, &copied_bin)?;
    rename_exported_binary(&copied_bin, &output_bin)?;
    if steam_enabled {
        let _ = copy_steam_runtime_library(&artifact_dir, &output_dir, native_target)?;
    }
    println!("exported project binary: {}", output_bin.display());
    Ok(())
}

fn export_universal_macos_binary(
    project_root: &Path,
    release: bool,
    demo: bool,
) -> Result<(), CompilerError> {
    let cfg = perro_project::load_project_toml_with_demo(project_root, demo)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    let package_bin_name = read_project_package_name(project_root)?;
    let output_bin_name = read_project_output_binary_name(project_root, &package_bin_name, demo)?;
    let artifact_name = format!(
        "{}-macos-universal-v{}",
        package_name_slug(&output_bin_name),
        package_name_slug(cfg.metadata.version.as_deref().unwrap_or("0.1.0"))
    );
    let output_dir = project_root.join(".output").join(format!(
        "{}-macos-universal",
        package_name_slug(&output_bin_name)
    ));
    fs::create_dir_all(&output_dir)?;
    let arm_bin = exported_native_binary_path(
        project_root,
        &output_bin_name,
        cfg.metadata.version.as_deref(),
        "aarch64-apple-darwin",
    );
    let intel_bin = exported_native_binary_path(
        project_root,
        &output_bin_name,
        cfg.metadata.version.as_deref(),
        "x86_64-apple-darwin",
    );
    let output_bin = output_dir.join(artifact_name);
    run_lipo(&arm_bin, &intel_bin, &output_bin)?;

    if cfg.steam.enabled {
        let arm_dir = native_artifact_dir(
            &project_root.join("target"),
            if release { "release" } else { "debug" },
            Some("aarch64-apple-darwin"),
        );
        let steam_lib = find_steam_runtime_library(&arm_dir.join("build"), "libsteam_api.dylib")
            .ok_or_else(|| {
                CompilerError::SceneParse(
                    "Steam enabled but macOS Steam runtime is missing".to_string(),
                )
            })?;
        fs::copy(steam_lib, output_dir.join("libsteam_api.dylib"))?;
    }
    println!(
        "exported universal macOS project binary: {}",
        output_bin.display()
    );
    Ok(())
}

fn exported_native_binary_path(
    project_root: &Path,
    output_name: &str,
    version: Option<&str>,
    target: &str,
) -> PathBuf {
    project_root
        .join(".output")
        .join(native_output_folder_name(output_name, Some(target)))
        .join(target_binary_name(
            &native_output_artifact_name(output_name, version, Some(target)),
            Some(target),
        ))
}

fn run_lipo(first: &Path, second: &Path, output: &Path) -> Result<(), CompilerError> {
    let status = Command::new("lipo")
        .arg("-create")
        .arg(first)
        .arg(second)
        .arg("-output")
        .arg(output)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)))
    }
}

fn native_artifact_dir(target_dir: &Path, profile: &str, target: Option<&str>) -> PathBuf {
    match target {
        Some(target) => target_dir.join(target).join(profile),
        None => target_dir.join(profile),
    }
}

fn native_output_folder_name(output_name: &str, target: Option<&str>) -> String {
    format!(
        "{}-{}",
        package_name_slug(output_name),
        native_system_slug(target)
    )
}

fn native_output_artifact_name(
    output_name: &str,
    version: Option<&str>,
    target: Option<&str>,
) -> String {
    format!(
        "{}-{}-v{}",
        package_name_slug(output_name),
        native_system_slug(target),
        package_name_slug(version.unwrap_or("0.1.0"))
    )
}

fn package_name_slug(name: &str) -> String {
    let slug = name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '_',
        })
        .collect::<String>();
    if slug.is_empty() {
        "perro-project".to_string()
    } else {
        slug
    }
}

fn host_os_slug() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        std::env::consts::OS
    }
}

fn host_system_slug() -> String {
    rustc_default_host_triple()
        .and_then(|triple| target_slug_from_triple(&triple))
        .unwrap_or_else(|| format!("{}-{}", host_os_slug(), std::env::consts::ARCH))
}

fn native_system_slug(target: Option<&str>) -> String {
    target
        .and_then(target_slug_from_triple)
        .unwrap_or_else(host_system_slug)
}

fn target_binary_name(bin_name: &str, target: Option<&str>) -> String {
    let windows = target
        .map(|target| target.contains("windows"))
        .unwrap_or_else(|| cfg!(target_os = "windows"));
    if windows {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    }
}

fn rustc_default_host_triple() -> Option<String> {
    let output = Command::new("rustc").arg("-vV").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout.lines().find_map(|line| {
        line.strip_prefix("host:")
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(str::to_string)
    })
}

fn target_slug_from_triple(triple: &str) -> Option<String> {
    let arch = triple.split('-').next()?.trim();
    if arch.is_empty() {
        return None;
    }
    let os = if triple.contains("windows") {
        "windows"
    } else if triple.contains("apple-darwin") {
        "macos"
    } else if triple.contains("linux") {
        "linux"
    } else {
        triple.split('-').nth(2).unwrap_or(std::env::consts::OS)
    };
    Some(format!(
        "{}-{}",
        package_name_slug(os),
        package_name_slug(arch)
    ))
}

fn copy_steam_runtime_library(
    artifact_dir: &Path,
    output_dir: &Path,
    native_target: Option<&str>,
) -> Result<Option<PathBuf>, CompilerError> {
    let Some(library_name) = steam_runtime_library_name(native_target) else {
        return Ok(None);
    };
    let build_dir = artifact_dir.join("build");
    let source = find_steam_runtime_library(&build_dir, library_name).ok_or_else(|| {
        CompilerError::SceneParse(format!(
            "Steam enabled but {library_name} was not found under {}",
            build_dir.display()
        ))
    })?;
    let target = output_dir.join(library_name);
    fs::copy(&source, &target)?;
    Ok(Some(target))
}

fn steam_runtime_library_name(native_target: Option<&str>) -> Option<&'static str> {
    let target = native_target.unwrap_or_default();
    if target.contains("windows") || (target.is_empty() && cfg!(target_os = "windows")) {
        if target.starts_with("i686-")
            || (target.is_empty()
                && cfg!(target_os = "windows")
                && cfg!(target_pointer_width = "32"))
        {
            Some("steam_api.dll")
        } else {
            Some("steam_api64.dll")
        }
    } else if target.contains("linux") || (target.is_empty() && cfg!(target_os = "linux")) {
        Some("libsteam_api.so")
    } else if target.contains("apple-darwin") || (target.is_empty() && cfg!(target_os = "macos")) {
        Some("libsteam_api.dylib")
    } else {
        None
    }
}

fn find_steam_runtime_library(build_dir: &Path, library_name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(build_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path().join("out").join(library_name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn android_apk_artifact_path(
    project_root: &Path,
    target_dir: &Path,
    release: bool,
) -> Result<PathBuf, CompilerError> {
    let package_name = read_project_package_name(project_root)?;
    let library_name = read_project_library_name(project_root, &package_name)?;
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let source = fs::read_to_string(&manifest_path)?;
    let manifest = toml::from_str::<toml::Value>(&source).map_err(|err| {
        CompilerError::SceneParse(format!(
            "failed to parse generated project manifest {}: {err}",
            manifest_path.display()
        ))
    })?;
    let apk_name = manifest
        .get("package")
        .and_then(|package| package.get("metadata"))
        .and_then(|metadata| metadata.get("android"))
        .and_then(|android| android.get("apk_name"))
        .and_then(toml::Value::as_str)
        .unwrap_or(&library_name);
    if apk_name.is_empty()
        || Path::new(apk_name)
            .file_name()
            .is_none_or(|name| name != std::ffi::OsStr::new(apk_name))
    {
        return Err(CompilerError::SceneParse(format!(
            "invalid Android apk_name `{apk_name}` in {}",
            manifest_path.display()
        )));
    }
    let profile_dir = if release { "release" } else { "debug" };
    Ok(target_dir
        .join(profile_dir)
        .join("apk")
        .join(format!("{apk_name}.apk")))
}

#[path = "project_bundle/android.rs"]
mod android;
pub(crate) use android::*;
#[path = "project_bundle/web.rs"]
mod web;
pub(crate) use web::*;
#[path = "project_bundle/codegen.rs"]
mod codegen;
pub(crate) use codegen::*;
