use perro_app::{entry, winit_runner::WinitRunner};
use perro_graphics::PerroGraphics;
use perro_project::resolve_local_path;
use perro_runtime::RuntimeProject;
use std::{env, path::PathBuf};

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

    let mut project = RuntimeProject::from_project_dir_with_default_name(&root, &fallback_name)
        .unwrap_or_else(|err| {
            panic!(
                "failed to load project at `{}`: {err}",
                root.to_string_lossy()
            )
        });
    let window_title = project.config.name.clone();

    // Minimal runtime params passthrough: --param key=value (repeatable)
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--param" {
            if let Some(pair) = args.get(i + 1) {
                if let Some((k, v)) = pair.split_once('=') {
                    project = project.with_param(k.to_string(), v.to_string());
                }
            }
        }
        i += 1;
    }

    let graphics = PerroGraphics::new();
    let mut app = entry::create_dev_app(graphics, project);
    app.set_debug_draw_rect(false);
    app.set_debug_draw_mesh(true);

    WinitRunner::new().run(app, &window_title);
}
