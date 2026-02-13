use perro_project::{create_new_project, resolve_local_path};
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
        _ => {
            print_usage();
            std::process::exit(2);
        }
    }
}
