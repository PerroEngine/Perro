use perro_app::{entry, winit_runner::WinitRunner};
use perro_graphics::PerroGraphics;
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

    let root = parse_flag_value(&args, "--path")
        .map(PathBuf::from)
        .unwrap_or_else(current_dir_fallback);

    let name = parse_flag_value(&args, "--name").unwrap_or_else(|| "Perro Project".to_string());

    let mut project = RuntimeProject::new(name.clone(), root);

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
    app.set_debug_draw_rect(true);

    WinitRunner::new().run(app, &name);
}
