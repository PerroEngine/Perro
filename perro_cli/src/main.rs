use perro_compiler::{compile_project_bundle, compile_scripts, sync_scripts};
use perro_project::{create_new_project, default_script_empty_rs, default_script_example_rs};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_PROJECT_NAME: &str = "Perro Project";

fn main() {
    let args: Vec<String> = env::args().collect();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let Some(command) = args.get(1).map(String::as_str) else {
        print_usage();
        std::process::exit(2);
    };

    let result = if command.starts_with('-') {
        legacy_flag_command(&args, &cwd)
    } else {
        match command {
            "new" => new_command(&args, &cwd),
            "build" => build_command(&args, &cwd),
            "project" => project_command(&args, &cwd),
            "dev" => dev_command(&args, &cwd),
            "run" => run_command(&args, &cwd),
            _ => {
                print_usage();
                Err(format!("unknown command `{command}`"))
            }
        }
    };

    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn legacy_flag_command(args: &[String], cwd: &Path) -> Result<(), String> {
    if args.iter().any(|a| a == "--script-new") {
        return script_new_command(args, cwd);
    }
    if args.iter().any(|a| a == "--script-template") {
        return script_template_command(args, cwd);
    }
    if args.iter().any(|a| a == "--dev") {
        return dev_command(args, cwd);
    }
    if args.iter().any(|a| a == "--scripts") {
        return build_command(args, cwd);
    }
    if args.iter().any(|a| a == "--build") {
        return build_command(args, cwd);
    }
    if args.iter().any(|a| a == "--project") {
        return project_command(args, cwd);
    }
    if args.iter().any(|a| a == "--run") {
        return run_command(args, cwd);
    }
    print_usage();
    Err(format!("unknown command `{}`", args[1]))
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  perro_cli new [--path <parent_dir>] [--name <project_name>]");
    eprintln!("  perro_cli build [--path <project_dir>]   # builds .perro/scripts");
    eprintln!("  perro_cli project [--path <project_dir>] # full static project bundle + build");
    eprintln!("  perro_cli dev [--path <project_dir>] [--name <project_name>]");
    eprintln!("  perro_cli run [--path <project_dir>]     # alias for build scripts");
    eprintln!("  perro_cli --path <project_dir|res|res/scripts> --script-new [<name.rs|res://scripts/name.rs>]");
    eprintln!(
        "  perro_cli --path <project_dir|res|res/scripts> --script-template [<name.rs|res://scripts/name.rs>]"
    );
}

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn parse_optional_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    let value = args.get(idx + 1)?;
    if value.starts_with('-') {
        None
    } else {
        Some(value.clone())
    }
}

fn resolve_local_path(input: &str, local_root: &Path) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("local://") {
        let rel = stripped.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }
    if input.starts_with('/') || input.starts_with('\\') {
        let rel = input.trim_start_matches('/').trim_start_matches('\\');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }
    PathBuf::from(input)
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

fn workspace_root() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    raw.canonicalize().unwrap_or(raw)
}

fn new_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let base_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_name =
        parse_flag_value(args, "--name").unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());
    let project_dir = base_dir.join(sanitize_project_dir_name(&project_name));

    create_new_project(&project_dir, &project_name).map_err(|err| {
        format!(
            "failed to create project at {}: {err}",
            project_dir.display()
        )
    })?;

    println!(
        "created project `{}` at {}",
        project_name,
        project_dir.display()
    );
    Ok(())
}

fn build_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    compile_scripts(&project_dir).map(|_| ()).map_err(|err| {
        format!(
            "scripts pipeline failed for {}: {err}",
            project_dir.display()
        )
    })
}

fn dev_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_name =
        parse_flag_value(args, "--name").unwrap_or_else(|| DEFAULT_PROJECT_NAME.to_string());

    compile_scripts(&project_dir).map(|_| ()).map_err(|err| {
        format!(
            "scripts pipeline failed for {}: {err}",
            project_dir.display()
        )
    })?;

    let root = workspace_root();

    let status = Command::new("cargo")
        .arg("run")
        .arg("-p")
        .arg("perro_dev_runner")
        .arg("--release")
        .arg("--")
        .arg("--path")
        .arg(project_dir.to_string_lossy().to_string())
        .arg("--name")
        .arg(project_name)
        .current_dir(&root)
        .status()
        .map_err(|err| {
            format!(
                "failed to run perro_dev_runner from {}: {err}",
                root.display()
            )
        })?;

    if !status.success() {
        return Err(format!(
            "perro_dev_runner failed with exit code {:?}",
            status.code()
        ));
    }
    Ok(())
}

fn run_command(args: &[String], cwd: &Path) -> Result<(), String> {
    build_command(args, cwd)
}

fn project_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    compile_project_bundle(&project_dir).map_err(|err| {
        format!(
            "project pipeline failed for {}: {err}",
            project_dir.display()
        )
    })
}

fn script_new_command(args: &[String], cwd: &Path) -> Result<(), String> {
    create_script_command(args, cwd, &default_script_empty_rs())
}

fn script_template_command(args: &[String], cwd: &Path) -> Result<(), String> {
    create_script_command(args, cwd, &default_script_example_rs())
}

fn create_script_command(args: &[String], cwd: &Path, contents: &str) -> Result<(), String> {
    let base_path = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let (project_dir, scripts_dir) = resolve_script_roots_for_create(&base_path)?;
    let script_name = parse_optional_flag_value(args, "--script-new")
        .or_else(|| parse_optional_flag_value(args, "--script-template"))
        .unwrap_or_else(|| "script.rs".to_string());
    let rel_script = normalize_script_rel_path(&script_name)?;
    let script_path = scripts_dir.join(&rel_script);

    if script_path.exists() {
        return Err(format!(
            "script already exists at {} (refusing to overwrite)",
            script_path.display()
        ));
    }

    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create script directory {}: {err}",
                parent.display()
            )
        })?;
    }

    fs::write(&script_path, contents).map_err(|err| {
        format!(
            "failed to write script at {}: {err}",
            script_path.display()
        )
    })?;

    sync_scripts(&project_dir).map_err(|err| {
        format!(
            "script created, but failed to sync generated scripts for {}: {err}",
            project_dir.display()
        )
    })?;

    println!("created script at {}", script_path.display());
    Ok(())
}

fn resolve_script_roots_for_create(path: &Path) -> Result<(PathBuf, PathBuf), String> {
    let path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());

    // `--path` can point at project root, `res`, or `res/scripts`.
    if path.join("project.toml").exists() {
        let scripts_dir = path.join("res").join("scripts");
        return Ok((path, scripts_dir));
    }

    if path.file_name().is_some_and(|n| n == "res") {
        let Some(project_dir) = path.parent() else {
            return Err(format!(
                "invalid --path `{}` (could not resolve project root)",
                path.display()
            ));
        };
        if project_dir.join("project.toml").exists() {
            return Ok((project_dir.to_path_buf(), path.join("scripts")));
        }
    }

    if path.file_name().is_some_and(|n| n == "scripts") {
        let Some(res_dir) = path.parent() else {
            return Err(format!(
                "invalid --path `{}` (could not resolve res directory)",
                path.display()
            ));
        };
        if !res_dir.file_name().is_some_and(|n| n == "res") {
            return Err(format!(
                "invalid --path `{}` (expected `res/scripts`)",
                path.display()
            ));
        }
        let Some(project_dir) = res_dir.parent() else {
            return Err(format!(
                "invalid --path `{}` (could not resolve project root)",
                path.display()
            ));
        };
        if project_dir.join("project.toml").exists() {
            return Ok((project_dir.to_path_buf(), path));
        }
    }

    Err(format!(
        "invalid --path `{}` for script creation. Use project root, `res`, or `res/scripts`.",
        path.display()
    ))
}

fn normalize_script_rel_path(input: &str) -> Result<PathBuf, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("script name/path cannot be empty".to_string());
    }

    let mut rel = if let Some(stripped) = trimmed.strip_prefix("res://") {
        stripped.to_string()
    } else if let Some(stripped) = trimmed.strip_prefix("res/") {
        stripped.to_string()
    } else if let Some(stripped) = trimmed.strip_prefix("scripts/") {
        stripped.to_string()
    } else {
        trimmed.to_string()
    };
    rel = rel.replace('\\', "/");

    if rel.starts_with('/') {
        rel = rel.trim_start_matches('/').to_string();
    }
    if rel.contains("..") {
        return Err("script path cannot contain `..` segments".to_string());
    }
    if !rel.ends_with(".rs") {
        rel.push_str(".rs");
    }

    let path = PathBuf::from(rel);
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("script path cannot contain parent-directory segments".to_string());
    }
    Ok(path)
}
