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
        }
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
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    perro_project::ensure_build_crates_scaffold(project_root, &cfg.name)?;
    ensure_source_overrides(project_root)?;
    sync_android_project_manifest(project_root, &cfg, options)?;
    reset_embedded_dir(project_root)?;
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

fn generate_perro_assets(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    fs::create_dir_all(&embedded_dir)?;
    let output = embedded_dir.join("assets.perro");
    let res_dir = project_root.join("res");
    build_perro_assets_archive(&output, &res_dir, project_root, &[])?;
    Ok(())
}

fn reset_embedded_dir(project_root: &Path) -> Result<(), CompilerError> {
    let embedded_dir = project_root.join(".perro").join("project").join("embedded");
    if embedded_dir.exists() {
        fs::remove_dir_all(&embedded_dir)?;
    }
    fs::create_dir_all(&embedded_dir)?;
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
        )?,
        ProjectBuildTarget::Web => export_project_web_bundle(project_root, &target_dir, options)?,
        ProjectBuildTarget::Android => export_project_android_bundle(
            project_root,
            android_apk
                .as_deref()
                .expect("android build must resolve one apk path"),
        )?,
    }
    Ok(())
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
) -> Result<(), CompilerError> {
    let package_bin_name = read_project_package_name(project_root)?;
    let output_bin_name = read_project_output_binary_name(project_root, &package_bin_name)?;
    let profile_dir = if release { "release" } else { "debug" };
    let built_bin = target_dir
        .join(profile_dir)
        .join(platform_binary_name(&package_bin_name));
    if !built_bin.exists() {
        return Err(CompilerError::SceneParse(format!(
            "project binary not found after build: {}",
            built_bin.display()
        )));
    }

    let output_dir = project_root
        .join(".output")
        .join(native_output_folder_name(&output_bin_name));
    fs::create_dir_all(&output_dir)?;
    let copied_bin = output_dir.join(platform_binary_name(&package_bin_name));
    let output_bin = output_dir.join(platform_binary_name(&native_output_artifact_name(
        &output_bin_name,
        version,
    )));
    fs::copy(&built_bin, &copied_bin)?;
    rename_exported_binary(&copied_bin, &output_bin)?;
    if steam_enabled {
        let _ = copy_steam_runtime_library(target_dir, profile_dir, &output_dir)?;
    }
    println!("exported project binary: {}", output_bin.display());
    Ok(())
}

fn native_output_folder_name(output_name: &str) -> String {
    format!("{}-{}", package_name_slug(output_name), host_system_slug())
}

fn native_output_artifact_name(output_name: &str, version: Option<&str>) -> String {
    format!(
        "{}-{}-v{}",
        package_name_slug(output_name),
        host_system_slug(),
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
    target_dir: &Path,
    profile_dir: &str,
    output_dir: &Path,
) -> Result<Option<PathBuf>, CompilerError> {
    let Some(library_name) = steam_runtime_library_name() else {
        return Ok(None);
    };
    let build_dir = target_dir.join(profile_dir).join("build");
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

fn steam_runtime_library_name() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        Some("steam_api64.dll")
    } else if cfg!(target_os = "linux") {
        Some("libsteam_api.so")
    } else if cfg!(target_os = "macos") {
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

fn export_project_android_bundle(
    project_root: &Path,
    built_apk: &Path,
) -> Result<(), CompilerError> {
    let output_name =
        read_project_output_binary_name(project_root, &read_project_package_name(project_root)?)?;
    if !built_apk.is_file() {
        return Err(CompilerError::SceneParse(format!(
            "android apk not found after build: {}",
            built_apk.display()
        )));
    }
    if !built_apk
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("apk"))
    {
        return Err(CompilerError::SceneParse(format!(
            "android build artifact is not an apk: {}",
            built_apk.display()
        )));
    }

    let output_dir = project_root.join(".output").join("android");
    fs::create_dir_all(&output_dir)?;
    let output_apk = output_dir.join(format!("{output_name}.apk"));
    fs::copy(built_apk, &output_apk)?;
    println!("exported android apk: {}", output_apk.display());
    Ok(())
}

fn rename_exported_binary(source: &Path, dest: &Path) -> Result<(), CompilerError> {
    if source == dest {
        return Ok(());
    }

    let source_str = source.to_string_lossy();
    let dest_str = dest.to_string_lossy();
    let case_only_rename =
        cfg!(target_os = "windows") && source_str.eq_ignore_ascii_case(&dest_str);

    if case_only_rename {
        return rename_exported_binary_via_temp(source, dest);
    }

    if dest.exists() {
        fs::remove_file(dest)?;
    }

    match fs::rename(source, dest) {
        Ok(()) => Ok(()),
        Err(err) => Err(CompilerError::Io(err)),
    }
}

fn rename_exported_binary_via_temp(source: &Path, dest: &Path) -> Result<(), CompilerError> {
    let Some(parent) = source.parent() else {
        return Err(CompilerError::SceneParse(format!(
            "failed to rename export: source has no parent: {}",
            source.display()
        )));
    };
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mut tmp = parent.join(if ext.is_empty() {
        "__perro_export_tmp__".to_string()
    } else {
        format!("__perro_export_tmp__.{ext}")
    });
    let mut idx = 0usize;
    while tmp.exists() {
        idx += 1;
        tmp = parent.join(if ext.is_empty() {
            format!("__perro_export_tmp__{idx}")
        } else {
            format!("__perro_export_tmp__{idx}.{ext}")
        });
    }
    fs::rename(source, &tmp)?;
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    fs::rename(tmp, dest)?;
    Ok(())
}

fn platform_binary_name(bin_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    }
}

fn read_project_package_name(project_root: &Path) -> Result<String, CompilerError> {
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let source = fs::read_to_string(&manifest_path)?;
    let mut in_package = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let Some((_, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = raw_value.trim().trim_matches('"');
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }

    Err(CompilerError::SceneParse(format!(
        "failed to resolve package.name from {}",
        manifest_path.display()
    )))
}

fn read_project_library_name(
    project_root: &Path,
    fallback_name: &str,
) -> Result<String, CompilerError> {
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let source = fs::read_to_string(&manifest_path)?;
    let mut in_lib = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_lib = trimmed == "[lib]";
            continue;
        }
        if !in_lib || !trimmed.starts_with("name") {
            continue;
        }
        let Some((_, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = raw_value.trim().trim_matches('"');
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }
    Ok(fallback_name.to_string())
}

fn read_project_output_binary_name(
    project_root: &Path,
    fallback_name: &str,
) -> Result<String, CompilerError> {
    let config = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    let sanitized = sanitize_output_binary_name(&config.name);
    if sanitized.is_empty() {
        Ok(fallback_name.to_string())
    } else {
        Ok(sanitized)
    }
}

fn sanitize_output_binary_name(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.trim().chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid || c.is_control() {
            out.push('_');
        } else {
            out.push(c);
        }
    }

    out.trim_matches([' ', '.']).to_string()
}

fn ensure_project_dependency_line(
    project_root: &Path,
    crate_name: &str,
    dependency_line: &str,
) -> Result<(), CompilerError> {
    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let mut src = fs::read_to_string(&manifest_path)?;

    let dotted_dependency = format!("[dependencies.{crate_name}]");
    let mut in_dependencies = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed == dotted_dependency {
            return Ok(());
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies {
            continue;
        }
        if trimmed.starts_with(&format!("{crate_name} "))
            || trimmed.starts_with(&format!("{crate_name}="))
        {
            return Ok(());
        }
    }

    if let Some(idx) = src.find("[dependencies]") {
        let insert_pos = src[idx..]
            .find('\n')
            .map(|off| idx + off + 1)
            .unwrap_or(src.len());
        src.insert_str(insert_pos, &format!("{dependency_line}\n"));
        write_string_if_changed(&manifest_path, &src)?;
    } else if let Some(idx) = src.find("[dependencies.") {
        src.insert_str(idx, &format!("[dependencies]\n{dependency_line}\n\n"));
        write_string_if_changed(&manifest_path, &src)?;
    }
    Ok(())
}

#[cfg(test)]
fn generate_embedded_entry_files(project_root: &Path) -> Result<(), CompilerError> {
    generate_embedded_entry_files_with_options(project_root, ProjectBuildOptions::new(false, false))
}

fn generate_embedded_entry_files_with_options(
    project_root: &Path,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    let routes = perro_project::load_routes_toml(project_root, &cfg)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load routes.toml: {e}")))?;
    let project_src = project_root.join(".perro").join("project").join("src");
    fs::create_dir_all(project_src.join("static"))?;
    ensure_project_dependency_line(project_root, "perro_scene", "perro_scene = \"0.1.0\"")?;
    ensure_project_dependency_line(
        project_root,
        "perro_render_bridge",
        "perro_render_bridge = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_runtime", "perro_runtime = \"0.1.0\"")?;
    ensure_project_dependency_line(
        project_root,
        "perro_input_api",
        "perro_input_api = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_ids", "perro_ids = \"0.1.0\"")?;
    ensure_project_dependency_line(project_root, "perro_csv", "perro_csv = \"0.1.0\"")?;
    ensure_project_dependency_line(
        project_root,
        "perro_animation",
        "perro_animation = \"0.1.0\"",
    )?;
    ensure_project_dependency_line(project_root, "perro_structs", "perro_structs = \"0.1.0\"")?;
    perro_project::ensure_source_overrides(project_root)?;

    let native_entry = "run_static_embedded_project";
    let mut embedded_block = format!(
        "let root = project_root();\n\
perro_app::entry::{native_entry}(perro_app::entry::StaticEmbeddedProject {{\n\
  project: perro_app::entry::StaticEmbeddedProjectInfo {{\n\
        project_root: &root,\n\
        project_name: \"{name}\",\n\
        main_scene_hash: {main_scene_hash}u64,\n\
        icon_hash: {icon_hash}u64,\n\
        startup_splash_hash: {startup_splash_hash}u64,\n\
        virtual_width: {w},\n\
        virtual_height: {h},\n\
  }},\n\
  routes: perro_app::entry::StaticEmbeddedRoutesConfig {{\n\
        routes: {routes_block},\n\
  }},\n\
  input: perro_app::entry::StaticEmbeddedInputMapConfig {{\n\
        actions: {input_map_block},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        msaa: {msaa},\n\
        ssao: {ssao},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
        ui_pixel_snapping: {ui_pixel_snapping},\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        frame_rate_cap: {frame_rate_cap},\n\
        physics_gravity: {physics_gravity},\n\
        physics_coef: {physics_coef},\n\
  }},\n\
  metadata: perro_app::entry::StaticEmbeddedMetadataConfig {{\n\
        description: {metadata_description},\n\
        company: {metadata_company},\n\
        version: {metadata_version},\n\
        copyright: {metadata_copyright},\n\
        trademark: {metadata_trademark},\n\
  }},\n\
  localization: perro_app::entry::StaticEmbeddedLocalizationConfig {{\n\
        default_locale: {localization_default_locale},\n\
  }},\n\
  steam: perro_app::entry::StaticEmbeddedSteamConfig {{\n\
        enabled: {steam_enabled},\n\
        app_id: {steam_app_id},\n\
        input_mode: {steam_input_mode},\n\
  }},\n\
  assets: perro_app::entry::StaticEmbeddedAssetsConfig {{\n\
        perro_assets: PERRO_ASSETS,\n\
        scene_lookup: static_assets::scenes::lookup_scene,\n\
        localization_lookup: static_assets::localizations::lookup_localized_string,\n\
        material_lookup: static_assets::materials::lookup_material,\n\
        ui_style_lookup: static_assets::ui_styles::lookup_ui_style,\n\
        tileset_lookup: static_assets::tilesets::lookup_tileset,\n\
        particle_lookup: static_assets::particles::lookup_particle,\n\
        animation_lookup: static_assets::animations::lookup_animation,\n\
        animation_tree_lookup: static_assets::animation_trees::lookup_animation_tree,\n\
        csv_lookup: static_assets::csvs::lookup_csv,\n\
        mesh_lookup: static_assets::meshes::lookup_mesh,\n\
        collision_trimesh_lookup: static_assets::collision_trimeshes::lookup_collision_trimesh,\n\
        navmesh_lookup: static_assets::navmeshes::lookup_navmesh,\n\
        skeleton_lookup: static_assets::skeletons::lookup_skeleton,\n\
        texture_lookup: static_assets::textures::lookup_texture,\n\
        shader_lookup: static_assets::shaders::lookup_shader,\n\
        audio_lookup: static_assets::audios::lookup_audio,\n\
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),\n\
  }},\n\
}})\n\
.expect(\"failed to run embedded static project\");",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        routes_block = emit_static_routes_block(&routes),
        input_map_block = emit_static_input_map_block(&cfg.input_map),
        vsync = cfg.vsync,
        msaa = cfg.msaa,
        ssao = emit_ssao_expr(cfg.ssao),
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        ui_pixel_snapping = cfg.rendering.ui.pixel_snapping,
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        frame_rate_cap = emit_frame_rate_cap_expr(cfg.frame_rate_cap),
        physics_gravity = emit_f32(cfg.physics_gravity),
        physics_coef = emit_f32(cfg.physics_coef),
        metadata_description = emit_optional_static_str(cfg.metadata.description.as_deref()),
        metadata_company = emit_optional_static_str(cfg.metadata.company.as_deref()),
        metadata_version = emit_optional_static_str(cfg.metadata.version.as_deref()),
        metadata_copyright = emit_optional_static_str(cfg.metadata.copyright.as_deref()),
        metadata_trademark = emit_optional_static_str(cfg.metadata.trademark.as_deref()),
        localization_default_locale = emit_static_str(
            cfg.localization
                .as_ref()
                .map(|loc| loc.default_locale.as_str())
                .unwrap_or("en"),
        ),
        steam_enabled = cfg.steam.enabled,
        steam_app_id = emit_optional_steam_app_id_fn(cfg.steam.app_id),
        steam_input_mode = emit_steam_input_mode(cfg.steam.input_mode),
    );
    if options.headless {
        embedded_block = embedded_block.replace("perro_app::entry", "perro_headless");
    }
    let embedded_block = indent_block(&embedded_block, 2);
    let embedded_web_block = format!(
        "let root = project_root();\n\
perro_app::entry::run_static_embedded_project_web(perro_app::entry::StaticEmbeddedProject {{\n\
  project: perro_app::entry::StaticEmbeddedProjectInfo {{\n\
        project_root: &root,\n\
        project_name: \"{name}\",\n\
        main_scene_hash: {main_scene_hash}u64,\n\
        icon_hash: {icon_hash}u64,\n\
        startup_splash_hash: {startup_splash_hash}u64,\n\
        virtual_width: {w},\n\
        virtual_height: {h},\n\
  }},\n\
  routes: perro_app::entry::StaticEmbeddedRoutesConfig {{\n\
        routes: {routes_block},\n\
  }},\n\
  input: perro_app::entry::StaticEmbeddedInputMapConfig {{\n\
        actions: {input_map_block},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        msaa: {msaa},\n\
        ssao: {ssao},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
        ui_pixel_snapping: {ui_pixel_snapping},\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        frame_rate_cap: {frame_rate_cap},\n\
        physics_gravity: {physics_gravity},\n\
        physics_coef: {physics_coef},\n\
  }},\n\
  metadata: perro_app::entry::StaticEmbeddedMetadataConfig {{\n\
        description: {metadata_description},\n\
        company: {metadata_company},\n\
        version: {metadata_version},\n\
        copyright: {metadata_copyright},\n\
        trademark: {metadata_trademark},\n\
  }},\n\
  localization: perro_app::entry::StaticEmbeddedLocalizationConfig {{\n\
        default_locale: {localization_default_locale},\n\
  }},\n\
  steam: perro_app::entry::StaticEmbeddedSteamConfig {{\n\
        enabled: {steam_enabled},\n\
        app_id: {steam_app_id},\n\
        input_mode: {steam_input_mode},\n\
  }},\n\
  assets: perro_app::entry::StaticEmbeddedAssetsConfig {{\n\
        perro_assets: PERRO_ASSETS,\n\
        scene_lookup: static_assets::scenes::lookup_scene,\n\
        localization_lookup: static_assets::localizations::lookup_localized_string,\n\
        material_lookup: static_assets::materials::lookup_material,\n\
        ui_style_lookup: static_assets::ui_styles::lookup_ui_style,\n\
        tileset_lookup: static_assets::tilesets::lookup_tileset,\n\
        particle_lookup: static_assets::particles::lookup_particle,\n\
        animation_lookup: static_assets::animations::lookup_animation,\n\
        animation_tree_lookup: static_assets::animation_trees::lookup_animation_tree,\n\
        csv_lookup: static_assets::csvs::lookup_csv,\n\
        mesh_lookup: static_assets::meshes::lookup_mesh,\n\
        collision_trimesh_lookup: static_assets::collision_trimeshes::lookup_collision_trimesh,\n\
        navmesh_lookup: static_assets::navmeshes::lookup_navmesh,\n\
        skeleton_lookup: static_assets::skeletons::lookup_skeleton,\n\
        texture_lookup: static_assets::textures::lookup_texture,\n\
        shader_lookup: static_assets::shaders::lookup_shader,\n\
        audio_lookup: static_assets::audios::lookup_audio,\n\
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),\n\
  }},\n\
}})",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        routes_block = emit_static_routes_block(&routes),
        input_map_block = emit_static_input_map_block(&cfg.input_map),
        vsync = cfg.vsync,
        msaa = cfg.msaa,
        ssao = emit_ssao_expr(cfg.ssao),
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        ui_pixel_snapping = cfg.rendering.ui.pixel_snapping,
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        frame_rate_cap = emit_frame_rate_cap_expr(cfg.frame_rate_cap),
        physics_gravity = emit_f32(cfg.physics_gravity),
        physics_coef = emit_f32(cfg.physics_coef),
        metadata_description = emit_optional_static_str(cfg.metadata.description.as_deref()),
        metadata_company = emit_optional_static_str(cfg.metadata.company.as_deref()),
        metadata_version = emit_optional_static_str(cfg.metadata.version.as_deref()),
        metadata_copyright = emit_optional_static_str(cfg.metadata.copyright.as_deref()),
        metadata_trademark = emit_optional_static_str(cfg.metadata.trademark.as_deref()),
        localization_default_locale = emit_static_str(
            cfg.localization
                .as_ref()
                .map(|loc| loc.default_locale.as_str())
                .unwrap_or("en"),
        ),
        steam_enabled = cfg.steam.enabled,
        steam_app_id = emit_optional_steam_app_id_fn(cfg.steam.app_id),
        steam_input_mode = emit_steam_input_mode(cfg.steam.input_mode),
    );
    let embedded_android_block = format!(
        "let root = project_root();\n\
perro_app::entry::run_static_embedded_project_android(app, perro_app::entry::StaticEmbeddedProject {{\n\
  project: perro_app::entry::StaticEmbeddedProjectInfo {{\n\
        project_root: &root,\n\
        project_name: \"{name}\",\n\
        main_scene_hash: {main_scene_hash}u64,\n\
        icon_hash: {icon_hash}u64,\n\
        startup_splash_hash: {startup_splash_hash}u64,\n\
        virtual_width: {w},\n\
        virtual_height: {h},\n\
  }},\n\
  routes: perro_app::entry::StaticEmbeddedRoutesConfig {{\n\
        routes: {routes_block},\n\
  }},\n\
  input: perro_app::entry::StaticEmbeddedInputMapConfig {{\n\
        actions: {input_map_block},\n\
  }},\n\
  graphics: perro_app::entry::StaticEmbeddedGraphicsConfig {{\n\
        vsync: {vsync},\n\
        msaa: {msaa},\n\
        ssao: {ssao},\n\
        meshlets: {meshlets},\n\
        dev_meshlets: {dev_meshlets},\n\
        release_meshlets: {release_meshlets},\n\
        meshlet_debug_view: {meshlet_debug_view},\n\
        occlusion_culling: {occlusion_culling},\n\
        particle_sim_default: {particle_sim_default},\n\
        ui_pixel_snapping: {ui_pixel_snapping},\n\
  }},\n\
  runtime: perro_app::entry::StaticEmbeddedRuntimeConfig {{\n\
        target_fixed_update: {target_fixed_update},\n\
        frame_rate_cap: {frame_rate_cap},\n\
        physics_gravity: {physics_gravity},\n\
        physics_coef: {physics_coef},\n\
  }},\n\
  metadata: perro_app::entry::StaticEmbeddedMetadataConfig {{\n\
        description: {metadata_description},\n\
        company: {metadata_company},\n\
        version: {metadata_version},\n\
        copyright: {metadata_copyright},\n\
        trademark: {metadata_trademark},\n\
  }},\n\
  localization: perro_app::entry::StaticEmbeddedLocalizationConfig {{\n\
        default_locale: {localization_default_locale},\n\
  }},\n\
  steam: perro_app::entry::StaticEmbeddedSteamConfig {{\n\
        enabled: {steam_enabled},\n\
        app_id: {steam_app_id},\n\
        input_mode: {steam_input_mode},\n\
  }},\n\
  assets: perro_app::entry::StaticEmbeddedAssetsConfig {{\n\
        perro_assets: PERRO_ASSETS,\n\
        scene_lookup: static_assets::scenes::lookup_scene,\n\
        localization_lookup: static_assets::localizations::lookup_localized_string,\n\
        material_lookup: static_assets::materials::lookup_material,\n\
        ui_style_lookup: static_assets::ui_styles::lookup_ui_style,\n\
        tileset_lookup: static_assets::tilesets::lookup_tileset,\n\
        particle_lookup: static_assets::particles::lookup_particle,\n\
        animation_lookup: static_assets::animations::lookup_animation,\n\
        animation_tree_lookup: static_assets::animation_trees::lookup_animation_tree,\n\
        csv_lookup: static_assets::csvs::lookup_csv,\n\
        mesh_lookup: static_assets::meshes::lookup_mesh,\n\
        collision_trimesh_lookup: static_assets::collision_trimeshes::lookup_collision_trimesh,\n\
        navmesh_lookup: static_assets::navmeshes::lookup_navmesh,\n\
        skeleton_lookup: static_assets::skeletons::lookup_skeleton,\n\
        texture_lookup: static_assets::textures::lookup_texture,\n\
        shader_lookup: static_assets::shaders::lookup_shader,\n\
        audio_lookup: static_assets::audios::lookup_audio,\n\
        static_script_registry: Some(scripts::SCRIPT_REGISTRY),\n\
  }},\n\
}})\n\
.expect(\"failed to run embedded static project on android\");",
        name = escape_str(&cfg.name),
        main_scene_hash = perro_ids::string_to_u64(&cfg.main_scene),
        icon_hash = perro_ids::string_to_u64(&cfg.icon),
        startup_splash_hash = perro_ids::string_to_u64(&cfg.startup_splash),
        w = cfg.virtual_width,
        h = cfg.virtual_height,
        routes_block = emit_static_routes_block(&routes),
        input_map_block = emit_static_input_map_block(&cfg.input_map),
        vsync = cfg.vsync,
        msaa = cfg.msaa,
        ssao = emit_ssao_expr(cfg.ssao),
        meshlets = cfg.meshlets,
        dev_meshlets = cfg.dev_meshlets,
        release_meshlets = cfg.release_meshlets,
        meshlet_debug_view = cfg.meshlet_debug_view,
        occlusion_culling = emit_occlusion_culling_expr(cfg.occlusion_culling),
        particle_sim_default = emit_particle_sim_default_expr(cfg.particle_sim_default),
        ui_pixel_snapping = cfg.rendering.ui.pixel_snapping,
        target_fixed_update = emit_optional_f32(cfg.target_fixed_update),
        frame_rate_cap = emit_frame_rate_cap_expr(cfg.frame_rate_cap),
        physics_gravity = emit_f32(cfg.physics_gravity),
        physics_coef = emit_f32(cfg.physics_coef),
        metadata_description = emit_optional_static_str(cfg.metadata.description.as_deref()),
        metadata_company = emit_optional_static_str(cfg.metadata.company.as_deref()),
        metadata_version = emit_optional_static_str(cfg.metadata.version.as_deref()),
        metadata_copyright = emit_optional_static_str(cfg.metadata.copyright.as_deref()),
        metadata_trademark = emit_optional_static_str(cfg.metadata.trademark.as_deref()),
        localization_default_locale = emit_static_str(
            cfg.localization
                .as_ref()
                .map(|loc| loc.default_locale.as_str())
                .unwrap_or("en"),
        ),
        steam_enabled = cfg.steam.enabled,
        steam_app_id = emit_optional_steam_app_id_fn(cfg.steam.app_id),
        steam_input_mode = emit_steam_input_mode(cfg.steam.input_mode),
    );
    let embedded_android_block = indent_block(&embedded_android_block, 2);
    let embedded_web_block = indent_block(&embedded_web_block, 4);
    let steam_app_id_fn_block = emit_static_steam_app_id_fn(cfg.steam.app_id, &cfg.name);

    let shared_src = format!(
        "#![allow(dead_code)]\n\n\
#[path = \"static/mod.rs\"]\n\
mod static_assets;\n\n\
static PERRO_ASSETS: &[u8] = include_bytes!(\"../embedded/assets.perro\");\n\n\
{steam_app_id_fn_block}\
#[used]\n\
#[unsafe(no_mangle)]\n\
pub static PERRO_ENGINE_DETECT: [u8; 89] =\n\
    *b\"PERRO_ENGINE_DETECT:v1;engine=Perro Engine;format=.perro;site=https://www.perroengine.com\";\n\n\
pub fn keep_perro_engine_marker() {{\n\
    // SAFETY: Reads stay within static marker bounds and use valid static pointers.\n\
    unsafe {{\n\
        std::hint::black_box(std::ptr::read_volatile(PERRO_ENGINE_DETECT.as_ptr()));\n\
        std::hint::black_box(std::ptr::read_volatile(\n\
            PERRO_ENGINE_DETECT.as_ptr().add(PERRO_ENGINE_DETECT.len() - 1),\n\
        ));\n\
    }}\n\
}}\n\n\
#[cfg(any(target_os = \"android\", target_arch = \"wasm32\"))]\n\
pub fn project_root() -> std::path::PathBuf {{\n\
    std::path::PathBuf::from(\".\")\n\
}}\n\n\
#[cfg(all(not(target_os = \"android\"), not(target_arch = \"wasm32\")))]\n\
pub fn project_root() -> std::path::PathBuf {{\n\
    if let Ok(exe) = std::env::current_exe() {{\n\
        if let Some(exe_dir) = exe.parent() {{\n\
            for dir in exe_dir.ancestors() {{\n\
                if dir.join(\"project.toml\").exists() {{\n\
                    return dir.to_path_buf();\n\
                }}\n\
            }}\n\
            return exe_dir.to_path_buf();\n\
        }}\n\
    }}\n\
    let root = std::path::PathBuf::from(env!(\"CARGO_MANIFEST_DIR\")).join(\"..\").join(\"..\");\n\
    if root.join(\"project.toml\").exists() {{\n\
        return root.canonicalize().unwrap_or(root);\n\
    }}\n\
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(\".\"))\n\
}}\n\n\
#[cfg(all(not(target_os = \"android\"), not(target_arch = \"wasm32\")))]\n\
pub fn run_native() {{\n\
{embedded_block}\n\
}}\n\n\
#[cfg(target_os = \"android\")]\n\
pub fn run_android(app: perro_app::entry::AndroidApp) {{\n\
{embedded_android_block}\n\
}}\n\n\
#[cfg(target_arch = \"wasm32\")]\n\
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {{\n\
    console_error_panic_hook::set_once();\n\
{embedded_web_block}\n\
}}\n",
        embedded_block = embedded_block,
        embedded_android_block = embedded_android_block,
        embedded_web_block = embedded_web_block,
        steam_app_id_fn_block = steam_app_id_fn_block,
    );
    let lib_src = "#![cfg_attr(all(perro_no_console, target_os = \"windows\"), windows_subsystem = \"windows\")]\n\n#[path = \"entry_shared.rs\"]\nmod entry_shared;\n\npub use entry_shared::*;\n\n#[cfg(target_os = \"android\")]\n#[unsafe(no_mangle)]\npub fn android_main(app: perro_app::entry::AndroidApp) {\n    keep_perro_engine_marker();\n    run_android(app);\n}\n\n#[cfg(target_arch = \"wasm32\")]\n#[wasm_bindgen::prelude::wasm_bindgen(start)]\npub fn run_web_entry() -> Result<(), wasm_bindgen::JsValue> {\n    keep_perro_engine_marker();\n    run_web()\n}\n";
    let main_src = "#![cfg_attr(all(perro_no_console, target_os = \"windows\"), windows_subsystem = \"windows\")]\n\n#[path = \"entry_shared.rs\"]\nmod entry_shared;\n\n#[cfg(all(not(target_os = \"android\"), not(target_arch = \"wasm32\")))]\nfn main() {\n  entry_shared::keep_perro_engine_marker();\n  entry_shared::run_native();\n}\n";
    write_string_if_changed(&project_src.join("entry_shared.rs"), &shared_src)?;
    write_string_if_changed(&project_src.join("lib.rs"), lib_src)?;
    write_string_if_changed(&project_src.join("main.rs"), main_src)?;
    Ok(())
}

fn export_project_web_bundle(
    project_root: &Path,
    target_dir: &Path,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
    let package_name = read_project_package_name(project_root)?;
    let library_name = read_project_library_name(project_root, &package_name)?;
    let project_cfg = load_project_toml(project_root)
        .map_err(|err| CompilerError::SceneParse(format!("failed to load project.toml: {err}")))?;
    let routes = perro_project::load_routes_toml(project_root, &project_cfg)
        .map_err(|err| CompilerError::SceneParse(format!("failed to load routes.toml: {err}")))?;
    let profile_dir = if options.release { "release" } else { "debug" };
    let built_wasm = target_dir
        .join("wasm32-unknown-unknown")
        .join(profile_dir)
        .join(format!("{library_name}.wasm"));
    if !built_wasm.exists() {
        return Err(CompilerError::SceneParse(format!(
            "project wasm not found after build: {}",
            built_wasm.display()
        )));
    }

    let output_dir = match options.web_output_dir {
        WebOutputDir::Build => project_root.join(".output").join("web"),
        WebOutputDir::Dev => project_root.join(".output").join("web-dev"),
    };
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)?;
    }
    fs::create_dir_all(&output_dir)?;

    wasm_bindgen_cli_support::Bindgen::new()
        .web(true)
        .and_then(|bindgen| {
            bindgen
                .input_path(&built_wasm)
                .out_name("app")
                .typescript(false)
                .generate(&output_dir)
        })
        .map_err(|err| CompilerError::SceneParse(format!("wasm-bindgen failed: {err:#}")))?;

    write_string_if_changed(&output_dir.join("boot.js"), web_boot_js())?;
    emit_web_route_html_files(project_root, &output_dir, &project_cfg, &routes)?;
    println!("exported web bundle: {}", output_dir.display());
    Ok(())
}

fn sync_android_project_manifest(
    project_root: &Path,
    cfg: &perro_project::ProjectConfig,
    options: ProjectBuildOptions,
) -> Result<(), CompilerError> {
    if options.target != ProjectBuildTarget::Android {
        return Ok(());
    }

    let manifest_path = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let src = fs::read_to_string(&manifest_path)?;
    let mut value = toml::from_str::<toml::Value>(&src).map_err(|err| {
        CompilerError::SceneParse(format!(
            "failed to parse generated project manifest {}: {err}",
            manifest_path.display()
        ))
    })?;
    let root = value.as_table_mut().ok_or_else(|| {
        CompilerError::SceneParse(format!(
            "generated project manifest is not a TOML table: {}",
            manifest_path.display()
        ))
    })?;

    {
        let package = root
            .entry("package")
            .or_insert_with(|| toml::Value::Table(Default::default()));
        let package_table = package.as_table_mut().ok_or_else(|| {
            CompilerError::SceneParse("generated project package table invalid".to_string())
        })?;
        if let Some(version) = cfg.metadata.version.as_ref() {
            package_table.insert("version".to_string(), toml::Value::String(version.clone()));
        }

        let metadata = package_table
            .entry("metadata")
            .or_insert_with(|| toml::Value::Table(Default::default()));
        let metadata_table = metadata.as_table_mut().ok_or_else(|| {
            CompilerError::SceneParse("generated project package.metadata invalid".to_string())
        })?;
        let android = metadata_table
            .entry("android")
            .or_insert_with(|| toml::Value::Table(Default::default()));
        let android_table = android.as_table_mut().ok_or_else(|| {
            CompilerError::SceneParse(
                "generated project package.metadata.android invalid".to_string(),
            )
        })?;
        android_table.insert(
            "package".to_string(),
            toml::Value::String(android_package_name(project_root, cfg)),
        );
        android_table.insert(
            "build_targets".to_string(),
            toml::Value::Array(vec![toml::Value::String(
                "aarch64-linux-android".to_string(),
            )]),
        );
        android_table.insert("label".to_string(), toml::Value::String(cfg.name.clone()));
        android_table.insert("min_sdk_version".to_string(), toml::Value::Integer(26));
        android_table.insert("target_sdk_version".to_string(), toml::Value::Integer(35));
    }

    let lib = root
        .entry("lib")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let lib_table = lib.as_table_mut().ok_or_else(|| {
        CompilerError::SceneParse("generated project lib table invalid".to_string())
    })?;
    lib_table.insert("name".to_string(), toml::Value::String("main".to_string()));

    let rendered = toml::to_string(&value).map_err(|err| {
        CompilerError::SceneParse(format!(
            "failed to render generated project manifest {}: {err}",
            manifest_path.display()
        ))
    })?;
    write_string_if_changed(&manifest_path, &rendered)?;
    Ok(())
}

fn android_package_name(project_root: &Path, cfg: &perro_project::ProjectConfig) -> String {
    let fallback = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("perro_project");
    let source = sanitize_android_ident(if cfg.name.trim().is_empty() {
        fallback
    } else {
        &cfg.name
    });
    format!("com.perro.{source}")
}

fn sanitize_android_ident(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "perro_project".to_string()
    } else if trimmed.as_bytes()[0].is_ascii_digit() {
        format!("perro_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn web_boot_js() -> &'static str {
    "import init from './app.js';\n\
\n\
const boot = document.getElementById('boot');\n\
const staticPage = document.getElementById('perro-static-page');\n\
const shellCache = new Map();\n\
const parser = new DOMParser();\n\
const setBoot = (text, kind = 'info') => {\n\
  if (!boot) return;\n\
  boot.textContent = text;\n\
  boot.dataset.kind = kind;\n\
};\n\
\n\
const appReady = () => document.body.dataset.perroApp === 'ready';\n\
\n\
const splitHref = (href) => {\n\
  const url = new URL(href, window.location.href);\n\
  let path = url.pathname || '/';\n\
  if (path.length > '/index.html'.length && path.endsWith('/index.html')) {\n\
    path = path.slice(0, -'/index.html'.length);\n\
  }\n\
  while (path.length > 1 && path.endsWith('/')) {\n\
    path = path.slice(0, -1);\n\
  }\n\
  if (!path.startsWith('/')) {\n\
    path = `/${path}`;\n\
  }\n\
  return {\n\
    path,\n\
    historyHref: `${path}${url.search}${url.hash}`,\n\
    documentHref: path === '/' ? '/index.html' : `${path}/index.html`,\n\
  };\n\
};\n\
\n\
const syncHead = (doc) => {\n\
  if (doc.title) {\n\
    document.title = doc.title;\n\
  }\n\
  for (const name of ['description', 'keywords']) {\n\
    const next = doc.head.querySelector(`meta[name=\"${name}\"]`);\n\
    const current = document.head.querySelector(`meta[name=\"${name}\"]`);\n\
    if (next && current) {\n\
      current.setAttribute('content', next.getAttribute('content') || '');\n\
    } else if (next && !current) {\n\
      document.head.appendChild(next.cloneNode(true));\n\
    } else if (!next && current) {\n\
      current.remove();\n\
    }\n\
  }\n\
  const nextIcon = doc.head.querySelector('link[rel=\"icon\"]');\n\
  const currentIcon = document.head.querySelector('link[rel=\"icon\"]');\n\
  if (nextIcon && currentIcon) {\n\
    currentIcon.setAttribute('href', nextIcon.getAttribute('href') || '');\n\
  }\n\
};\n\
\n\
const fetchShellDoc = async (href) => {\n\
  const parts = splitHref(href);\n\
  let pending = shellCache.get(parts.path);\n\
  if (!pending) {\n\
    pending = fetch(parts.documentHref, { credentials: 'same-origin' }).then((resp) => {\n\
      if (!resp.ok) {\n\
        throw new Error(`route fetch fail: ${resp.status}`);\n\
      }\n\
      return resp.text();\n\
    });\n\
    shellCache.set(parts.path, pending);\n\
  }\n\
  const text = await pending;\n\
  return { parts, doc: parser.parseFromString(text, 'text/html') };\n\
};\n\
\n\
const applyShellDoc = (doc) => {\n\
  if (!staticPage) return;\n\
  const nextStatic = doc.getElementById('perro-static-page');\n\
  if (!nextStatic) return;\n\
  staticPage.innerHTML = nextStatic.innerHTML;\n\
  syncHead(doc);\n\
};\n\
\n\
const navShell = async (href, pushHistory) => {\n\
  if (appReady()) return;\n\
  const { parts, doc } = await fetchShellDoc(href);\n\
  applyShellDoc(doc);\n\
  if (pushHistory) {\n\
    window.history.pushState(null, '', parts.historyHref);\n\
  }\n\
};\n\
\n\
const hideBoot = () => {\n\
  if (!boot) return;\n\
  boot.dataset.state = 'done';\n\
  document.body.dataset.perroApp = 'ready';\n\
  window.setTimeout(() => boot.remove(), 400);\n\
};\n\
\n\
const obs = new MutationObserver(() => {\n\
  if (document.querySelector('canvas')) {\n\
    hideBoot();\n\
    obs.disconnect();\n\
  }\n\
});\n\
obs.observe(document.body, { childList: true, subtree: true });\n\
\n\
document.addEventListener('click', (event) => {\n\
  if (appReady()) return;\n\
  if (event.defaultPrevented || event.button !== 0) return;\n\
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;\n\
  const anchor = event.target instanceof Element\n\
    ? event.target.closest('#perro-static-page a[href]')\n\
    : null;\n\
  if (!(anchor instanceof HTMLAnchorElement)) return;\n\
  if (anchor.target && anchor.target !== '_self') return;\n\
  const url = new URL(anchor.href, window.location.href);\n\
  if (url.origin !== window.location.origin) return;\n\
  event.preventDefault();\n\
  setBoot('loading route...');\n\
  navShell(url.href, true).catch((err) => {\n\
    console.error('perro route shell fail', err);\n\
    window.location.href = url.href;\n\
  });\n\
});\n\
\n\
const prefetchShell = (target) => {\n\
  if (appReady()) return;\n\
  const anchor = target instanceof Element\n\
    ? target.closest('#perro-static-page a[href]')\n\
    : null;\n\
  if (!(anchor instanceof HTMLAnchorElement)) return;\n\
  const url = new URL(anchor.href, window.location.href);\n\
  if (url.origin !== window.location.origin) return;\n\
  fetchShellDoc(url.href).catch(() => {});\n\
};\n\
\n\
document.addEventListener('pointerover', (event) => prefetchShell(event.target), { passive: true });\n\
document.addEventListener('focusin', (event) => prefetchShell(event.target));\n\
window.addEventListener('popstate', () => {\n\
  if (appReady()) return;\n\
  setBoot('loading route...');\n\
  navShell(window.location.href, false).catch((err) => {\n\
    console.error('perro route shell fail', err);\n\
    window.location.reload();\n\
  });\n\
});\n\
\n\
setBoot('loading wasm...');\n\
\n\
try {\n\
  await init();\n\
  setBoot('starting render...');\n\
  if (document.querySelector('canvas')) {\n\
    hideBoot();\n\
    obs.disconnect();\n\
  }\n\
} catch (err) {\n\
  console.error('perro web boot fail', err);\n\
  const msg = err instanceof Error ? err.message : String(err);\n\
  document.body.dataset.perroApp = 'boot-fail';\n\
  setBoot(`boot fail: ${msg}`, 'error');\n\
  obs.disconnect();\n\
}\n"
}

struct StaticWebPage {
    title: String,
    description: Option<String>,
    keywords: Vec<String>,
    icon_href: String,
    boot_href: String,
    app_href: String,
    wasm_href: String,
    body_html: String,
    boot_label: String,
}

fn emit_web_route_html_files(
    project_root: &Path,
    output_dir: &Path,
    project_cfg: &perro_project::ProjectConfig,
    routes: &perro_project::ProjectRoutesConfig,
) -> Result<(), CompilerError> {
    let icon_output = copy_res_asset_into_web_output(project_root, output_dir, &project_cfg.icon)?;
    for route in &routes.routes {
        let html_path = web_route_html_path(output_dir, &route.href)?;
        ensure_web_write_path(output_dir, &html_path, "route output")?;
        if let Some(parent) = html_path.parent() {
            fs::create_dir_all(parent)?;
        }
        ensure_web_write_path(output_dir, &html_path, "route output")?;
        let body_html =
            render_route_scene_html(project_root, output_dir, &html_path, &route.scene)?;
        let title = route
            .title
            .clone()
            .or_else(|| {
                (route.href == "/")
                    .then(|| project_cfg.web.title.clone())
                    .flatten()
            })
            .unwrap_or_else(|| {
                if route.href == "/" {
                    project_cfg
                        .web
                        .title
                        .clone()
                        .unwrap_or_else(|| project_cfg.name.clone())
                } else {
                    let site_title = project_cfg
                        .web
                        .title
                        .clone()
                        .unwrap_or_else(|| project_cfg.name.clone());
                    format!("{} | {site_title}", route.name)
                }
            });
        let description = route
            .description
            .clone()
            .or_else(|| project_cfg.web.description.clone());
        let keywords = merge_web_keywords(&project_cfg.web.keywords, &route.keywords);
        let page = StaticWebPage {
            title: title.clone(),
            description,
            keywords,
            icon_href: relative_output_href(&html_path, &icon_output),
            boot_href: relative_output_href(&html_path, &output_dir.join("boot.js")),
            app_href: relative_output_href(&html_path, &output_dir.join("app.js")),
            wasm_href: relative_output_href(&html_path, &output_dir.join("app_bg.wasm")),
            body_html,
            boot_label: format!("{title} boot"),
        };
        write_string_if_changed(&html_path, &web_index_html(&page))?;
    }
    Ok(())
}

fn web_route_html_path(output_dir: &Path, href: &str) -> Result<PathBuf, CompilerError> {
    let href = perro_project::normalize_route_href(href);
    if href == "/" {
        return Ok(output_dir.join("index.html"));
    }
    let relative = checked_portable_relative_path(href.trim_start_matches('/'), "route href")?;
    Ok(output_dir.join(relative).join("index.html"))
}

fn copy_res_asset_into_web_output(
    project_root: &Path,
    output_dir: &Path,
    res_path: &str,
) -> Result<PathBuf, CompilerError> {
    let rel = checked_res_relative_path(res_path, "web asset")?;
    let res_root = project_root.join("res");
    let source = res_root.join(&rel);
    if !source.exists() {
        return Err(CompilerError::SceneParse(format!(
            "web asset not found: {}",
            source.display()
        )));
    }
    ensure_existing_path_within(&res_root, &source, "web asset source")?;
    let target = output_dir.join("assets").join(rel);
    ensure_web_write_path(output_dir, &target, "web asset output")?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    ensure_web_write_path(output_dir, &target, "web asset output")?;
    fs::copy(&source, &target)?;
    Ok(target)
}

fn render_route_scene_html(
    project_root: &Path,
    output_dir: &Path,
    html_path: &Path,
    scene_path: &str,
) -> Result<String, CompilerError> {
    let rel = checked_res_relative_path(scene_path, "web scene")?;
    let res_root = project_root.join("res");
    let scene_file = res_root.join(rel);
    ensure_existing_path_within(&res_root, &scene_file, "web scene source")?;
    let scene_src = fs::read_to_string(&scene_file)?;
    let scene = std::panic::catch_unwind(|| perro_scene::Parser::new(&scene_src).parse_scene())
        .map_err(|_| {
            CompilerError::SceneParse(format!(
                "failed to parse scene for static web html: {}",
                scene_file.display()
            ))
        })?;
    let mut html = String::new();
    if let Some(root) = scene.root {
        html.push_str(&render_scene_entry_html(
            project_root,
            output_dir,
            html_path,
            &scene,
            root,
        )?);
    } else {
        for entry in scene.nodes.iter().filter(|entry| entry.parent.is_none()) {
            html.push_str(&render_scene_entry_html(
                project_root,
                output_dir,
                html_path,
                &scene,
                entry.key,
            )?);
        }
    }
    if html.trim().is_empty() {
        html.push_str("<main class=\"perro-static-page__content\"></main>");
    }
    Ok(html)
}

fn render_scene_entry_html(
    project_root: &Path,
    output_dir: &Path,
    html_path: &Path,
    scene: &perro_scene::Scene,
    key: perro_scene::SceneKey,
) -> Result<String, CompilerError> {
    let Some(entry) = scene.nodes.get(key.as_usize()) else {
        return Ok(String::new());
    };
    if scene_field_bool(&entry.data, "visible") == Some(false) {
        return Ok(String::new());
    }
    let children_html =
        render_scene_children_html(project_root, output_dir, html_path, scene, entry)?;
    let name_attr = entry
        .name
        .as_deref()
        .map(escape_html_attr)
        .map(|value| format!(" data-perro-name=\"{value}\""))
        .unwrap_or_default();
    let node_attr = format!(
        " class=\"perro-node perro-node--{}\" data-perro-node=\"{}\"{}",
        escape_html_attr(entry.data.type_name()),
        escape_html_attr(entry.data.type_name()),
        name_attr
    );
    match entry.data.type_name() {
        "UiLabel" => {
            let text = scene_field_str(&entry.data, "text")
                .map(decode_scene_text_literal)
                .map(normalize_static_html_text)
                .unwrap_or_default();
            Ok(format!("<p{node_attr}>{}</p>", escape_html(&text)))
        }
        "UiTextBox" | "UiTextBlock" => {
            let text = scene_field_str(&entry.data, "text")
                .map(decode_scene_text_literal)
                .or_else(|| {
                    scene_field_str(&entry.data, "placeholder").map(decode_scene_text_literal)
                })
                .map(normalize_static_html_text)
                .unwrap_or_default();
            Ok(format!("<p{node_attr}>{}</p>", escape_html(&text)))
        }
        "UiButton" => {
            let inner = if children_html.trim().is_empty() {
                let fallback = entry
                    .name
                    .as_deref()
                    .map(str::to_string)
                    .unwrap_or_else(|| "link".to_string());
                escape_html(&fallback)
            } else {
                children_html
            };
            if let Some(href) = extract_button_href(&entry.data) {
                Ok(format!(
                    "<a href=\"{}\"{node_attr}>{inner}</a>",
                    escape_html_attr(&href)
                ))
            } else {
                Ok(format!(
                    "<button type=\"button\"{node_attr}>{inner}</button>"
                ))
            }
        }
        "UiImage" | "UiImageButton" | "UiNineSliceButton" | "UiNineSlice" | "UiAnimatedImage" | "NineSlice2D" | "NineSliceButton2D" => {
            if let Some(texture) = extract_ui_image_source(&entry.data) {
                let copied = copy_res_asset_into_web_output(project_root, output_dir, &texture)?;
                let src = relative_output_href(html_path, &copied);
                let alt = entry.name.as_deref().unwrap_or("");
                Ok(format!(
                    "<img src=\"{}\" alt=\"{}\"{node_attr}>",
                    escape_html_attr(&src),
                    escape_html_attr(alt)
                ))
            } else {
                Ok(children_html)
            }
        }
        ty if is_static_web_container(ty) => {
            let tag = static_web_container_tag(entry);
            Ok(format!("<{tag}{node_attr}>{children_html}</{tag}>"))
        }
        _ => Ok(children_html),
    }
}

fn render_scene_children_html(
    project_root: &Path,
    output_dir: &Path,
    html_path: &Path,
    scene: &perro_scene::Scene,
    entry: &perro_scene::SceneNodeEntry,
) -> Result<String, CompilerError> {
    let mut out = String::new();
    let child_keys: Vec<_> = if entry.children.is_empty() {
        scene
            .nodes
            .iter()
            .filter(|candidate| candidate.parent == Some(entry.key))
            .map(|candidate| candidate.key)
            .collect()
    } else {
        entry.children.iter().copied().collect()
    };
    for child in child_keys {
        out.push_str(&render_scene_entry_html(
            project_root,
            output_dir,
            html_path,
            scene,
            child,
        )?);
    }
    Ok(out)
}

fn is_static_web_container(ty: &str) -> bool {
    matches!(
        ty,
        "UiNode"
            | "UiPanel"
            | "UiLayout"
            | "UiHLayout"
            | "UiHBox"
            | "UiVLayout"
            | "UiVBox"
            | "UiGrid"
            | "UiScrollContainer"
            | "UiScroll"
            | "UiTreeList"
    )
}

fn static_web_container_tag(entry: &perro_scene::SceneNodeEntry) -> &'static str {
    let name = entry.name.as_deref().unwrap_or("").to_ascii_lowercase();
    if name.contains("nav") {
        "nav"
    } else if name.contains("header") {
        "header"
    } else if name.contains("footer") {
        "footer"
    } else if name.contains("section") || name.contains("hero") {
        "section"
    } else {
        "div"
    }
}

fn scene_field_bool(data: &perro_scene::SceneNodeData, field: &str) -> Option<bool> {
    scene_field_value(data, field)?.as_bool()
}

fn scene_field_str<'a>(data: &'a perro_scene::SceneNodeData, field: &str) -> Option<&'a str> {
    scene_field_value(data, field)?.as_str()
}

fn scene_field_value<'a>(
    data: &'a perro_scene::SceneNodeData,
    field: &str,
) -> Option<&'a perro_scene::SceneValue> {
    let mut found = data
        .base_ref()
        .and_then(|base| scene_field_value(base, field));
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            found = Some(value);
        }
    }
    found
}

fn extract_button_href(data: &perro_scene::SceneNodeData) -> Option<String> {
    let perro_scene::SceneValue::Object(fields) = scene_field_value(data, "web")? else {
        return None;
    };
    fields.iter().find_map(|(name, value)| {
        (name.as_ref() == "href")
            .then(|| value.as_str().map(perro_project::normalize_route_href))
            .flatten()
    })
}

fn extract_ui_image_source(data: &perro_scene::SceneNodeData) -> Option<String> {
    for field in ["texture", "image", "source", "src"] {
        if let Some(value) = scene_field_str(data, field)
            && value.starts_with("res://")
        {
            return Some(value.to_string());
        }
    }
    None
}

fn decode_scene_text_literal(text: &str) -> String {
    if let Some(stripped) = text.strip_prefix("%%loc:") {
        return decode_text_escapes(&format!("%loc:{stripped}"));
    }
    if let Some(raw) = text.strip_prefix("%loc:") {
        let raw = raw.trim().trim_matches('"').trim();
        return raw.to_string();
    }
    decode_text_escapes(text)
}

fn normalize_static_html_text(text: String) -> String {
    text.replace("\\n", " ")
        .replace("\\r", " ")
        .replace("\\t", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_text_escapes(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn merge_web_keywords(global: &[String], route: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for keyword in global.iter().chain(route.iter()) {
        let trimmed = keyword.trim();
        if trimmed.is_empty() || out.iter().any(|existing: &String| existing == trimmed) {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

fn relative_output_href(from_html: &Path, to: &Path) -> String {
    relative_include_path(from_html, to).replace('\\', "/")
}

fn checked_res_relative_path(res_path: &str, label: &str) -> Result<PathBuf, CompilerError> {
    let relative = res_path.trim().strip_prefix("res://").ok_or_else(|| {
        CompilerError::SceneParse(format!(
            "expected res:// path for {label}, got `{res_path}`"
        ))
    })?;
    checked_portable_relative_path(relative, label)
}

fn checked_portable_relative_path(value: &str, label: &str) -> Result<PathBuf, CompilerError> {
    if value.is_empty()
        || value.contains(['\\', ':'])
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(CompilerError::SceneParse(format!(
            "{label} must use normal relative path components: `{value}`"
        )));
    }
    Ok(value.split('/').collect())
}

fn ensure_existing_path_within(root: &Path, path: &Path, label: &str) -> Result<(), CompilerError> {
    let root = fs::canonicalize(root).map_err(CompilerError::Io)?;
    let path = fs::canonicalize(path).map_err(CompilerError::Io)?;
    if path.starts_with(&root) {
        return Ok(());
    }
    Err(CompilerError::SceneParse(format!(
        "{label} escapes root: {}",
        path.display()
    )))
}

fn ensure_web_write_path(
    output_dir: &Path,
    target: &Path,
    label: &str,
) -> Result<(), CompilerError> {
    let mut check_path = if target.exists() {
        target
    } else {
        target.parent().unwrap_or(output_dir)
    };
    while !check_path.exists() {
        let Some(parent) = check_path.parent() else {
            break;
        };
        check_path = parent;
    }
    ensure_existing_path_within(output_dir, check_path, label)
}

fn web_index_html(page: &StaticWebPage) -> String {
    let description = page
        .description
        .as_deref()
        .map(|value| {
            format!(
                "<meta name=\"description\" content=\"{}\">\n",
                escape_html_attr(value)
            )
        })
        .unwrap_or_default();
    let keywords = if page.keywords.is_empty() {
        String::new()
    } else {
        format!(
            "<meta name=\"keywords\" content=\"{}\">\n",
            escape_html_attr(&page.keywords.join(", "))
        )
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<link rel=\"icon\" href=\"{icon}\">\n<link rel=\"modulepreload\" href=\"{app_href}\">\n<link rel=\"preload\" href=\"{wasm_href}\" as=\"fetch\" type=\"application/wasm\" crossorigin>\n{description}{keywords}<style>\n:root{{color-scheme:dark}}html,body{{margin:0;min-height:100%;background:radial-gradient(circle at top,#182233 0%,#0b0d12 55%,#07090d 100%);color:#dce7f9;font-family:Inter,Segoe UI,system-ui,sans-serif}}body{{display:flex;flex-direction:column}}#perro-static-page{{width:min(1240px,calc(100vw - 32px));margin:0 auto;padding:18px 0 36px}}#perro-static-page *{{box-sizing:border-box}}#perro-static-page a{{color:inherit}}#perro-static-page img{{max-width:100%;height:auto;display:block}}#perro-static-page nav,#perro-static-page header,#perro-static-page section,#perro-static-page footer,#perro-static-page div{{width:100%}}#perro-static-page .perro-node--UiHLayout{{display:flex;gap:18px;align-items:stretch;flex-wrap:wrap}}#perro-static-page .perro-node--UiVLayout,#perro-static-page .perro-node--UiScrollContainer{{display:grid;gap:18px}}#perro-static-page .perro-node--UiPanel,#perro-static-page .perro-node--UiScrollContainer{{padding:18px;border:1px solid #334158;border-radius:18px;background:rgba(16,21,30,.92);box-shadow:0 18px 60px rgba(0,0,0,.28)}}#perro-static-page nav{{display:flex;gap:14px;align-items:center;flex-wrap:wrap;padding:16px 18px;border:1px solid #334158;border-radius:18px;background:rgba(16,21,30,.92);box-shadow:0 18px 60px rgba(0,0,0,.28)}}#perro-static-page nav .perro-node--UiButton{{background:#1a2230;color:#dce7f9;border-color:#4a5f81;box-shadow:none}}#perro-static-page nav .perro-node--UiButton:first-child{{background:transparent;border-color:transparent;color:#fff4d4;padding-inline:10px}}#perro-static-page footer{{padding:18px;border:1px solid #334158;border-radius:18px;background:rgba(16,21,30,.92)}}#perro-static-page .perro-node--UiLabel{{margin:0;line-height:1.45;color:inherit}}#perro-static-page a.perro-node--UiButton,#perro-static-page button.perro-node--UiButton{{display:inline-flex;align-items:center;justify-content:center;gap:8px;min-height:46px;padding:12px 18px;border:1px solid #f7d891;border-radius:14px;background:#e4b85b;color:#201406;font-weight:700;text-decoration:none;transition:transform .18s ease,background .18s ease,border-color .18s ease}}#perro-static-page a.perro-node--UiButton:hover,#perro-static-page button.perro-node--UiButton:hover{{transform:translateY(-1px);background:#f0c96d}}#perro-static-page p[data-perro-name*='title'],#perro-static-page p[data-perro-name*='hero']{{font-size:clamp(1.8rem,4vw,3.4rem);line-height:1.05;color:#fff7e0;font-weight:800;letter-spacing:-.04em}}#perro-static-page p[data-perro-name*='text'],#perro-static-page p[data-perro-name*='copy']{{color:#c8d4e8}}body[data-perro-app='ready'] #perro-static-page{{display:none}}canvas{{display:block;width:100vw;height:100vh;outline:none}}#boot{{position:fixed;left:12px;top:12px;max-width:min(480px,calc(100vw - 24px));padding:8px 10px;background:rgba(0,0,0,.78);border:1px solid rgba(255,255,255,.12);border-radius:8px;font-size:13px;line-height:1.4;z-index:10;transition:opacity .2s ease;opacity:0;pointer-events:none}}#boot[data-kind='error']{{opacity:1;pointer-events:auto;color:#ffb4b4;border-color:rgba(255,120,120,.35)}}#boot[data-state='done']{{opacity:0;pointer-events:none}}@media (max-width: 760px){{#perro-static-page{{width:calc(100vw - 24px);padding:12px 0 28px}}#perro-static-page .perro-node--UiHLayout{{gap:12px}}#perro-static-page nav{{gap:10px;padding:14px}}#perro-static-page .perro-node--UiPanel,#perro-static-page .perro-node--UiScrollContainer,#perro-static-page footer{{padding:14px}}#perro-static-page a.perro-node--UiButton,#perro-static-page button.perro-node--UiButton{{width:100%}}}}\n</style>\n</head>\n<body>\n<main id=\"perro-static-page\">{body}</main>\n<div id=\"boot\">{boot}</div>\n<script type=\"module\" src=\"{boot_href}\"></script>\n</body>\n</html>\n",
        title = escape_html(&page.title),
        icon = escape_html_attr(&page.icon_href),
        app_href = escape_html_attr(&page.app_href),
        wasm_href = escape_html_attr(&page.wasm_href),
        description = description,
        keywords = keywords,
        body = page.body_html,
        boot = escape_html(&page.boot_label),
        boot_href = escape_html_attr(&page.boot_href),
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(s: &str) -> String {
    escape_html(s).replace('"', "&quot;").replace('\'', "&#39;")
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_generated_include_path(path: &str) -> String {
    let raw = if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else {
        path.to_string()
    };
    raw.replace('\\', "/")
}

fn relative_include_path(generated_file: &Path, source_file: &Path) -> String {
    let from_dir = generated_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let from_abs = from_dir.canonicalize().unwrap_or(from_dir);
    let to_abs = source_file
        .canonicalize()
        .unwrap_or_else(|_| source_file.to_path_buf());

    let from_components: Vec<_> = from_abs.components().collect();
    let to_components: Vec<_> = to_abs.components().collect();

    let mut common = 0usize;
    let max_common = from_components.len().min(to_components.len());
    while common < max_common && from_components[common] == to_components[common] {
        common += 1;
    }

    if common == 0 {
        return normalize_generated_include_path(&to_abs.to_string_lossy());
    }

    let mut rel = PathBuf::new();
    for _ in common..from_components.len() {
        rel.push("..");
    }
    for comp in &to_components[common..] {
        rel.push(comp.as_os_str());
    }

    normalize_generated_include_path(&rel.to_string_lossy())
}

fn emit_occlusion_culling_expr(mode: perro_project::OcclusionCulling) -> &'static str {
    match mode {
        perro_project::OcclusionCulling::Cpu => "perro_app::entry::OcclusionCulling::Cpu",
        perro_project::OcclusionCulling::Gpu => "perro_app::entry::OcclusionCulling::Gpu",
        perro_project::OcclusionCulling::Off => "perro_app::entry::OcclusionCulling::Off",
    }
}

fn emit_ssao_expr(quality: perro_project::SsaoQuality) -> &'static str {
    match quality {
        perro_project::SsaoQuality::Off => "perro_runtime::SsaoQuality::Off",
        perro_project::SsaoQuality::Low => "perro_runtime::SsaoQuality::Low",
        perro_project::SsaoQuality::Medium => "perro_runtime::SsaoQuality::Medium",
        perro_project::SsaoQuality::High => "perro_runtime::SsaoQuality::High",
        perro_project::SsaoQuality::Ultra => "perro_runtime::SsaoQuality::Ultra",
    }
}

fn emit_particle_sim_default_expr(mode: perro_project::ParticleSimDefault) -> &'static str {
    match mode {
        perro_project::ParticleSimDefault::Cpu => "perro_app::entry::ParticleSimDefault::Cpu",
        perro_project::ParticleSimDefault::GpuVertex => {
            "perro_app::entry::ParticleSimDefault::GpuVertex"
        }
        perro_project::ParticleSimDefault::GpuCompute => {
            "perro_app::entry::ParticleSimDefault::GpuCompute"
        }
    }
}

fn emit_frame_rate_cap_expr(cap: perro_project::FrameRateCap) -> String {
    match cap {
        perro_project::FrameRateCap::Unlimited => {
            "perro_app::entry::FrameRateCap::Unlimited".to_string()
        }
        perro_project::FrameRateCap::Fps(fps) if fps.is_finite() && fps > 0.0 => {
            format!("perro_app::entry::FrameRateCap::Fps({}f32)", fps)
        }
        perro_project::FrameRateCap::Fps(_) => {
            "perro_app::entry::FrameRateCap::Unlimited".to_string()
        }
        perro_project::FrameRateCap::RefreshRate => {
            "perro_app::entry::FrameRateCap::RefreshRate".to_string()
        }
    }
}

fn emit_optional_f32(value: Option<f32>) -> String {
    match value {
        Some(v) if v.is_finite() => format!("Some({}f32)", v),
        _ => "None".to_string(),
    }
}

fn emit_optional_steam_app_id_fn(value: Option<u32>) -> String {
    match value {
        Some(_) => "Some(steam_app_id)".to_string(),
        None => "None".to_string(),
    }
}

fn emit_steam_input_mode(mode: perro_project::SteamInputMode) -> &'static str {
    match mode {
        perro_project::SteamInputMode::Off => "perro_runtime::SteamInputMode::Off",
        perro_project::SteamInputMode::Metadata => "perro_runtime::SteamInputMode::Metadata",
        perro_project::SteamInputMode::Actions => "perro_runtime::SteamInputMode::Actions",
    }
}

fn emit_static_steam_app_id_fn(value: Option<u32>, project_name: &str) -> String {
    let Some(app_id) = value else {
        return String::new();
    };

    let mut seed = 0x9e37_79b9_7f4a_7c15u64 ^ u64::from(app_id);
    for byte in project_name.as_bytes() {
        seed = splitmix64(seed ^ u64::from(*byte));
    }

    let data_key = next_nonzero_u32(&mut seed);
    let data_mask = next_nonzero_u32(&mut seed);
    let add = next_nonzero_u32(&mut seed);
    let split = next_nonzero_u32(&mut seed);
    let noise = next_nonzero_u32(&mut seed);
    let check_key = next_nonzero_u32(&mut seed) | 1;
    let rot_a = (next_nonzero_u32(&mut seed) % 31) + 1;
    let rot_b = (next_nonzero_u32(&mut seed) % 31) + 1;
    let encoded = app_id
        .rotate_left(rot_a)
        .wrapping_add(add)
        .rotate_left(rot_b)
        ^ data_key
        ^ data_mask;
    let data_a = encoded ^ split;
    let data_b = split;
    let check = app_id.wrapping_mul(check_key).rotate_left(rot_b) ^ noise;
    let poison = next_nonzero_u32(&mut seed);

    format!(
        "fn steam_app_id() -> u32 {{\n\
    const DATA_A: u32 = 0x{data_a:08x};\n\
    const DATA_B: u32 = 0x{data_b:08x};\n\
    const DATA_KEY: u32 = 0x{data_key:08x};\n\
    const DATA_MASK: u32 = 0x{data_mask:08x};\n\
    const ADD: u32 = 0x{add:08x};\n\
    const CHECK_KEY: u32 = 0x{check_key:08x};\n\
    const CHECK: u32 = 0x{check:08x};\n\
    const NOISE: u32 = 0x{noise:08x};\n\
    const POISON: u32 = 0x{poison:08x};\n\
    let mut x = std::hint::black_box(DATA_A) ^ std::hint::black_box(DATA_B);\n\
    x = std::hint::black_box(x ^ std::hint::black_box(DATA_KEY));\n\
    x = std::hint::black_box(x ^ std::hint::black_box(DATA_MASK));\n\
    x = std::hint::black_box(x.rotate_right({rot_b}));\n\
    x = std::hint::black_box(x.wrapping_sub(std::hint::black_box(ADD)));\n\
    let id = std::hint::black_box(x.rotate_right({rot_a}));\n\
    let check_key = std::hint::black_box(CHECK_KEY);\n\
    let noise = std::hint::black_box(NOISE);\n\
    let check = std::hint::black_box(id.wrapping_mul(check_key).rotate_left({rot_b}) ^ noise);\n\
    if check == CHECK {{\n\
        id\n\
    }} else {{\n\
        id ^ POISON\n\
    }}\n\
}}\n\n"
    )
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn next_nonzero_u32(seed: &mut u64) -> u32 {
    loop {
        *seed = splitmix64(*seed);
        let value = (*seed >> 16) as u32;
        if value != 0 {
            return value;
        }
    }
}

fn emit_f32(value: f32) -> String {
    if value.is_finite() {
        format!("{value}f32")
    } else {
        "0.0f32".to_string()
    }
}

fn emit_optional_static_str(value: Option<&str>) -> String {
    match value {
        Some(v) => format!("Some({})", emit_static_str(v)),
        None => "None".to_string(),
    }
}

fn emit_static_str(value: &str) -> String {
    format!("\"{}\"", escape_str(value))
}

fn emit_static_routes_block(routes: &perro_project::ProjectRoutesConfig) -> String {
    let mut out = String::from("&[");
    for route in &routes.routes {
        out.push_str("\n            perro_app::entry::StaticEmbeddedRoute { ");
        out.push_str(&format!(
            "href: {}, name: {}, scene_hash: {}u64 }},",
            emit_static_str(&route.href),
            emit_static_str(&route.name),
            perro_ids::parse_hashed_source_uri(&route.scene)
                .unwrap_or_else(|| perro_ids::string_to_u64(&route.scene))
        ));
    }
    if !routes.routes.is_empty() {
        out.push_str("\n        ");
    }
    out.push(']');
    out
}

fn emit_static_input_map_block(input_map: &perro_input_api::InputMap) -> String {
    let mut out = String::from("&[");
    for action in input_map.actions() {
        let mut keys = Vec::new();
        let mut mouse = Vec::new();
        let mut gamepad = Vec::new();
        let mut joycon = Vec::new();
        for binding in &action.bindings {
            match binding {
                perro_input_api::InputBinding::Key(key) => {
                    keys.push(format!("perro_input_api::KeyCode::{key:?}"));
                }
                perro_input_api::InputBinding::Mouse(button) => {
                    mouse.push(format!("perro_input_api::MouseButton::{button:?}"));
                }
                perro_input_api::InputBinding::Gamepad(button) => {
                    gamepad.push(format!("perro_input_api::GamepadButton::{button:?}"));
                }
                perro_input_api::InputBinding::JoyCon(button) => {
                    joycon.push(format!("perro_input_api::JoyConButton::{button:?}"));
                }
            }
        }
        out.push_str("\n            perro_app::entry::StaticEmbeddedInputAction { ");
        out.push_str(&format!(
            "name: {}, keys: &{}, mouse: &{}, gamepad: &{}, joycon: &{} }},",
            emit_static_str(&action.name),
            emit_static_input_binding_array(&keys),
            emit_static_input_binding_array(&mouse),
            emit_static_input_binding_array(&gamepad),
            emit_static_input_binding_array(&joycon)
        ));
    }
    if !input_map.actions().is_empty() {
        out.push_str("\n        ");
    }
    out.push(']');
    out
}

fn emit_static_input_binding_array(items: &[String]) -> String {
    if items.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", items.join(", "))
    }
}

fn indent_block(src: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    src.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
