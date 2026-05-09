use crate::install::normalize_powershell_path;
use crate::project::{maybe_open_file_in_editor, maybe_open_project_in_new_window, prompt_text};
use crate::vscode::{
    update_project_vscode_linked_projects, update_workspace_vscode_linked_projects,
};
use crate::{
    DEFAULT_PROJECT_NAME, find_project_root, log_done, log_step, parse_flag_value,
    resolve_local_path, workspace_root,
};
use perro_compiler::{ScriptsBuildProfile, compile_scripts_with_profile};
use perro_project::{create_new_project, default_script_empty_rs};
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

fn sanitize_script_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("script name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("script name must not include path separators".to_string());
    }
    let mut out = String::with_capacity(trimmed.len() + 3);
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let mut rendered = out.trim_matches('.').to_string();
    if rendered.is_empty() {
        return Err("script name must include at least one valid character".to_string());
    }
    if !rendered.ends_with(".rs") {
        rendered.push_str(".rs");
    }
    Ok(rendered)
}

fn sanitize_scene_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("scene name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("scene name must not include path separators".to_string());
    }
    let mut out = String::with_capacity(trimmed.len() + 4);
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let mut rendered = out.trim_matches('.').to_string();
    if rendered.is_empty() {
        return Err("scene name must include at least one valid character".to_string());
    }
    if !rendered.ends_with(".scn") {
        rendered.push_str(".scn");
    }
    Ok(rendered)
}

fn sanitize_animation_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("animation name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("animation name must not include path separators".to_string());
    }
    let mut out = String::with_capacity(trimmed.len() + 6);
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let mut rendered = out.trim_matches('.').to_string();
    if rendered.is_empty() {
        return Err("animation name must include at least one valid character".to_string());
    }
    if !rendered.ends_with(".panim") {
        rendered.push_str(".panim");
    }
    Ok(rendered)
}

fn sanitize_panimtree_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("animation tree name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("animation tree name must not include path separators".to_string());
    }
    let mut out = String::with_capacity(trimmed.len() + 10);
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let mut rendered = out.trim_matches('.').to_string();
    if rendered.is_empty() {
        return Err("animation tree name must include at least one valid character".to_string());
    }
    if !rendered.ends_with(".panimtree") {
        rendered.push_str(".panimtree");
    }
    Ok(rendered)
}

fn resolve_res_subdir(
    input: &str,
    res_root: &Path,
    allowed_prefix: &str,
) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(res_root.to_path_buf());
    }

    let rel = if let Some(stripped) = trimmed.strip_prefix(allowed_prefix) {
        stripped.trim_start_matches('/').trim_start_matches('\\')
    } else if trimmed.contains("://") {
        return Err(format!(
            "path must use `{allowed_prefix}*` or `/...` style segments"
        ));
    } else if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        trimmed.trim_start_matches('/').trim_start_matches('\\')
    } else {
        trimmed
    };

    let rel_path = PathBuf::from(rel);
    if rel_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("res subdir cannot contain `..` segments".to_string());
    }
    Ok(res_root.join(rel_path))
}

pub(crate) fn validate_dlc_name(raw: &str) -> Result<String, String> {
    let dlc = raw.trim();
    if dlc.is_empty() {
        return Err("dlc name cannot be empty".to_string());
    }
    if dlc.eq_ignore_ascii_case("self") {
        return Err("dlc name `self` is reserved".to_string());
    }
    if dlc.contains('/') || dlc.contains('\\') {
        return Err("dlc name must not include path separators".to_string());
    }
    if Path::new(dlc)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("dlc name must not include `..` segments".to_string());
    }
    Ok(dlc.to_string())
}

fn resolve_content_root(project_dir: &Path, args: &[String]) -> Result<PathBuf, String> {
    if let Some(raw_dlc) = parse_flag_value(args, "--dlc") {
        let dlc = validate_dlc_name(&raw_dlc)?;
        return Ok(project_dir.join("dlcs").join(dlc));
    }

    let res_root = project_dir.join("res");
    if !res_root.exists() {
        return Err(format!("res directory not found at {}", res_root.display()));
    }
    Ok(res_root)
}

fn resolve_content_prefix(args: &[String]) -> Result<String, String> {
    if let Some(raw_dlc) = parse_flag_value(args, "--dlc") {
        let dlc = validate_dlc_name(&raw_dlc)?;
        return Ok(format!("dlc://{dlc}/"));
    }
    Ok("res://".to_string())
}

fn write_new_file(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!("file already exists: {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(())
}

fn sanitize_project_dir_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "perro_project".to_string();
    }

    let mut out = String::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }

    let collapsed = out.trim_matches('.');
    if collapsed.is_empty() {
        "perro_project".to_string()
    } else {
        collapsed.to_string()
    }
}

pub(crate) fn new_command(args: &[String], cwd: &Path) -> Result<(), String> {
    if args
        .iter()
        .any(|a| a == "--build-scripts" || a == "--open" || a == "--no-open")
    {
        return Err("`perro new` only accepts --path and --name".to_string());
    }
    let mut project_name = parse_flag_value(args, "--name");
    let mut base_dir_input = parse_flag_value(args, "--path");

    if project_name.is_none() && base_dir_input.is_none() {
        if !io::stdin().is_terminal() {
            return Err(
                "`perro new` without --name/--path needs interactive terminal input".to_string(),
            );
        }
        let input_name = prompt_text(
            &format!("Project name [{DEFAULT_PROJECT_NAME}]: "),
            Some(DEFAULT_PROJECT_NAME),
        )?;
        let input_root = prompt_text(
            &format!("Project root folder [{}]: ", normalize_powershell_path(cwd)),
            Some(&normalize_powershell_path(cwd)),
        )?;
        project_name = Some(input_name);
        base_dir_input = Some(input_root);
    }

    let base_dir = base_dir_input
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let base_dir = base_dir.canonicalize().unwrap_or(base_dir);
    let project_name = project_name.unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());
    let project_dir = base_dir.join(sanitize_project_dir_name(&project_name));

    create_new_project(&project_dir, &project_name).map_err(|err| {
        format!(
            "failed to create project at {}: {err}",
            project_dir.display()
        )
    })?;
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    println!(
        "created project `{}` at {}",
        project_name,
        normalize_powershell_path(&project_dir)
    );
    maybe_open_project_in_new_window(&project_dir)?;
    Ok(())
}

pub(crate) fn new_dlc_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let dlc_name = validate_dlc_name(&raw_name)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    if !project_dir.join("project.toml").exists() {
        return Err(format!(
            "invalid --path `{}` for new_dlc. Use project root (directory containing project.toml).",
            project_dir.display()
        ));
    }

    let dlc_root = project_dir.join("dlcs").join(&dlc_name);
    if dlc_root.exists() {
        return Err(format!("dlc already exists: {}", dlc_root.display()));
    }

    fs::create_dir_all(dlc_root.join("scenes"))
        .map_err(|err| format!("failed to create dlc scenes dir: {err}"))?;
    fs::create_dir_all(dlc_root.join("scripts"))
        .map_err(|err| format!("failed to create dlc scripts dir: {err}"))?;
    fs::create_dir_all(dlc_root.join("materials"))
        .map_err(|err| format!("failed to create dlc materials dir: {err}"))?;
    fs::create_dir_all(dlc_root.join("meshes"))
        .map_err(|err| format!("failed to create dlc meshes dir: {err}"))?;

    let script_path = dlc_root.join("scripts").join("script.rs");
    write_new_file(&script_path, &default_script_empty_rs())?;

    let scene_path = dlc_root.join("scenes").join("main.scn");
    let scene = format!(
        "$root = @main\n\n[main]\nscript = \"dlc://{dlc_name}/scripts/script.rs\"\n[Node2D]\n    position = (0, 0)\n[/Node2D]\n[/main]\n"
    );
    write_new_file(&scene_path, &scene)?;
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;

    println!(
        "created dlc `{}` at {}",
        dlc_name,
        normalize_powershell_path(&dlc_root)
    );
    println!("reference scene with: dlc://{}/scenes/main.scn", dlc_name);
    println!(
        "reference script with: dlc://{}/scripts/script.rs",
        dlc_name
    );
    maybe_open_file_in_editor(args, &scene_path)?;
    Ok(())
}

pub(crate) fn new_script_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let file_name = sanitize_script_file_name(&raw_name)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let content_root = resolve_content_root(&project_dir, args)?;
    let content_prefix = resolve_content_prefix(args)?;

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &content_root, &content_prefix)?
    } else {
        content_root
    };

    let target_path = target_dir.join(file_name);
    write_new_file(&target_path, &default_script_empty_rs())?;
    println!(
        "created script at {}",
        normalize_powershell_path(&target_path)
    );
    maybe_open_file_in_editor(args, &target_path)?;
    update_workspace_vscode_linked_projects(&workspace_root(), &project_dir)?;
    update_project_vscode_linked_projects(&project_dir)?;
    log_step("Building Scripts");
    compile_scripts_with_profile(&project_dir, ScriptsBuildProfile::Debug)
        .map(|_| {
            log_done("Scripts Built");
        })
        .map_err(|err| {
            format!(
                "scripts pipeline failed for {}: {err}",
                project_dir.display()
            )
        })?;
    Ok(())
}

fn parse_scene_template(args: &[String]) -> Result<SceneTemplate, String> {
    let Some(raw) = parse_flag_value(args, "--template") else {
        return Ok(SceneTemplate::TwoD);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "2d" => Ok(SceneTemplate::TwoD),
        "3d" => Ok(SceneTemplate::ThreeD),
        _ => Err("invalid --template value. Use 2D or 3D.".to_string()),
    }
}

enum SceneTemplate {
    TwoD,
    ThreeD,
}

fn default_scene_2d() -> String {
    r#"$root = @main

[main]

[Node2D]
    position = (0, 0)
[/Node2D]
[/main]
"#
    .to_string()
}

fn default_scene_3d() -> String {
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

pub(crate) fn new_scene_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let file_name = sanitize_scene_file_name(&raw_name)?;
    let template = parse_scene_template(args)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let content_root = resolve_content_root(&project_dir, args)?;
    let content_prefix = resolve_content_prefix(args)?;

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &content_root, &content_prefix)?
    } else {
        content_root
    };

    let target_path = target_dir.join(file_name);
    let contents = match template {
        SceneTemplate::TwoD => default_scene_2d(),
        SceneTemplate::ThreeD => default_scene_3d(),
    };
    write_new_file(&target_path, &contents)?;
    println!(
        "created scene at {}",
        normalize_powershell_path(&target_path)
    );
    maybe_open_file_in_editor(args, &target_path)?;
    Ok(())
}

fn default_animation_panim(animation_name: &str) -> String {
    r#"[Animation]
name = "__ANIMATION_NAME__"
fps = 60
default_interp = "interpolate"
default_ease = "linear"
[/Animation]

[Objects]
Target = Node3D
[/Objects]

[Frame0]
@Target {
    position = (0, 0, 0)
}
[/Frame0]

[Frame30]
@Target {
    position = (2, 0, 0)
}
[/Frame30]
"#
    .replace("__ANIMATION_NAME__", animation_name)
}

fn default_panimtree(tree_name: &str) -> String {
    r#"[AnimationTree]
name = "__TREE_NAME__"
[/AnimationTree]

[AnimationSlots]
Idle
Run
[/AnimationSlots]

[MoveBlend]
    [Blend]
        inputs = [@Idle, @Run]
        weights = [1.0, 0.0]
    [/Blend]
[/MoveBlend]

[Output]
    input = @MoveBlend
[/Output]
"#
    .replace("__TREE_NAME__", tree_name)
}

pub(crate) fn new_animation_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let file_name = sanitize_animation_file_name(&raw_name)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let content_root = resolve_content_root(&project_dir, args)?;
    let content_prefix = resolve_content_prefix(args)?;

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &content_root, &content_prefix)?
    } else {
        content_root.join("animations")
    };

    let target_path = target_dir.join(file_name);
    let animation_name = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("NewAnimation");
    write_new_file(&target_path, &default_animation_panim(animation_name))?;
    println!(
        "created animation at {}",
        normalize_powershell_path(&target_path)
    );
    maybe_open_file_in_editor(args, &target_path)?;
    Ok(())
}

pub(crate) fn new_panimtree_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_name) = parse_flag_value(args, "--name") else {
        return Err("missing required flag `--name`".to_string());
    };
    let file_name = sanitize_panimtree_file_name(&raw_name)?;

    let project_dir = if let Some(raw_project) = parse_flag_value(args, "--path") {
        resolve_local_path(&raw_project, cwd)
    } else {
        find_project_root(cwd).ok_or_else(|| {
            "could not find project.toml. Run from a project directory or pass --path <project_dir>."
                .to_string()
        })?
    };
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    let content_root = resolve_content_root(&project_dir, args)?;
    let content_prefix = resolve_content_prefix(args)?;

    let target_dir = if let Some(raw_path) = parse_flag_value(args, "--res") {
        resolve_res_subdir(&raw_path, &content_root, &content_prefix)?
    } else {
        content_root.join("animations")
    };

    let target_path = target_dir.join(file_name);
    let tree_name = target_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("NewAnimationTree");
    write_new_file(&target_path, &default_panimtree(tree_name))?;
    println!(
        "created animation tree at {}",
        normalize_powershell_path(&target_path)
    );
    maybe_open_file_in_editor(args, &target_path)?;
    Ok(())
}
