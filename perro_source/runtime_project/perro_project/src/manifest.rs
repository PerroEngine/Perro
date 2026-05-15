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
    let dev_runner_main = project_root
        .join(".perro")
        .join("dev_runner")
        .join("src")
        .join("main.rs");
    let scripts_cargo_config = project_root
        .join(".perro")
        .join("scripts")
        .join(".cargo")
        .join("config.toml");
    ensure_project_build_script(&project_build_script)?;
    ensure_project_target_dir_config(&project_cargo_config)?;
    ensure_scripts_crate_sync(&scripts_manifest)?;
    ensure_project_manifest_deps(&project_manifest)?;
    ensure_project_manifest_icon_build_support(&project_manifest)?;
    ensure_project_manifest_features(&project_manifest)?;
    ensure_scripts_manifest_deps(&scripts_manifest)?;
    ensure_scripts_manifest_features(&scripts_manifest)?;
    ensure_scripts_manifest_user_deps(project_root, &scripts_manifest)?;
    ensure_dev_runner_source_sync(&dev_runner_manifest, &dev_runner_main)?;
    ensure_dev_runner_manifest_deps(&dev_runner_manifest)?;
    ensure_dev_runner_manifest_features(&dev_runner_manifest)?;
    ensure_dev_runner_manifest_profile_debug(&dev_runner_manifest)?;
    ensure_scripts_manifest_rust_analyzer_cfg(&scripts_manifest)?;
    ensure_patch_block_in_manifest(&project_manifest)?;
    ensure_patch_block_in_manifest(&scripts_manifest)?;
    ensure_patch_block_in_manifest(&dev_runner_manifest)?;
    ensure_scripts_target_dir_config(&scripts_cargo_config)?;
    Ok(())
}

fn ensure_scripts_crate_sync(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(path, &default_scripts_crate_toml())
}

fn ensure_dev_runner_source_sync(manifest_path: &Path, main_rs_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = main_rs_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(manifest_path, &default_dev_runner_crate_toml())?;
    write_if_changed(main_rs_path, &default_dev_runner_main_rs())?;
    Ok(())
}

fn ensure_project_build_script(path: &Path) -> std::io::Result<()> {
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
    let Ok(mut scripts_value) = scripts_src.parse::<Value>() else {
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
        let deps_value = deps_src.parse::<Value>().map_err(|err| {
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
    fs::write(scripts_manifest, rendered)
}

fn ensure_scripts_target_dir_config(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_scripts_cargo_config_toml())
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
    let Ok(mut value) = src.parse::<Value>() else {
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

    let mut changed = false;

    if !deps_table.contains_key("perro_api") {
        deps_table.insert("perro_api".to_string(), Value::String("0.1.0".to_string()));
        changed = true;
    }
    if !deps_table.contains_key("perro_runtime") {
        deps_table.insert(
            "perro_runtime".to_string(),
            Value::String("0.1.0".to_string()),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_project_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
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
    if !features_table.contains_key("steamworks") {
        features_table.insert(
            "steamworks".to_string(),
            Value::Array(vec![
                Value::String("perro_app/steamworks".to_string()),
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
    fs::write(path, rendered)
}

fn ensure_project_manifest_icon_build_support(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
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

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_scripts_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
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

    let mut changed = false;

    if !deps_table.contains_key("perro_api") {
        deps_table.insert("perro_api".to_string(), Value::String("0.1.0".to_string()));
        changed = true;
    }
    if !deps_table.contains_key("perro_runtime") {
        deps_table.insert(
            "perro_runtime".to_string(),
            Value::String("0.1.0".to_string()),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_scripts_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
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

    if features_table.contains_key("steamworks") {
        return Ok(());
    }
    features_table.insert(
        "steamworks".to_string(),
        Value::Array(vec![
            Value::String("perro_api/steamworks".to_string()),
            Value::String("perro_runtime/steamworks".to_string()),
        ]),
    );

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_dev_runner_manifest_deps(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
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

    let mut changed = false;

    if !deps_table.contains_key("perro_app") {
        deps_table.insert("perro_app".to_string(), Value::String("0.1.0".to_string()));
        changed = true;
    }
    if !deps_table.contains_key("perro_project") {
        deps_table.insert(
            "perro_project".to_string(),
            Value::String("0.1.0".to_string()),
        );
        changed = true;
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_dev_runner_manifest_features(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
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
    fs::write(path, rendered)
}

fn ensure_dev_runner_manifest_profile_debug(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let src = fs::read_to_string(path)?;
    let Ok(mut value) = src.parse::<Value>() else {
        return Ok(());
    };
    let Some(root) = value.as_table_mut() else {
        return Ok(());
    };

    let profile = root
        .entry("profile")
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(profile_table) = profile.as_table_mut() else {
        return Ok(());
    };
    let mut changed = false;

    {
        let release = profile_table
            .entry("release")
            .or_insert_with(|| Value::Table(Default::default()));
        let Some(release_table) = release.as_table_mut() else {
            return Ok(());
        };
        if !release_table.contains_key("debug") {
            release_table.insert("debug".to_string(), Value::Boolean(true));
            changed = true;
        }
    }

    {
        let dev = profile_table
            .entry("dev")
            .or_insert_with(|| Value::Table(Default::default()));
        let Some(dev_table) = dev.as_table_mut() else {
            return Ok(());
        };
        if !dev_table.contains_key("opt-level") {
            dev_table.insert("opt-level".to_string(), Value::Integer(1));
            changed = true;
        }

        let dev_package = dev_table
            .entry("package")
            .or_insert_with(|| Value::Table(Default::default()));
        let Some(dev_package_table) = dev_package.as_table_mut() else {
            return Ok(());
        };
        changed |= ensure_dev_package_opt_level(dev_package_table, "perro_runtime");
        changed |= ensure_dev_package_opt_level(dev_package_table, "perro_app");
        changed |= ensure_dev_package_opt_level(dev_package_table, "perro_graphics");
        changed |= ensure_dev_package_opt_level(dev_package_table, "perro_physics");
        changed |= ensure_dev_package_opt_level(dev_package_table, "rapier2d");
        changed |= ensure_dev_package_opt_level(dev_package_table, "rapier3d");
        changed |= ensure_dev_package_opt_level(dev_package_table, "parry2d");
        changed |= ensure_dev_package_opt_level(dev_package_table, "parry3d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "perro_runtime");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "perro_physics");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "rapier2d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "rapier3d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "parry2d");
        changed |= ensure_dev_package_fast_checks(dev_package_table, "parry3d");
    }

    if !changed {
        return Ok(());
    }

    let rendered = toml::to_string(&value)
        .map_err(|err| std::io::Error::other(format!("failed to render Cargo.toml: {err}")))?;
    fs::write(path, rendered)
}

fn ensure_dev_package_opt_level(
    dev_package_table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
) -> bool {
    let package = dev_package_table
        .entry(crate_name.to_string())
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(package_table) = package.as_table_mut() else {
        return false;
    };
    if package_table.contains_key("opt-level") {
        return false;
    }
    package_table.insert("opt-level".to_string(), Value::Integer(3));
    true
}

fn ensure_dev_package_fast_checks(
    dev_package_table: &mut toml::map::Map<String, Value>,
    crate_name: &str,
) -> bool {
    let package = dev_package_table
        .entry(crate_name.to_string())
        .or_insert_with(|| Value::Table(Default::default()));
    let Some(package_table) = package.as_table_mut() else {
        return false;
    };
    let mut changed = false;
    if !package_table.contains_key("debug-assertions") {
        package_table.insert("debug-assertions".to_string(), Value::Boolean(false));
        changed = true;
    }
    if !package_table.contains_key("overflow-checks") {
        package_table.insert("overflow-checks".to_string(), Value::Boolean(false));
        changed = true;
    }
    changed
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
    fs::write(path, out)
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
    fs::write(path, out)
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

fn source_overrides_block_for_manifest(manifest_path: &Path, manifest_src: &str) -> String {
    let engine_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("..")
        });
    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| {
            manifest_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        });

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

    let mut lines = Vec::new();
    lines.push("[patch.crates-io]".to_string());
    for crate_name in ordered_crates {
        let Some(rel_crate_path) = crate_workspace_rel_path(&crate_name) else {
            continue;
        };
        let path = rel_path(&manifest_dir, &engine_root.join(rel_crate_path));
        lines.push(format!("{crate_name} = {{ path = \"{path}\" }}"));
    }
    lines.join("\n")
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
    let value: Value = src.parse::<Value>().ok()?;
    let mut out = BTreeSet::new();
    collect_perro_dep_keys(value.get("dependencies"), &mut out);
    collect_perro_dep_keys(value.get("build-dependencies"), &mut out);
    collect_perro_dep_keys(value.get("dev-dependencies"), &mut out);
    Some(out)
}

fn local_path_dependencies_from_manifest(src: &str) -> Vec<String> {
    let Ok(value) = src.parse::<Value>() else {
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
        "perro_internal_updates" => Some("perro_source/runtime_project/perro_internal_updates"),
        "perro_scene" => Some("perro_source/runtime_project/perro_scene"),
        "perro_runtime_api" => Some("perro_source/api_modules/perro_runtime_api"),
        "perro_resource_api" => Some("perro_source/api_modules/perro_resource_api"),
        "perro_api" => Some("perro_source/api_modules/perro_api"),
        "perro_modules" => Some("perro_source/api_modules/perro_modules"),
        "perro_networking" => Some("perro_source/api_modules/perro_networking"),
        "perro_input_api" => Some("perro_source/api_modules/perro_input_api"),
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
