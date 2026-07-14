#[cfg(not(feature = "headless"))]
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
    #[cfg(feature = "headless")]
    {
        if let Err(err) = perro_headless::run_dev_project_from_path(&root, &fallback_name) {
            eprintln!("perro exit error at `{}`: {err}", root.to_string_lossy());
            process::exit(1);
        }
        println!("perro exit: headless stop");
    }

    #[cfg(not(feature = "headless"))]
    let run_result = entry::run_dev_project_from_path(&root, &fallback_name);

    #[cfg(not(feature = "headless"))]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parse_flag_value_reads_value_after_flag() {
        let values = args(&["perro_dev_runner", "--path", "demo", "--name", "Demo"]);

        assert_eq!(parse_flag_value(&values, "--path"), Some("demo".to_owned()));
        assert_eq!(parse_flag_value(&values, "--name"), Some("Demo".to_owned()));
    }

    #[test]
    fn parse_flag_value_returns_none_when_flag_missing_or_value_missing() {
        let missing = args(&["perro_dev_runner", "--path", "demo"]);
        let no_value = args(&["perro_dev_runner", "--path"]);

        assert_eq!(parse_flag_value(&missing, "--name"), None);
        assert_eq!(parse_flag_value(&no_value, "--path"), None);
    }

    #[test]
    fn parse_flag_value_uses_first_flag_occurrence() {
        let values = args(&["perro_dev_runner", "--path", "first", "--path", "second"]);

        assert_eq!(
            parse_flag_value(&values, "--path"),
            Some("first".to_owned())
        );
    }
}
