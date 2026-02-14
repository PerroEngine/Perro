use perro_app::entry;
use perro_compiler::compile_scripts;
use perro_project::{bootstrap_project, create_new_project, resolve_local_path};
use std::{env, path::PathBuf};

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn current_dir_fallback() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cargo run -p perro_cli -- new --path <parent_dir> [--name <project_name>]");
    eprintln!("  cargo run -p perro_cli -- doctor --path <project_dir>");
    eprintln!("  cargo run -p perro_cli -- --path <project_dir> [--name <project_name>] --scripts");
    eprintln!("  cargo run -p perro_cli -- --path <project_dir> [--name <project_name>] --dev");
    eprintln!("  cargo run -p perro_cli -- --path <project_dir> [--name <project_name>] --project");
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let local_root = current_dir_fallback();
    let Some(command) = args.get(1) else {
        print_usage();
        std::process::exit(2);
    };

    match command.as_str() {
        "new" => {
            let base_dir = parse_flag_value(&args, "--path")
                .map(|p| resolve_local_path(&p, &local_root))
                .unwrap_or_else(|| local_root.clone());
            let name =
                parse_flag_value(&args, "--name").unwrap_or_else(|| "Perro Project".to_string());
            let project_dir_name = sanitize_project_dir_name(&name);
            let project_root = base_dir.join(project_dir_name);

            create_new_project(&project_root, &name).unwrap_or_else(|err| {
                panic!(
                    "failed to create project `{}` at `{}`: {err}",
                    name,
                    project_root.to_string_lossy()
                )
            });
            println!(
                "created project `{}` at {}",
                name,
                project_root.to_string_lossy()
            );
        }
        "doctor" => {
            let project_root = parse_flag_value(&args, "--path")
                .map(|p| resolve_local_path(&p, &local_root))
                .unwrap_or_else(|| local_root.clone());
            run_doctor(&project_root);
        }
        _ => run_project_command(&args, &local_root),
    }
}

fn run_project_command(args: &[String], local_root: &std::path::Path) {
    let project_root = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, local_root))
        .unwrap_or_else(|| local_root.to_path_buf());
    let fallback_name =
        parse_flag_value(args, "--name").unwrap_or_else(|| "Perro Project".to_string());

    let has_scripts = args.iter().any(|a| a == "--scripts");
    let has_dev = args.iter().any(|a| a == "--dev");
    let has_project = args.iter().any(|a| a == "--project");

    if has_project {
        eprintln!("--project mode is not implemented yet. Static compilation pipeline comes next.");
        std::process::exit(2);
    }

    if has_scripts {
        bootstrap_project(&project_root, &fallback_name).unwrap_or_else(|err| {
            panic!(
                "failed to bootstrap project `{}`: {err}",
                project_root.to_string_lossy()
            )
        });
        let copied = compile_scripts(&project_root).unwrap_or_else(|err| {
            panic!(
                "failed to compile scripts for `{}`: {err}",
                project_root.to_string_lossy()
            )
        });
        println!(
            "compiled scripts for {} ({} source file(s))",
            project_root.to_string_lossy(),
            copied.len()
        );
    }

    if has_dev {
        if !has_scripts {
            bootstrap_project(&project_root, &fallback_name).unwrap_or_else(|err| {
                panic!(
                    "failed to bootstrap project `{}`: {err}",
                    project_root.to_string_lossy()
                )
            });
            compile_scripts(&project_root).unwrap_or_else(|err| {
                panic!(
                    "failed to compile scripts for `{}`: {err}",
                    project_root.to_string_lossy()
                )
            });
        }
        entry::run_dev_project_from_path(&project_root, &fallback_name).unwrap_or_else(|err| {
            panic!(
                "failed to run project `{}`: {err}",
                project_root.to_string_lossy()
            )
        });
        return;
    }

    if !has_scripts {
        print_usage();
        std::process::exit(2);
    }
}

fn run_doctor(project_root: &std::path::Path) {
    let project_crate = project_root.join(".perro").join("project");
    let scripts_crate = project_root.join(".perro").join("scripts");
    let target_root = project_root.join("target");
    let project_target = project_crate.join("target");
    let scripts_target = scripts_crate.join("target");

    println!("Perro Doctor");
    println!("project_root: {}", project_root.to_string_lossy());
    println!("project_exists: {}", project_root.exists());
    println!("root_target_dir: {}", target_root.to_string_lossy());
    println!("root_target_exists: {}", target_root.exists());

    println!();
    println!("subworkspace: .perro/project");
    println!("path: {}", project_crate.to_string_lossy());
    println!("exists: {}", project_crate.exists());
    println!(
        "cargo_config: {}",
        project_crate
            .join(".cargo")
            .join("config.toml")
            .to_string_lossy()
    );
    println!(
        "own_target_exists: {} (expected false when shared target-dir is active)",
        project_target.exists()
    );

    println!();
    println!("subworkspace: .perro/scripts");
    println!("path: {}", scripts_crate.to_string_lossy());
    println!("exists: {}", scripts_crate.exists());
    println!(
        "cargo_config: {}",
        scripts_crate
            .join(".cargo")
            .join("config.toml")
            .to_string_lossy()
    );
    println!(
        "own_target_exists: {} (expected false when shared target-dir is active)",
        scripts_target.exists()
    );

    println!();
    println!("note: scripts compile also enforces CARGO_TARGET_DIR=<project_root>/target");
}
