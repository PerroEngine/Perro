#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptsBuildProfile {
    Debug,
    Release,
}

fn validate_dlc_name(dlc_name: &str) -> Result<(), CompilerError> {
    perro_io::validate_dlc_name(dlc_name)
        .map_err(|err| CompilerError::SceneParse(format!("invalid dlc name `{dlc_name}`: {err}")))
}

pub fn sync_scripts(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    let res_dir = project_root.join("res");
    let scripts_src = project_root.join(".perro").join("scripts").join("src");
    sync_scripts_from_source(&res_dir, &scripts_src, "res://")
}

pub fn sync_dlc_scripts(project_root: &Path, dlc_name: &str) -> Result<Vec<String>, CompilerError> {
    validate_dlc_name(dlc_name)?;
    let dlc_root = project_root.join("dlcs").join(dlc_name);
    let scripts_src = project_root
        .join(".perro")
        .join("dlc")
        .join(dlc_name)
        .join("scripts")
        .join("src");
    let prefix = format!("dlc://{dlc_name}/");
    sync_scripts_from_source(&dlc_root, &scripts_src, &prefix)
}

fn sync_scripts_from_source(
    source_dir: &Path,
    scripts_src: &Path,
    script_path_prefix: &str,
) -> Result<Vec<String>, CompilerError> {
    fs::create_dir_all(scripts_src)?;

    let mut copied = Vec::<String>::new();
    let mut registrable = Vec::<String>::new();
    let mut generated_rel_paths = HashSet::<String>::new();
    if source_dir.exists() {
        walk_dir(source_dir, &mut |path| {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                return Ok(());
            }
            let rel = path.strip_prefix(source_dir).map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "script path {} escaped source root {}: {err}",
                        path.display(),
                        source_dir.display()
                    ),
                )
            })?;
            let rel_norm = rel.to_string_lossy().replace('\\', "/");
            let generated_rel = generated_script_rel(&rel_norm);
            let dst = scripts_src.join(&generated_rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            let source = fs::read_to_string(path)?;
            let source_include = relative_include_path(&dst, path);
            let transformed = transpile_frontend_script(&source, &source_include);
            if transpiled_exports_script_ctor(&transformed) {
                registrable.push(rel_norm.clone());
            }
            write_string_if_changed(&dst, &transformed)?;
            generated_rel_paths.insert(generated_rel);
            copied.push(rel_norm);
            Ok(())
        })?;
    }

    copied.sort();
    registrable.sort();
    let _ = remove_stale_generated_scripts(scripts_src, &generated_rel_paths)?;
    let _ = write_scripts_lib(scripts_src, &copied, &registrable, script_path_prefix)?;
    Ok(copied)
}

pub fn compile_scripts(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    compile_scripts_with_profile(project_root, ScriptsBuildProfile::Release)
}

pub fn compile_scripts_with_profile(
    project_root: &Path,
    profile: ScriptsBuildProfile,
) -> Result<Vec<String>, CompilerError> {
    ensure_source_overrides(project_root)?;
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    let copied = sync_scripts(project_root)?;
    let scripts_crate = project_root.join(".perro").join("scripts");
    let target_dir = project_root.join("target");

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(scripts_crate);
    if profile == ScriptsBuildProfile::Release {
        cmd.arg("--release");
        apply_fast_release_dylib_profile(&mut cmd);
    }
    add_dynamic_scripts_feature(&mut cmd);
    add_steamworks_feature(&mut cmd, cfg.steam.enabled);
    run_cargo_command_with_normalized_paths(&mut cmd, project_root)?;
    compile_all_dlc_scripts_with_profile(project_root, profile, cfg.steam.enabled)?;

    Ok(copied)
}

fn compile_all_dlc_scripts_with_profile(
    project_root: &Path,
    profile: ScriptsBuildProfile,
    steam_enabled: bool,
) -> Result<(), CompilerError> {
    let dlcs_root = project_root.join("dlcs");
    if !dlcs_root.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(&dlcs_root)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dlc_name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        validate_dlc_name(dlc_name)?;
        let crate_slug = sanitize_crate_slug(dlc_name);
        let crate_name = format!("scripts_{crate_slug}");
        let scripts_crate = project_root
            .join(".perro")
            .join("dlc")
            .join(dlc_name)
            .join("scripts");
        let scripts_src = scripts_crate.join("src");
        fs::create_dir_all(&scripts_src)?;
        write_dlc_scripts_manifest(project_root, &crate_name, &scripts_crate)?;
        write_string_if_changed(&scripts_src.join("lib.rs"), &default_scripts_lib_rs())?;
        let _ = sync_dlc_scripts(project_root, dlc_name)?;
        compile_scripts_crate(project_root, &scripts_crate, profile, steam_enabled)?;
        let dylib = resolve_compiled_dylib(
            project_root,
            &dylib_name_for_crate(&crate_name),
            &dylib_prefix_for_crate(&crate_name),
        )?;
        fs::copy(dylib, scripts_crate.join(scripts_dylib_name()))?;
    }
    Ok(())
}

fn compile_scripts_crate(
    project_root: &Path,
    scripts_crate: &Path,
    profile: ScriptsBuildProfile,
    steam_enabled: bool,
) -> Result<(), CompilerError> {
    let target_dir = project_root.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(scripts_crate);
    if profile == ScriptsBuildProfile::Release {
        cmd.arg("--release");
        apply_fast_release_dylib_profile(&mut cmd);
    }
    add_dynamic_scripts_feature(&mut cmd);
    add_steamworks_feature(&mut cmd, steam_enabled);
    run_cargo_command_with_normalized_paths(&mut cmd, project_root)?;
    Ok(())
}

fn compile_dlc_package_crate(
    project_root: &Path,
    scripts_crate: &Path,
    dynamic_scripts: bool,
) -> Result<(), CompilerError> {
    let target_dir = project_root.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(scripts_crate);
    apply_dlc_release_dylib_profile(&mut cmd);
    let cfg = load_project_toml(project_root)
        .map_err(|e| CompilerError::SceneParse(format!("failed to load project.toml: {e}")))?;
    if dynamic_scripts {
        add_dynamic_scripts_feature(&mut cmd);
    }
    add_steamworks_feature(&mut cmd, cfg.steam.enabled);
    run_cargo_command_with_normalized_paths(&mut cmd, project_root)?;
    Ok(())
}

fn run_cargo_command_with_normalized_paths(
    cmd: &mut Command,
    project_root: &Path,
) -> Result<(), CompilerError> {
    let crate_dir = cmd.get_current_dir().map(Path::to_path_buf);
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_root = project_root.to_path_buf();
    let stdout_crate_dir = crate_dir.clone();
    let stdout_thread = thread::spawn(move || -> Result<(), CompilerError> {
        if let Some(stream) = stdout {
            stream_normalized_cargo_output(
                io::stdout(),
                &stdout_root,
                stdout_crate_dir.as_deref(),
                stream,
            )?;
        }
        Ok(())
    });

    let stderr_root = project_root.to_path_buf();
    let stderr_crate_dir = crate_dir;
    let stderr_thread = thread::spawn(move || -> Result<(), CompilerError> {
        if let Some(stream) = stderr {
            stream_normalized_cargo_output(
                io::stderr(),
                &stderr_root,
                stderr_crate_dir.as_deref(),
                stream,
            )?;
        }
        Ok(())
    });

    let status = child.wait()?;
    stdout_thread
        .join()
        .map_err(|_| CompilerError::SceneParse("cargo stdout thread panic".to_string()))??;
    stderr_thread
        .join()
        .map_err(|_| CompilerError::SceneParse("cargo stderr thread panic".to_string()))??;

    if !status.success() {
        return Err(CompilerError::CargoFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

fn stream_normalized_cargo_output<W: Write, R: Read>(
    mut writer: W,
    project_root: &Path,
    crate_dir: Option<&Path>,
    reader: R,
) -> Result<(), CompilerError> {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        let normalized = normalize_cargo_output_paths(project_root, crate_dir, &line);
        writer.write_all(normalized.as_bytes())?;
        writer.flush()?;
    }
    Ok(())
}

fn normalize_cargo_output_paths(
    project_root: &Path,
    crate_dir: Option<&Path>,
    text: &str,
) -> String {
    let slash_text = text.replace('\\', "/");
    let mut out = String::with_capacity(slash_text.len());
    let mut cursor = 0usize;
    while let Some(rel) = slash_text[cursor..].find(".rs") {
        let rs_end = cursor + rel + ".rs".len();
        let start = find_path_start(&slash_text, cursor + rel);
        let end = find_path_end(&slash_text, rs_end);
        if start < cursor {
            out.push_str(&slash_text[cursor..rs_end]);
            cursor = rs_end;
            continue;
        }
        out.push_str(&slash_text[cursor..start]);
        let segment = &slash_text[start..end];
        out.push_str(&normalize_cargo_path_segment(
            project_root,
            crate_dir,
            segment,
        ));
        cursor = end;
    }
    out.push_str(&slash_text[cursor..]);
    out
}

fn find_path_start(text: &str, before_rs: usize) -> usize {
    let bytes = text.as_bytes();
    let mut start = before_rs;
    while start > 0 && is_path_byte(bytes[start - 1]) {
        start -= 1;
    }
    start
}

fn find_path_end(text: &str, after_rs: usize) -> usize {
    let bytes = text.as_bytes();
    let mut end = after_rs;
    while end < bytes.len() && is_path_suffix_byte(bytes[end]) {
        end += 1;
    }
    end
}

fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b':' | b'@')
}

fn is_path_suffix_byte(byte: u8) -> bool {
    is_path_byte(byte)
}

fn normalize_cargo_path_segment(
    project_root: &Path,
    crate_dir: Option<&Path>,
    segment: &str,
) -> String {
    let Some((path, suffix)) = split_rust_path_suffix(segment) else {
        return segment.to_string();
    };
    let path_buf = PathBuf::from(path);
    let joined = if path_buf.is_absolute() {
        path_buf
    } else if let Some(crate_dir) = crate_dir {
        crate_dir.join(path)
    } else {
        path_buf
    };
    let cleaned = clean_path(&joined);
    let display_path = project_relative_display_path(project_root, &cleaned);
    format!("{display_path}{suffix}")
}

fn split_rust_path_suffix(segment: &str) -> Option<(&str, &str)> {
    let idx = segment.find(".rs")? + ".rs".len();
    Some(segment.split_at(idx))
}

fn project_relative_display_path(project_root: &Path, path: &Path) -> String {
    let project_root = clean_path(project_root);
    let rel = path.strip_prefix(&project_root).unwrap_or(path);
    normalize_generated_include_path(&rel.to_string_lossy())
}

fn clean_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            std::path::Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            std::path::Component::RootDir => out.push(comp.as_os_str()),
            std::path::Component::Normal(part) => out.push(part),
        }
    }
    out
}

fn apply_fast_release_dylib_profile(cmd: &mut Command) {
    cmd.env("CARGO_PROFILE_RELEASE_INCREMENTAL", "true")
        .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "64");
}

fn apply_dlc_release_dylib_profile(cmd: &mut Command) {
    cmd.env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "3")
        .env("CARGO_PROFILE_RELEASE_LTO", "fat")
        .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1")
        .env("CARGO_PROFILE_RELEASE_PANIC", "abort")
        .env("CARGO_PROFILE_RELEASE_INCREMENTAL", "false");
}

fn add_steamworks_feature(cmd: &mut Command, steam_enabled: bool) {
    if steam_enabled {
        cmd.arg("--features").arg("steamworks");
    }
}

fn add_dynamic_scripts_feature(cmd: &mut Command) {
    cmd.arg("--features").arg("dynamic-scripts");
}

fn write_dlc_scripts_manifest(
    project_root: &Path,
    crate_name: &str,
    scripts_crate: &Path,
) -> Result<(), CompilerError> {
    fs::create_dir_all(scripts_crate.join("src"))?;
    let engine_root = engine_root_dir();
    let perro_api_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("api_modules")
            .join("perro_api"),
    );
    let perro_runtime_path = normalize_toml_path(
        &engine_root
            .join("perro_source")
            .join("runtime_project")
            .join("perro_runtime"),
    );
    let mut manifest = format!(
        "[workspace]\n\n[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nperro_api = {{ path = \"{perro_api_path}\" }}\nperro_runtime = {{ path = \"{perro_runtime_path}\" }}\n\n[features]\ndynamic-scripts = []\nsteamworks = [\"perro_api/steamworks\", \"perro_runtime/steamworks\"]\n"
    );
    let extra_deps = read_extra_script_deps(project_root)?;
    if !extra_deps.is_empty() {
        for line in extra_deps {
            manifest.push_str(&line);
            manifest.push('\n');
        }
    }
    manifest.push_str(&build_patch_crates_io_block(&engine_root));
    let manifest_path = scripts_crate.join("Cargo.toml");
    write_string_if_changed(&manifest_path, &manifest)?;
    Ok(())
}

fn engine_root_dir() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    raw.canonicalize().unwrap_or(raw)
}

fn normalize_toml_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw.strip_prefix("\\\\?\\").unwrap_or(raw.as_ref());
    stripped.replace('\\', "/")
}

fn build_patch_crates_io_block(engine_root: &Path) -> String {
    const CRATES: &[(&str, &str)] = &[
        ("perro_animation", "perro_source/core/perro_animation"),
        ("perro_nodes", "perro_source/core/perro_nodes"),
        ("perro_ui", "perro_source/core/perro_ui"),
        ("perro_structs", "perro_source/core/perro_structs"),
        ("perro_ids", "perro_source/core/perro_ids"),
        ("perro_variant", "perro_source/core/perro_variant"),
        (
            "perro_particle_math",
            "perro_source/core/perro_particle_math",
        ),
        (
            "perro_runtime",
            "perro_source/runtime_project/perro_runtime",
        ),
        (
            "perro_internal_updates",
            "perro_source/runtime_project/perro_internal_updates",
        ),
        ("perro_scene", "perro_source/runtime_project/perro_scene"),
        (
            "perro_runtime_api",
            "perro_source/api_modules/perro_runtime_api",
        ),
        (
            "perro_resource_api",
            "perro_source/api_modules/perro_resource_api",
        ),
        ("perro_api", "perro_source/api_modules/perro_api"),
        ("perro_modules", "perro_source/api_modules/perro_modules"),
        (
            "perro_networking",
            "perro_source/api_modules/perro_networking",
        ),
        (
            "perro_input_api",
            "perro_source/api_modules/perro_input_api",
        ),
        ("perro_jobs", "perro_source/api_modules/perro_jobs"),
        ("perro_web", "perro_source/api_modules/perro_web"),
        (
            "perro_render_bridge",
            "perro_source/render_stack/perro_render_bridge",
        ),
        ("perro_graphics", "perro_source/render_stack/perro_graphics"),
        ("perro_meshlets", "perro_source/render_stack/perro_meshlets"),
        ("perro_app", "perro_source/render_stack/perro_app"),
        (
            "perro_scripting",
            "perro_source/script_stack/perro_scripting",
        ),
        (
            "perro_scripting_macros",
            "perro_source/script_stack/perro_scripting_macros",
        ),
        (
            "perro_compiler",
            "perro_source/build_pipeline/perro_compiler",
        ),
        (
            "perro_static_pipeline",
            "perro_source/build_pipeline/perro_static_pipeline",
        ),
        ("perro_io", "perro_source/io_stack/perro_io"),
        ("perro_assets", "perro_source/io_stack/perro_assets"),
        ("perro_pawdio", "perro_source/audio_stack/perro_pawdio"),
        (
            "perro_project",
            "perro_source/runtime_project/perro_project",
        ),
        ("perro_cli", "perro_source/devtools/perro_cli"),
        ("perro_dev_runner", "perro_source/devtools/perro_dev_runner"),
    ];
    let mut out = String::new();
    out.push_str("\n[patch.crates-io]\n");
    for (name, rel) in CRATES {
        let full = normalize_toml_path(&engine_root.join(rel));
        out.push_str(&format!("{name} = {{ path = \"{full}\" }}\n"));
    }
    out
}

fn read_extra_script_deps(project_root: &Path) -> Result<Vec<String>, CompilerError> {
    let deps_toml = project_root.join("deps.toml");
    if !deps_toml.exists() {
        return Ok(Vec::new());
    }
    let src = fs::read_to_string(&deps_toml)?;
    let parsed = src.parse::<toml::Value>().map_err(|err| {
        CompilerError::SceneParse(format!("failed to parse {}: {err}", deps_toml.display()))
    })?;
    let Some(table) = parsed.get("dependencies").and_then(toml::Value::as_table) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (name, spec) in table {
        if name == "perro_api" || name == "perro_runtime" {
            continue;
        }
        if let Some(v) = spec.as_str() {
            out.push(format!("{name} = \"{v}\""));
        } else {
            out.push(format!("{name} = {}", spec));
        }
    }
    Ok(out)
}

fn sanitize_crate_slug(input: &str) -> String {
    let mut out = String::new();
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "dlc".to_string()
    } else {
        trimmed.to_string()
    }
}
