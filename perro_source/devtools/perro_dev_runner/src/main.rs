use perro_app::{entry, winit_runner::AppExitKind};
use perro_project::resolve_local_path;
use std::{env, path::PathBuf, process};

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn current_dir_fallback() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let local_root = current_dir_fallback();

    let root = parse_flag_value(&args, "--path")
        .map(|p| resolve_local_path(&p, &local_root))
        .unwrap_or_else(|| local_root.clone());

    let fallback_name =
        parse_flag_value(&args, "--name").unwrap_or_else(|| "Perro Project".to_string());

    eprintln!("perro dev runner: start {}", root.to_string_lossy());
    let run_result = entry::run_dev_project_from_path(&root, &fallback_name);

    match run_result {
        Ok(result) => match result.kind {
            AppExitKind::WindowClose => println!("perro exit: window close"),
            AppExitKind::EventLoopExit => println!("perro exit: event loop exit"),
        },
        Err(err) => {
            eprintln!("perro exit error at `{}`: {err}", root.to_string_lossy());
            process::exit(1);
        }
    }
}
