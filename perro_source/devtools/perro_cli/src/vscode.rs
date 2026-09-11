use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn update_workspace_vscode_linked_projects(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<(), String> {
    let settings_path = workspace_root.join(".vscode").join("settings.json");
    let mut json: Value = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)
            .map_err(|err| format!("failed to read {}: {err}", settings_path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|err| format!("failed to parse {} as JSON: {err}", settings_path.display()))?
    } else {
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        Value::Object(Default::default())
    };
    let Some(root) = json.as_object_mut() else {
        return Err(format!(
            "expected {} to contain a JSON object",
            settings_path.display()
        ));
    };

    let workspace_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());
    let entry = root
        .entry("rust-analyzer.linkedProjects".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = entry.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.linkedProjects` to be an array in {}",
            settings_path.display()
        ));
    };

    arr.retain(|v| {
        let Some(s) = v.as_str() else {
            return false;
        };
        let p = PathBuf::from(s);
        let full = if p.is_absolute() {
            p
        } else {
            workspace_root.join(p)
        };
        full.exists()
    });

    for rel in workspace_internal_project_manifests(&workspace_root, project_dir)? {
        if !arr.iter().any(|v| v.as_str() == Some(rel.as_str())) {
            arr.push(Value::String(rel));
        }
    }

    let vfs = root
        .entry("rust-analyzer.vfs.extraIncludes".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(vfs_arr) = vfs.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.vfs.extraIncludes` to be an array in {}",
            settings_path.display()
        ));
    };

    vfs_arr.retain(|v| {
        let Some(s) = v.as_str() else {
            return false;
        };
        let Some(path_part) = s.strip_prefix("${workspaceFolder}/") else {
            return true;
        };
        let trimmed = path_part.trim_end_matches('/').trim_end_matches('\\');
        workspace_root.join(trimmed).exists()
    });

    for vfs_entry in workspace_internal_project_vfs_entries(&workspace_root, project_dir)? {
        if !vfs_arr
            .iter()
            .any(|v| v.as_str() == Some(vfs_entry.as_str()))
        {
            vfs_arr.push(Value::String(vfs_entry));
        }
    }

    let rendered = serde_json::to_string_pretty(&json).map_err(|err| {
        format!(
            "failed to render {} as JSON: {err}",
            settings_path.display()
        )
    })?;
    fs::write(&settings_path, format!("{rendered}\n"))
        .map_err(|err| format!("failed to write {}: {err}", settings_path.display()))?;
    Ok(())
}

fn workspace_internal_project_roots(
    workspace_root: &Path,
    _project_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();

    let playground_root = workspace_root.join("playground");
    if playground_root.exists() {
        let entries = fs::read_dir(&playground_root).map_err(|err| {
            format!(
                "failed to read playground directory {}: {err}",
                playground_root.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                format!(
                    "failed to read playground directory entry in {}: {err}",
                    playground_root.display()
                )
            })?;
            let path = entry.path();
            if !path.is_dir() || !path.join("project.toml").exists() {
                continue;
            }
            roots.push(path);
        }
    }

    roots.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    roots.dedup();
    Ok(roots)
}

fn workspace_internal_project_manifests(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for root in workspace_internal_project_roots(workspace_root, project_dir)? {
        let scripts_manifest = root
            .join(".perro")
            .join("scripts")
            .join("Cargo.toml")
            .canonicalize()
            .unwrap_or_else(|_| root.join(".perro").join("scripts").join("Cargo.toml"));
        let Ok(rel) = scripts_manifest
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
        else {
            continue;
        };
        out.push(rel);
    }
    Ok(out)
}

fn workspace_internal_project_vfs_entries(
    workspace_root: &Path,
    project_dir: &Path,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for root in workspace_internal_project_roots(workspace_root, project_dir)? {
        let res_dir = root
            .join("res")
            .canonicalize()
            .unwrap_or_else(|_| root.join("res"));
        let Ok(rel_res) = res_dir
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
        else {
            continue;
        };
        out.push(format!("${{workspaceFolder}}/{rel_res}/"));
    }
    Ok(out)
}

pub(crate) fn update_project_vscode_linked_projects(project_dir: &Path) -> Result<(), String> {
    let project_cfg = perro_project::load_project_toml(project_dir).map_err(|err| {
        format!(
            "failed to load {}: {err}",
            project_dir.join("project.toml").display()
        )
    })?;
    let settings_dir = project_dir.join(".vscode");
    fs::create_dir_all(&settings_dir)
        .map_err(|err| format!("failed to create {}: {err}", settings_dir.display()))?;

    let settings_path = settings_dir.join("settings.json");
    let mut json: Value = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)
            .map_err(|err| format!("failed to read {}: {err}", settings_path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|err| format!("failed to parse {} as JSON: {err}", settings_path.display()))?
    } else {
        Value::Object(Default::default())
    };

    let Some(root) = json.as_object_mut() else {
        return Err(format!(
            "expected {} to contain a JSON object",
            settings_path.display()
        ));
    };

    let vfs_entry = "${workspaceFolder}/res/".to_string();

    sync_steamworks_feature(
        root,
        "rust-analyzer.cargo.features",
        project_cfg.steam.enabled,
        &settings_path,
    )?;
    // Explicit check features override cargo features; keep diagnostics in sync too.
    if root
        .get("rust-analyzer.check.features")
        .is_some_and(|value| !value.is_null())
    {
        sync_steamworks_feature(
            root,
            "rust-analyzer.check.features",
            project_cfg.steam.enabled,
            &settings_path,
        )?;
    }

    let entry = root
        .entry("rust-analyzer.linkedProjects".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = entry.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.linkedProjects` to be an array in {}",
            settings_path.display()
        ));
    };
    arr.retain(|v| {
        let Some(s) = v.as_str() else {
            return false;
        };
        if Path::new(s).is_absolute() {
            return Path::new(s).exists();
        }
        project_dir.join(s).exists()
    });
    for linked_manifest in project_internal_linked_manifests(project_dir)? {
        if !arr
            .iter()
            .any(|v| v.as_str() == Some(linked_manifest.as_str()))
        {
            arr.push(Value::String(linked_manifest));
        }
    }

    let vfs = root
        .entry("rust-analyzer.vfs.extraIncludes".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(vfs_arr) = vfs.as_array_mut() else {
        return Err(format!(
            "expected `rust-analyzer.vfs.extraIncludes` to be an array in {}",
            settings_path.display()
        ));
    };
    if !vfs_arr
        .iter()
        .any(|v| v.as_str() == Some(vfs_entry.as_str()))
    {
        vfs_arr.push(Value::String(vfs_entry));
    }

    let rendered = serde_json::to_string_pretty(&json).map_err(|err| {
        format!(
            "failed to render {} as JSON: {err}",
            settings_path.display()
        )
    })?;
    fs::write(&settings_path, format!("{rendered}\n"))
        .map_err(|err| format!("failed to write {}: {err}", settings_path.display()))?;
    Ok(())
}

fn sync_steamworks_feature(
    root: &mut serde_json::Map<String, Value>,
    key: &str,
    enabled: bool,
    settings_path: &Path,
) -> Result<(), String> {
    let features = root
        .entry(key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if features.as_str() == Some("all") {
        return Ok(());
    }
    let Some(features) = features.as_array_mut() else {
        return Err(format!(
            "expected `{key}` to be an array or \"all\" in {}",
            settings_path.display()
        ));
    };
    features.retain(|feature| feature.as_str() != Some("steamworks"));
    if enabled {
        features.push(Value::String("steamworks".to_string()));
    }
    Ok(())
}

fn project_internal_linked_manifests(project_dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::<String>::new();
    out.push(".perro/scripts/Cargo.toml".to_string());

    let dlc_root = project_dir.join(".perro").join("dlc");
    if dlc_root.exists() {
        let entries = fs::read_dir(&dlc_root)
            .map_err(|err| format!("failed to read {}: {err}", dlc_root.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                format!(
                    "failed to read dlc generated entry in {}: {err}",
                    dlc_root.display()
                )
            })?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            out.push(format!(".perro/dlc/{name}/scripts/Cargo.toml"));
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn project_settings_follow_steam_toggle() {
        let temp_root = std::env::temp_dir()
            .canonicalize()
            .expect("test setup must succeed");
        let project_dir = temp_root.join(format!(
            "perro-vscode-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test setup must succeed")
                .as_nanos()
        ));
        fs::create_dir_all(project_dir.join(".vscode")).expect("test setup must succeed");
        let settings_path = project_dir.join(".vscode/settings.json");
        fs::write(
            &settings_path,
            json!({
                "editor.tabSize": 4,
                "rust-analyzer.cargo.features": ["perro-spec"],
                "rust-analyzer.check.features": ["perro-spec"]
            })
            .to_string(),
        )
        .expect("test setup must succeed");

        for enabled in [true, true, false, true] {
            fs::write(project_dir.join("project.toml"), format!(
                "[project]\nname = \"Test\"\nmain_scene = \"res://main.scn\"\n[steam]\nenabled = {enabled}\napp_id = 480\n"
            )).expect("test setup must succeed");
            update_project_vscode_linked_projects(&project_dir).expect("test setup must succeed");
            let settings: Value = serde_json::from_str(
                &fs::read_to_string(&settings_path).expect("test setup must succeed"),
            )
            .expect("test setup must succeed");
            let expected = if enabled {
                json!(["perro-spec", "steamworks"])
            } else {
                json!(["perro-spec"])
            };
            assert_eq!(settings["rust-analyzer.cargo.features"], expected);
            assert_eq!(settings["rust-analyzer.check.features"], expected);
            assert_eq!(settings["editor.tabSize"], 4);
            assert_eq!(
                settings["rust-analyzer.linkedProjects"],
                json!([".perro/scripts/Cargo.toml"])
            );
            assert_eq!(
                settings["rust-analyzer.vfs.extraIncludes"],
                json!(["${workspaceFolder}/res/"])
            );
        }
        let resolved = project_dir.canonicalize().expect("test setup must succeed");
        assert!(resolved.starts_with(&temp_root) && resolved != temp_root);
        fs::remove_dir_all(resolved).expect("test setup must succeed");
    }

    #[test]
    fn steam_feature_defaults_and_all_are_supported() {
        let key = "rust-analyzer.cargo.features";
        let mut settings = serde_json::Map::new();
        sync_steamworks_feature(&mut settings, key, true, Path::new("settings.json"))
            .expect("test setup must succeed");
        assert_eq!(settings[key], json!(["steamworks"]));
        settings.insert(key.to_string(), json!("all"));
        for enabled in [false, true] {
            sync_steamworks_feature(&mut settings, key, enabled, Path::new("settings.json"))
                .expect("test setup must succeed");
            assert_eq!(settings[key], "all");
        }
    }

    #[test]
    fn invalid_feature_setting_reports_path() {
        let key = "rust-analyzer.cargo.features";
        let mut settings = serde_json::Map::new();
        settings.insert(key.to_string(), json!(true));
        let error = sync_steamworks_feature(&mut settings, key, true, Path::new("settings.json"))
            .expect_err("invalid test input");
        assert!(error.contains(key) && error.contains("settings.json"));
    }
}
