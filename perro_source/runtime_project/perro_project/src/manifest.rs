pub fn ensure_source_overrides(project_root: &Path) -> std::io::Result<()> {
    let project_manifest = project_root
        .join(".perro")
        .join("project")
        .join("Cargo.toml");
    let project_build_script = project_root.join(".perro").join("project").join("build.rs");
    let project_cargo_config = project_root
        .join(".perro")
        .join("project")
        .join(".cargo")
        .join("config.toml");
    let scripts_manifest = project_root
        .join(".perro")
        .join("scripts")
        .join("Cargo.toml");
    let dev_runner_manifest = project_root
        .join(".perro")
        .join("dev_runner")
        .join("Cargo.toml");
    let dev_runner_build_script = project_root
        .join(".perro")
        .join("dev_runner")
        .join("build.rs");
    let dev_runner_main = project_root
        .join(".perro")
        .join("dev_runner")
        .join("src")
        .join("main.rs");
    let perro_dir = project_root.join(".perro");
    let workspace_manifest = perro_dir.join("Cargo.toml");
    let workspace_cargo_config = perro_dir.join(".cargo").join("config.toml");
    let scripts_lib = project_root
        .join(".perro")
        .join("scripts")
        .join("src")
        .join("lib.rs");
    ensure_project_build_script(&project_build_script)?;
    ensure_project_target_dir_config(&project_cargo_config)?;
    ensure_scripts_crate_sync(&scripts_manifest)?;
    ensure_scripts_lib(&scripts_lib)?;
    ensure_project_manifest_deps(&project_manifest)?;
    ensure_project_manifest_icon_build_support(&project_manifest)?;
    ensure_project_manifest_features(&project_manifest)?;
    ensure_project_manifest_web_support(&project_manifest)?;
    ensure_project_manifest_android_support(&project_manifest)?;
    ensure_scripts_manifest_deps(&scripts_manifest)?;
    ensure_scripts_manifest_features(&scripts_manifest)?;
    ensure_scripts_manifest_user_deps(project_root, &scripts_manifest)?;
    ensure_dev_runner_source_sync(&dev_runner_manifest, &dev_runner_main)?;
    ensure_dev_runner_build_script(&dev_runner_build_script)?;
    ensure_dev_runner_manifest_deps(&dev_runner_manifest)?;
    ensure_project_manifest_icon_build_support(&dev_runner_manifest)?;
    ensure_dev_runner_manifest_features(&dev_runner_manifest)?;
    ensure_scripts_manifest_rust_analyzer_cfg(&scripts_manifest)?;
    // `dev_runner` + `scripts` share one workspace so a single cargo resolve links
    // both against the same engine units. Members must not carry `[workspace]`,
    // `[profile.*]` or `[patch.crates-io]` of their own -- cargo rejects or silently
    // ignores those in a member, and a stale one is what used to fork the build.
    migrate_member_manifest_to_workspace(&scripts_manifest)?;
    migrate_member_manifest_to_workspace(&dev_runner_manifest)?;
    ensure_workspace_manifest(&workspace_manifest, &[&dev_runner_manifest, &scripts_manifest])?;
    ensure_workspace_target_dir_config(&workspace_cargo_config)?;
    remove_stale_member_build_files(&perro_dir)?;
    // `project` is excluded from the workspace (it owns the ship `[profile.release]`),
    // so it still needs its own patch block.
    ensure_patch_block_in_manifest(&project_manifest)?;
    Ok(())
}

/// Strips the sections a workspace member is not allowed to own.
fn migrate_member_manifest_to_workspace(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let src = fs::read_to_string(path)?;
    let stripped = strip_toml_sections(&src, &["workspace", "profile", "patch"]);
    let out = format!("{}\n", stripped.trim_end());
    if src == out {
        return Ok(());
    }
    write_if_changed(path, &out)
}

fn ensure_workspace_manifest(
    path: &Path,
    member_manifests: &[&Path],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let base = if path.exists() {
        let src = fs::read_to_string(path)?;
        // Keep any hand-edits outside the patch block; regenerate the patch block.
        strip_patch_crates_io(&src)
    } else {
        default_perro_workspace_toml()
    };
    let overrides = source_overrides_block_for_workspace(path, member_manifests);
    let mut out = base.trim_end().to_string();
    if !overrides.is_empty() {
        out.push_str("\n\n");
        out.push_str(&overrides);
    }
    out.push('\n');
    write_if_changed(path, &out)
}

fn ensure_workspace_target_dir_config(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_perro_workspace_cargo_config_toml())
}

/// Removes per-member build files that the workspace root now owns. A leftover
/// `scripts/.cargo/config.toml` points at the old `../../target`, and a leftover
/// member `Cargo.lock` shadows the workspace lock.
fn remove_stale_member_build_files(perro_dir: &Path) -> std::io::Result<()> {
    for member in ["scripts", "dev_runner"] {
        let member_dir = perro_dir.join(member);
        let cargo_config = member_dir.join(".cargo").join("config.toml");
        if cargo_config.exists() {
            fs::remove_file(&cargo_config)?;
            let cargo_dir = member_dir.join(".cargo");
            // Only prune the directory when nothing else lives there.
            let empty = matches!(cargo_dir.read_dir().map(|mut d| d.next().is_none()), Ok(true));
            if empty {
                fs::remove_dir(&cargo_dir)?;
            }
        }
        let lock = member_dir.join("Cargo.lock");
        if lock.exists() {
            fs::remove_file(&lock)?;
        }
    }
    Ok(())
}

fn ensure_scripts_crate_sync(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(path, &default_scripts_crate_toml())
}

fn ensure_scripts_lib(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_missing(path.to_path_buf(), &default_scripts_lib_rs())
}

fn ensure_dev_runner_source_sync(manifest_path: &Path, main_rs_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = main_rs_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_missing(
        manifest_path.to_path_buf(),
        &default_dev_runner_crate_toml(),
    )?;
    write_if_changed(main_rs_path, &default_dev_runner_main_rs())?;
    Ok(())
}

fn ensure_project_build_script(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(path, &default_project_build_rs())
}

fn ensure_dev_runner_build_script(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(path, &default_project_build_rs())
}

fn ensure_scripts_manifest_user_deps(
    project_root: &Path,
    scripts_manifest: &Path,
) -> std::io::Result<()> {
    if !scripts_manifest.exists() {
        return Ok(());
    }

    let scripts_src = fs::read_to_string(scripts_manifest)?;
    let Ok(mut scripts_value) = parse_toml_document_value(&scripts_src) else {
        return Ok(());
    };
    let Some(scripts_root) = scripts_value.as_table_mut() else {
        return Ok(());
    };
    let scripts_deps = scripts_root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(scripts_deps_table) = scripts_deps.as_table_mut() else {
        return Ok(());
    };

    let mut desired = toml::value::Table::new();
    let deps_toml = project_root.join("deps.toml");
    if deps_toml.exists() {
        let deps_src = fs::read_to_string(&deps_toml)?;
        let deps_value = parse_toml_document_value(&deps_src).map_err(|err| {
            std::io::Error::other(format!("failed to parse {}: {err}", deps_toml.display()))
        })?;
        if let Some(extra_deps) = deps_value.get("dependencies").and_then(Value::as_table) {
            for (name, spec) in extra_deps {
                if !matches!(
                    name.as_str(),
                    "perro_api" | "perro_runtime" | "perro_steamworks"
                ) {
                    desired.insert(name.clone(), spec.clone());
                }
            }
        }
    }
    let before_len = scripts_deps_table.len();
    let mut changed = false;
    scripts_deps_table.retain(|name, _| {
        name == "perro_api" || name == "perro_runtime" || desired.contains_key(name)
    });
    if scripts_deps_table.len() != before_len {
        changed = true;
    }
    for (name, spec) in &desired {
        if scripts_deps_table.get(name) != Some(spec) {
            scripts_deps_table.insert(name.clone(), spec.clone());
            changed = true;
        }
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&scripts_value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(scripts_manifest, &rendered)
}

fn ensure_project_target_dir_config(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_project_cargo_config_toml())
}

fn ensure_project_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let deps = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    let manifest_dir = manifest_dir_for(path);
    let engine_root = engine_root_dir();
    let mut changed = ensure_existing_local_perro_deps(deps_table, &manifest_dir, &engine_root);
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_api");
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_runtime");
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_headless");
    changed |= set_dep_optional(deps_table, "perro_app");
    changed |= set_dep_optional(deps_table, "perro_headless");

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_project_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let features = root
        .entry("features")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(features_table) = features.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    changed |= ensure_feature_values(features_table, "default", &["app"]);
    changed |= ensure_feature_values(features_table, "app", &["dep:perro_app"]);
    changed |= ensure_feature_values(features_table, "headless", &["dep:perro_headless"]);
    changed |= ensure_feature_values(features_table, "perro-demo", &["scripts/perro-demo"]);
    changed |= ensure_feature_values(
        features_table,
        "headless_profile",
        &["perro_headless/profile"],
    );
    changed |= ensure_feature_values(
        features_table,
        "headless_steamworks",
        &[
            "perro_headless/steamworks",
            "perro_api/steamworks",
            "perro_runtime/steamworks",
            "scripts/steamworks",
        ],
    );

    if !features_table.contains_key("profile") {
        features_table.insert(
            "profile".to_string(),
            Value::Array(vec![Value::String("perro_app/profile".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("mem_profile") {
        features_table.insert(
            "mem_profile".to_string(),
            Value::Array(vec![Value::String("perro_app/mem_profile".to_string())]),
        );
        changed = true;
    }
    changed |= ensure_feature_values(
        features_table,
        "steamworks",
        &[
            "perro_app/steamworks",
            "perro_api/steamworks",
            "perro_runtime/steamworks",
            "scripts/steamworks",
        ],
    );

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_project_manifest_icon_build_support(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    let package = root
        .entry("package")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(package_table) = package.as_table_mut() else {
        return Ok(());
    };
    if package_table.get("build").and_then(Value::as_str) != Some("build.rs") {
        package_table.insert("build".to_string(), Value::String("build.rs".to_string()));
        changed = true;
    }

    let target = root
        .entry("target")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(target_table) = target.as_table_mut() else {
        return Ok(());
    };
    let windows_key = "cfg(target_os = \"windows\")".to_string();
    let windows_target = target_table
        .entry(windows_key)
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(windows_target_table) = windows_target.as_table_mut() else {
        return Ok(());
    };
    let build_deps = windows_target_table
        .entry("build-dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(build_deps_table) = build_deps.as_table_mut() else {
        return Ok(());
    };

    if !build_deps_table.contains_key("winresource") {
        build_deps_table.insert(
            "winresource".to_string(),
            Value::String("0.1.20".to_string()),
        );
        changed = true;
    }
    let manifest_dir = manifest_dir_for(path);
    let engine_root = engine_root_dir();
    changed |= ensure_local_perro_dep(build_deps_table, &manifest_dir, &engine_root, "perro_api");
    if build_deps_table.get("toml").and_then(Value::as_str) != Some("0.8.23") {
        build_deps_table.insert("toml".to_string(), Value::String("0.8.23".to_string()));
        changed = true;
    }
    if !build_deps_table.contains_key("image") {
        let mut image = toml::value::Table::new();
        image.insert("version".to_string(), Value::String("0.25.9".to_string()));
        image.insert("default-features".to_string(), Value::Boolean(false));
        image.insert(
            "features".to_string(),
            Value::Array(vec![
                Value::String("png".to_string()),
                Value::String("jpeg".to_string()),
                Value::String("gif".to_string()),
                Value::String("bmp".to_string()),
                Value::String("tga".to_string()),
                Value::String("webp".to_string()),
                Value::String("ico".to_string()),
            ]),
        );
        build_deps_table.insert("image".to_string(), Value::Table(image));
        changed = true;
    }
    if build_deps_table.get("resvg").and_then(Value::as_str) != Some("0.47.0") {
        build_deps_table.insert("resvg".to_string(), Value::String("0.47.0".to_string()));
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_feature_values(
    features_table: &mut toml::map::Map<String, Value>,
    name: &str,
    values: &[&str],
) -> bool {
    let entry = features_table
        .entry(name.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(array) = entry.as_array_mut() else {
        return false;
    };

    let mut changed = false;
    for value in values {
        if !array
            .iter()
            .any(|existing| existing.as_str() == Some(*value))
        {
            array.push(Value::String((*value).to_string()));
            changed = true;
        }
    }
    changed
}

fn ensure_project_manifest_web_support(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let lib = root
        .entry("lib")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(lib_table) = lib.as_table_mut() else {
        return Ok(());
    };
    let crate_type = lib_table
        .entry("crate-type")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(crate_type_arr) = crate_type.as_array_mut() else {
        return Ok(());
    };

    let mut changed = false;
    for name in ["cdylib", "rlib"] {
        if !crate_type_arr.iter().any(|v| v.as_str() == Some(name)) {
            crate_type_arr.push(Value::String(name.to_string()));
            changed = true;
        }
    }

    let target = root
        .entry("target")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(target_table) = target.as_table_mut() else {
        return Ok(());
    };
    let wasm_key = "cfg(target_arch = \"wasm32\")".to_string();
    let wasm = target_table
        .entry(wasm_key)
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(wasm_table) = wasm.as_table_mut() else {
        return Ok(());
    };
    let deps = wasm_table
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    for (name, version) in [
        ("wasm-bindgen", "=0.2.126"),
        ("console_error_panic_hook", "0.1.7"),
        ("getrandom", "0.3.4"),
    ] {
        let missing = !deps_table.contains_key(name);
        let stale_wasm_bindgen =
            name == "wasm-bindgen" && deps_table.get(name).and_then(Value::as_str) != Some(version);
        if missing || stale_wasm_bindgen {
            deps_table.insert(name.to_string(), Value::String(version.to_string()));
            changed = true;
        }
    }
    if deps_table.get("getrandom").and_then(Value::as_str) == Some("0.3.4") {
        let mut spec = toml::value::Table::new();
        spec.insert("version".to_string(), Value::String("0.3.4".to_string()));
        spec.insert(
            "features".to_string(),
            Value::Array(vec![Value::String("wasm_js".to_string())]),
        );
        deps_table.insert("getrandom".to_string(), Value::Table(spec));
        changed = true;
    }
    if !deps_table.contains_key("getrandom_js") {
        let mut spec = toml::value::Table::new();
        spec.insert(
            "package".to_string(),
            Value::String("getrandom".to_string()),
        );
        spec.insert("version".to_string(), Value::String("0.2.17".to_string()));
        spec.insert(
            "features".to_string(),
            Value::Array(vec![Value::String("js".to_string())]),
        );
        deps_table.insert("getrandom_js".to_string(), Value::Table(spec));
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_project_manifest_android_support(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let lib = root
        .entry("lib")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(lib_table) = lib.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;
    if lib_table.get("name").and_then(Value::as_str) != Some("main") {
        lib_table.insert("name".to_string(), Value::String("main".to_string()));
        changed = true;
    }

    let package = root
        .entry("package")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(package_table) = package.as_table_mut() else {
        return Ok(());
    };
    let metadata = package_table
        .entry("metadata")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(metadata_table) = metadata.as_table_mut() else {
        return Ok(());
    };
    let android = metadata_table
        .entry("android")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(android_table) = android.as_table_mut() else {
        return Ok(());
    };

    if !android_table.contains_key("package") {
        android_table.insert(
            "package".to_string(),
            Value::String("com.perro.perro_project".to_string()),
        );
        changed = true;
    }
    if !android_table.contains_key("build_targets") {
        android_table.insert(
            "build_targets".to_string(),
            Value::Array(vec![Value::String("aarch64-linux-android".to_string())]),
        );
        changed = true;
    }
    if !android_table.contains_key("label") {
        android_table.insert(
            "label".to_string(),
            Value::String("Perro Project".to_string()),
        );
        changed = true;
    }
    if !android_table.contains_key("min_sdk_version") {
        android_table.insert("min_sdk_version".to_string(), Value::Integer(26));
        changed = true;
    }
    if !android_table.contains_key("target_sdk_version") {
        android_table.insert("target_sdk_version".to_string(), Value::Integer(35));
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_scripts_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let deps = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    let manifest_dir = manifest_dir_for(path);
    let engine_root = engine_root_dir();
    let mut changed = ensure_existing_local_perro_deps(deps_table, &manifest_dir, &engine_root);
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_api");
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_runtime");

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_scripts_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let features = root
        .entry("features")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(features_table) = features.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;
    if !features_table.contains_key("dynamic-scripts") {
        features_table.insert("dynamic-scripts".to_string(), Value::Array(Vec::new()));
        changed = true;
    }
    if !features_table.contains_key("perro-demo") {
        features_table.insert("perro-demo".to_string(), Value::Array(Vec::new()));
        changed = true;
    }
    if !features_table.contains_key("perro-spec") {
        features_table.insert(
            "perro-spec".to_string(),
            Value::Array(vec![Value::String("perro_api/spec".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("steamworks") {
        features_table.insert(
            "steamworks".to_string(),
            Value::Array(vec![
                Value::String("perro_api/steamworks".to_string()),
                Value::String("perro_runtime/steamworks".to_string()),
            ]),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_dev_runner_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let deps = root
        .entry("dependencies")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(deps_table) = deps.as_table_mut() else {
        return Ok(());
    };

    let manifest_dir = manifest_dir_for(path);
    let engine_root = engine_root_dir();
    let mut changed = ensure_existing_local_perro_deps(deps_table, &manifest_dir, &engine_root);
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_app");
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_headless");
    changed |= ensure_local_perro_dep(deps_table, &manifest_dir, &engine_root, "perro_project");
    changed |= set_dep_optional(deps_table, "perro_app");
    changed |= set_dep_optional(deps_table, "perro_headless");

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn ensure_dev_runner_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = parse_toml_document_value(src) else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let features = root
        .entry("features")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(features_table) = features.as_table_mut() else {
        return Ok(());
    };

    let mut changed = false;

    changed |= ensure_feature_values(features_table, "default", &["app"]);
    changed |= ensure_feature_values(features_table, "app", &["dep:perro_app"]);
    changed |= ensure_feature_values(features_table, "headless", &["dep:perro_headless"]);
    changed |= ensure_feature_values(
        features_table,
        "headless_profile",
        &["perro_headless/profile"],
    );
    changed |= ensure_feature_values(
        features_table,
        "headless_steamworks",
        &["perro_headless/steamworks"],
    );

    if !features_table.contains_key("timings") {
        features_table.insert(
            "timings".to_string(),
            Value::Array(vec![Value::String("perro_app/fps".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("profile") {
        features_table.insert(
            "profile".to_string(),
            Value::Array(vec![Value::String("perro_app/profile".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("ui_profile") {
        features_table.insert(
            "ui_profile".to_string(),
            Value::Array(vec![Value::String("perro_app/ui_profile".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("mem_profile") {
        features_table.insert(
            "mem_profile".to_string(),
            Value::Array(vec![Value::String("perro_app/mem_profile".to_string())]),
        );
        changed = true;
    }
    if !features_table.contains_key("steamworks") {
        features_table.insert(
            "steamworks".to_string(),
            Value::Array(vec![Value::String("perro_app/steamworks".to_string())]),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    write_if_changed(path, &rendered)
}

fn manifest_dir_for(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| {
            manifest_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        })
}

fn engine_root_dir() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    raw.canonicalize().unwrap_or(raw)
}

fn ensure_existing_local_perro_deps(
    deps_table: &mut toml::map::Map<String, Value>,
    manifest_dir: &Path,
    engine_root: &Path,
) -> bool {
    let crate_names: Vec<String> = deps_table
        .keys()
        .filter(|name| crate_workspace_rel_path(name).is_some())
        .cloned()
        .collect();
    let mut changed = false;
    for crate_name in crate_names {
        changed |= ensure_local_perro_dep(deps_table, manifest_dir, engine_root, &crate_name);
    }
    changed
}

fn ensure_local_perro_dep(
    deps_table: &mut toml::map::Map<String, Value>,
    manifest_dir: &Path,
    engine_root: &Path,
    crate_name: &str,
) -> bool {
    let Some(mut spec) = local_perro_dep_spec(manifest_dir, engine_root, crate_name) else {
        return false;
    };
    if deps_table
        .get(crate_name)
        .and_then(Value::as_table)
        .and_then(|table| table.get("optional"))
        == Some(&Value::Boolean(true))
        && let Value::Table(table) = &mut spec
    {
        table.insert("optional".to_string(), Value::Boolean(true));
    }
    if deps_table.get(crate_name) == Some(&spec) {
        return false;
    }
    deps_table.insert(crate_name.to_string(), spec);
    true
}

fn set_dep_optional(deps: &mut toml::map::Map<String, Value>, name: &str) -> bool {
    let Some(Value::Table(spec)) = deps.get_mut(name) else {
        return false;
    };
    if spec.get("optional") == Some(&Value::Boolean(true)) {
        return false;
    }
    spec.insert("optional".to_string(), Value::Boolean(true));
    true
}

/// Default engine version requirement when the local crate cannot be read.
const ENGINE_FALLBACK_VERSION: &str = "0.1.0";

/// Emits `{ version = "x.y.z", path = "..." }`.
///
/// The version requirement is what makes the generated manifest valid without
/// the engine checkout: cargo prefers `path` when it resolves, and falls back to
/// the registry release when the path is gone (or the `[patch.crates-io]` block
/// is dropped). Path-only specs pinned every project to a local source tree.
fn local_perro_dep_spec(
    manifest_dir: &Path,
    engine_root: &Path,
    crate_name: &str,
) -> Option<Value> {
    let rel_crate_path = crate_workspace_rel_path(crate_name)?;
    let crate_dir = engine_root.join(rel_crate_path);
    let mut spec = toml::value::Table::new();
    spec.insert(
        "version".to_string(),
        Value::String(
            engine_crate_version(&crate_dir)
                .unwrap_or_else(|| ENGINE_FALLBACK_VERSION.to_string()),
        ),
    );
    if crate_dir.join("Cargo.toml").is_file() {
        spec.insert(
            "path".to_string(),
            Value::String(rel_path(manifest_dir, &crate_dir)),
        );
    }
    Some(Value::Table(spec))
}

/// Reads `package.version` from an engine crate so the generated dep pins the
/// version actually being built against, not a hardcoded guess.
///
/// Engine crates declare `version.workspace = true`, so the literal lives in the
/// engine root's `[workspace.package]`. Resolving it here is what makes a single
/// version bump propagate into every generated project manifest.
fn engine_crate_version(crate_dir: &Path) -> Option<String> {
    let src = fs::read_to_string(crate_dir.join("Cargo.toml")).ok()?;
    let value = parse_toml_document_value(src).ok()?;
    let version = value.get("package")?.get("version")?;
    if let Some(literal) = version.as_str() {
        return Some(literal.to_string());
    }
    if version.get("workspace").and_then(Value::as_bool) == Some(true) {
        return engine_workspace_version();
    }
    None
}

/// `[workspace.package] version` from the engine root manifest.
fn engine_workspace_version() -> Option<String> {
    let src = fs::read_to_string(engine_root_dir().join("Cargo.toml")).ok()?;
    let value = parse_toml_document_value(src).ok()?;
    value
        .get("workspace")?
        .get("package")?
        .get("version")?
        .as_str()
        .map(str::to_string)
}

fn ensure_scripts_manifest_rust_analyzer_cfg(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    if src.contains("cfg(rust_analyzer)") {
        return Ok(());
    }
    let mut out = src.trim_end().to_string();
    out.push_str(
        "\n\n[lints.rust]\nunexpected_cfgs = { level = \"warn\", check-cfg = [\"cfg(rust_analyzer)\"] }\n",
    );
    write_if_changed(path, &out)
}

fn ensure_patch_block_in_manifest(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let src = fs::read_to_string(path)?;
    let overrides = source_overrides_block_for_manifest(path, &src);
    let stripped = strip_patch_crates_io(&src);
    let mut out = stripped.trim_end().to_string();
    if !overrides.is_empty() {
        out.push_str("\n\n");
        out.push_str(&overrides);
        out.push('\n');
    }
    if src == out {
        return Ok(());
    }
    write_if_changed(path, &out)
}

fn strip_patch_crates_io(src: &str) -> String {
    let mut out = String::new();
    let mut in_patch = false;

    for line in src.lines() {
        let trimmed = line.trim();
        let is_header = trimmed.starts_with('[') && trimmed.ends_with(']');
        let is_patch_header = is_header
            && (trimmed == "[patch.crates-io]" || trimmed.starts_with("[patch.crates-io."));
        if is_patch_header {
            in_patch = true;
            continue;
        }
        if in_patch && is_header {
            in_patch = false;
        }
        if !in_patch {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Drops every TOML section whose first path segment matches `first_segments`.
///
/// Line-based on purpose: these manifests are hand-editable and round-tripping
/// them through `toml::to_string` would reorder and reflow the whole file.
fn strip_toml_sections(src: &str, first_segments: &[&str]) -> String {
    let mut out = String::new();
    let mut in_stripped = false;

    for line in src.lines() {
        let trimmed = line.trim();
        let is_header = trimmed.starts_with('[') && trimmed.ends_with(']');
        if is_header {
            let head = toml_header_first_segment(trimmed);
            in_stripped = first_segments.iter().any(|seg| *seg == head);
        }
        if !in_stripped {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// `[profile.dev.package."*"]` -> `profile`, `[[bin]]` -> `bin`.
fn toml_header_first_segment(header: &str) -> String {
    let inner = header
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let mut out = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';
    for ch in inner.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => in_quotes = false,
            '.' if !in_quotes => break,
            c if !in_quotes => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Union of every engine crate reachable from the workspace members, rendered
/// relative to the workspace root. Members must not carry their own patch block.
fn source_overrides_block_for_workspace(
    workspace_path: &Path,
    member_manifests: &[&Path],
) -> String {
    let engine_root = engine_root_dir();
    let workspace_dir = manifest_dir_for(workspace_path);

    let mut crates = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for manifest in member_manifests {
        let Ok(src) = fs::read_to_string(manifest) else {
            continue;
        };
        if let Some(direct) = direct_perro_deps_from_manifest(&src) {
            crates.extend(direct);
        }
        collect_perro_deps_from_local_path_deps(manifest, &src, &mut crates, &mut visited);
    }
    expand_transitive_perro_deps(&engine_root, &mut crates);
    if crates.is_empty() {
        return String::new();
    }

    let mut ordered_crates: Vec<_> = crates.into_iter().collect();
    ordered_crates.sort_by(|a, b| {
        let ka = crate_group_sort_key(a);
        let kb = crate_group_sort_key(b);
        ka.cmp(&kb).then_with(|| a.cmp(b))
    });

    render_patch_block(&workspace_dir, &engine_root, &ordered_crates)
}

/// Renders `[patch.crates-io]` over the engine crates that actually exist on
/// disk. With no engine checkout the block is empty and the version
/// requirements in `[dependencies]` resolve from the registry instead.
fn render_patch_block(manifest_dir: &Path, engine_root: &Path, crates: &[String]) -> String {
    let mut lines = Vec::new();
    for crate_name in crates {
        let Some(rel_crate_path) = crate_workspace_rel_path(crate_name) else {
            continue;
        };
        let crate_dir = engine_root.join(rel_crate_path);
        if !crate_dir.join("Cargo.toml").is_file() {
            continue;
        }
        let path = rel_path(manifest_dir, &crate_dir);
        lines.push(format!("{crate_name} = {{ path = \"{path}\" }}"));
    }
    if lines.is_empty() {
        return String::new();
    }
    lines.insert(0, "[patch.crates-io]".to_string());
    lines.join("\n")
}

fn source_overrides_block_for_manifest(manifest_path: &Path, manifest_src: &str) -> String {
    let engine_root = engine_root_dir();
    let manifest_dir = manifest_dir_for(manifest_path);

    let Some(mut crates) = direct_perro_deps_from_manifest(manifest_src) else {
        return String::new();
    };
    let mut visited = BTreeSet::new();
    collect_perro_deps_from_local_path_deps(manifest_path, manifest_src, &mut crates, &mut visited);
    expand_transitive_perro_deps(&engine_root, &mut crates);
    if crates.is_empty() {
        return String::new();
    }

    let mut ordered_crates: Vec<_> = crates.into_iter().collect();
    ordered_crates.sort_by(|a, b| {
        let ka = crate_group_sort_key(a);
        let kb = crate_group_sort_key(b);
        ka.cmp(&kb).then_with(|| a.cmp(b))
    });

    render_patch_block(&manifest_dir, &engine_root, &ordered_crates)
}

fn collect_perro_deps_from_local_path_deps(
    manifest_path: &Path,
    manifest_src: &str,
    crates: &mut BTreeSet<String>,
    visited: &mut BTreeSet<PathBuf>,
) {
    let Some(manifest_dir) = manifest_path.parent() else {
        return;
    };
    for rel_path in local_path_dependencies_from_manifest(manifest_src) {
        let dep_manifest = manifest_dir.join(rel_path).join("Cargo.toml");
        let dep_manifest = dep_manifest.canonicalize().unwrap_or(dep_manifest);
        if !visited.insert(dep_manifest.clone()) {
            continue;
        }
        let Ok(dep_src) = fs::read_to_string(&dep_manifest) else {
            continue;
        };
        if let Some(extra) = direct_perro_deps_from_manifest(&dep_src) {
            crates.extend(extra);
        }
        collect_perro_deps_from_local_path_deps(&dep_manifest, &dep_src, crates, visited);
    }
}

fn direct_perro_deps_from_manifest(src: &str) -> Option<BTreeSet<String>> {
    let value: Value = parse_toml_document_value(src).ok()?;
    let mut out = BTreeSet::new();
    collect_perro_dep_keys(value.get("dependencies"), &mut out);
    collect_perro_dep_keys(value.get("build-dependencies"), &mut out);
    collect_perro_dep_keys(value.get("dev-dependencies"), &mut out);
    Some(out)
}

fn local_path_dependencies_from_manifest(src: &str) -> Vec<String> {
    let Ok(value) = parse_toml_document_value(src) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_local_path_deps(value.get("dependencies"), &mut out);
    collect_local_path_deps(value.get("build-dependencies"), &mut out);
    collect_local_path_deps(value.get("dev-dependencies"), &mut out);
    out
}

fn collect_perro_dep_keys(table: Option<&Value>, out: &mut BTreeSet<String>) {
    let Some(table) = table.and_then(Value::as_table) else {
        return;
    };
    for key in table.keys() {
        if key.starts_with("perro_") || key == "perro_api" {
            out.insert(key.to_string());
        }
    }
}

fn collect_local_path_deps(table: Option<&Value>, out: &mut Vec<String>) {
    let Some(table) = table.and_then(Value::as_table) else {
        return;
    };
    for dep in table.values() {
        let Some(dep_table) = dep.as_table() else {
            continue;
        };
        let Some(path) = dep_table.get("path").and_then(Value::as_str) else {
            continue;
        };
        out.push(path.to_string());
    }
}

fn expand_transitive_perro_deps(engine_root: &Path, crates: &mut BTreeSet<String>) {
    let mut queue: Vec<String> = crates.iter().cloned().collect();
    while let Some(crate_name) = queue.pop() {
        let Some(rel_path) = crate_workspace_rel_path(&crate_name) else {
            continue;
        };
        let manifest = engine_root.join(rel_path).join("Cargo.toml");
        let Ok(src) = fs::read_to_string(manifest) else {
            continue;
        };
        let Some(extra) = direct_perro_deps_from_manifest(&src) else {
            continue;
        };
        for dep in extra {
            if crates.insert(dep.clone()) {
                queue.push(dep);
            }
        }
    }
}

fn crate_workspace_rel_path(crate_name: &str) -> Option<&'static str> {
    match crate_name {
        "perro_animation" => Some("perro_source/core/perro_animation"),
        "perro_nodes" => Some("perro_source/core/perro_nodes"),
        "perro_ui" => Some("perro_source/core/perro_ui"),
        "perro_structs" => Some("perro_source/core/perro_structs"),
        "perro_ids" => Some("perro_source/core/perro_ids"),
        "perro_variant" => Some("perro_source/core/perro_variant"),
        "perro_particle_math" => Some("perro_source/core/perro_particle_math"),
        "perro_csv" => Some("perro_source/core/perro_csv"),
        "perro_runtime" => Some("perro_source/runtime_project/perro_runtime"),
        "perro_headless" => Some("perro_source/runtime_project/perro_headless"),
        "perro_internal_updates" => Some("perro_source/runtime_project/perro_internal_updates"),
        "perro_scene" => Some("perro_source/runtime_project/perro_scene"),
        "perro_runtime_api" => Some("perro_source/api_modules/perro_runtime_api"),
        "perro_resource_api" => Some("perro_source/api_modules/perro_resource_api"),
        "perro_api" => Some("perro_source/api_modules/perro_api"),
        "perro_modules" => Some("perro_source/api_modules/perro_modules"),
        "perro_networking" => Some("perro_source/api_modules/perro_networking"),
        "perro_input_api" => Some("perro_source/api_modules/perro_input_api"),
        "perro_web" => Some("perro_source/api_modules/perro_web"),
        "perro_steamworks" => Some("perro_source/api_modules/perro_steamworks"),
        "perro_render_bridge" => Some("perro_source/render_stack/perro_render_bridge"),
        "perro_graphics" => Some("perro_source/render_stack/perro_graphics"),
        "perro_meshlets" => Some("perro_source/render_stack/perro_meshlets"),
        "perro_app" => Some("perro_source/render_stack/perro_app"),
        "perro_scripting" => Some("perro_source/script_stack/perro_scripting"),
        "perro_scripting_macros" => Some("perro_source/script_stack/perro_scripting_macros"),
        "perro_compiler" => Some("perro_source/build_pipeline/perro_compiler"),
        "perro_static_pipeline" => Some("perro_source/build_pipeline/perro_static_pipeline"),
        "perro_io" => Some("perro_source/io_stack/perro_io"),
        "perro_assets" => Some("perro_source/io_stack/perro_assets"),
        "perro_pawdio" => Some("perro_source/audio_stack/perro_pawdio"),
        "perro_project" => Some("perro_source/runtime_project/perro_project"),
        "perro_cli" => Some("perro_source/devtools/perro_cli"),
        "perro_dev_runner" => Some("perro_source/devtools/perro_dev_runner"),
        _ => None,
    }
}

fn crate_group_sort_key(crate_name: &str) -> u8 {
    let Some(rel) = crate_workspace_rel_path(crate_name) else {
        return u8::MAX;
    };
    if rel.starts_with("perro_source/core/") {
        return 0;
    }
    if rel.starts_with("perro_source/runtime_project/") {
        return 1;
    }
    if rel.starts_with("perro_source/api_modules/") {
        return 2;
    }
    if rel.starts_with("perro_source/render_stack/") {
        return 3;
    }
    if rel.starts_with("perro_source/script_stack/") {
        return 4;
    }
    if rel.starts_with("perro_source/build_pipeline/") {
        return 5;
    }
    if rel.starts_with("perro_source/io_stack/") {
        return 6;
    }
    if rel.starts_with("perro_source/devtools/") {
        return 7;
    }
    8
}
