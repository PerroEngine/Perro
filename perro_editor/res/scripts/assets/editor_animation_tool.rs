use crate::scripts::editor::main::EditorState;
use crate::scripts::ui::editor_ui::{
    read_text_box, refresh_status, set_log, set_text_box, set_ui_display,
};
use perro_api::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;

type Job = (String, JoinHandle<Result<String, String>>);
static JOB: OnceLock<Mutex<Option<Job>>> = OnceLock::new();
const FIELDS: &[&str] = &[
    "input",
    "output",
    "clip",
    "fps",
    "skeleton",
    "retarget_map",
    "target_rig",
];

fn project<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> String {
    with_state!(ctx.run, EditorState, ctx.id, |s| s.project_root.clone()).unwrap_or_default()
}

pub fn open<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let source = with_state!(ctx.run, EditorState, ctx.id, |s| s
        .active_asset_path
        .replace("res://", "res/"))
    .unwrap_or_default();
    set_text_box(ctx, "tool_input", &source);
    for (key, value) in [
        ("output", "res/animations/clip.panim"),
        ("clip", "0"),
        ("fps", "60"),
        ("skeleton", "Rig"),
        ("retarget_map", ""),
        ("target_rig", ""),
        ("options", "editor_tools/animation.toml"),
    ] {
        set_text_box(ctx, &format!("tool_{key}"), value);
    }
    let cli = FileMod::load_string("user://editor_cli.txt")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(default_cli);
    set_text_box(ctx, "tool_cli", &cli);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |s| {
        s.animation_tool_open = true;
        s.focused_inspector_box.clear();
    });
    set_ui_display(ctx, "animation_tools", true);
}

fn default_cli() -> String {
    let name = if cfg!(windows) {
        "perro_cli.exe"
    } else {
        "perro_cli"
    };
    if let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join(name)))
        && path.is_file()
    {
        return path.to_string_lossy().into_owned();
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for name in [name, if cfg!(windows) { "perro.exe" } else { "perro" }] {
            let path = dir.join(name);
            if path.is_file() {
                return path.to_string_lossy().into_owned();
            }
        }
    }
    String::new()
}

fn options_path(root: &str, path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if !path.starts_with("editor_tools")
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("save options under editor_tools/".into());
    }
    let base = Path::new(root).canonicalize().map_err(|e| e.to_string())?;
    let target = base.join(path);
    let ancestor = target
        .ancestors()
        .find(|p| p.exists())
        .ok_or("missing project directory")?;
    if !ancestor
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&base)
    {
        return Err("options path leaves project".into());
    }
    Ok(target)
}

pub fn action<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, action: &str) {
    if action == "tool_close" {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |s| s.animation_tool_open =
            false);
        set_ui_display(ctx, "animation_tools", false);
        return;
    }
    if action == "tool_pick_cli" {
        if let Some(path) = FileMod::pick_file("Choose Perro CLI", &[]) {
            set_text_box(ctx, "tool_cli", &path);
            if let Err(err) = FileMod::save_string("user://editor_cli.txt", &path) {
                set_log(ctx, &err.to_string());
            }
        }
        return;
    }
    let root = project(ctx);
    if root.is_empty() {
        set_log(ctx, "open project first");
        return;
    }
    let result = (|| -> Result<String, String> {
        let path = options_path(
            &root,
            &read_text_box(ctx, "tool_options").unwrap_or_default(),
        )?;
        if action == "tool_load" {
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
            if table.get("version").and_then(toml::Value::as_integer) != Some(1) {
                return Err("unsupported options version".into());
            }
            for key in FIELDS {
                let value = table
                    .get(*key)
                    .map(|v| {
                        v.as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_default();
                set_text_box(ctx, &format!("tool_{key}"), &value);
            }
            return Ok(format!("load options\n{}", path.display()));
        }
        let mut table = toml::Table::new();
        table.insert("version".into(), toml::Value::Integer(1));
        for key in FIELDS {
            let value = read_text_box(ctx, &format!("tool_{key}")).unwrap_or_default();
            if !value.trim().is_empty() {
                table.insert((*key).into(), toml::Value::String(value));
            }
        }
        if !table.contains_key("input") || !table.contains_key("output") {
            return Err("input + output required".into());
        }
        let text = toml::to_string_pretty(&table).map_err(|e| e.to_string())?;
        if action == "tool_save" {
            write_atomic(&path, text.as_bytes())?;
            return Ok(format!("save options\n{}", path.display()));
        }
        let cli = read_text_box(ctx, "tool_cli").unwrap_or_default();
        if !Path::new(&cli).is_file() {
            return Err("choose Perro CLI executable".into());
        }
        let mut slot = JOB
            .get_or_init(|| Mutex::new(None))
            .lock()
            .map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Err("animation conversion already active".into());
        }
        let mut args = vec!["import_anim".to_string()];
        for key in FIELDS {
            if let Some(value) = table.get(*key).and_then(toml::Value::as_str) {
                args.extend([
                    format!("--{}", key.replace('_', "-")),
                    value.replace("res://", "res/"),
                ]);
            }
        }
        let job_root = root.clone();
        let job = std::thread::spawn(move || {
            let mut command = std::process::Command::new(cli);
            command.args(args).current_dir(&job_root);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            let output = command.output().map_err(|e| e.to_string())?;
            let log = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success() {
                Ok(log)
            } else {
                Err(log)
            }
        });
        *slot = Some((root.clone(), job));
        Ok("convert animation\njob active".into())
    })();
    match result {
        Ok(log) => set_log(ctx, &log),
        Err(err) => set_log(ctx, &format!("animation tool fail\n{err}")),
    }
    refresh_status(ctx);
}

pub fn tick<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(slot) = JOB.get() else {
        return;
    };
    let Ok(mut slot) = slot.lock() else {
        return;
    };
    if !slot.as_ref().is_some_and(|(_, job)| job.is_finished()) {
        return;
    }
    let Some((root, job)) = slot.take() else {
        return;
    };
    drop(slot);
    let result = job
        .join()
        .unwrap_or_else(|_| Err("conversion worker failed".into()));
    if root != project(ctx) {
        return;
    }
    match result {
        Ok(log) => set_log(ctx, &log),
        Err(err) => set_log(ctx, &format!("conversion fail\n{err}")),
    }
    refresh_status(ctx);
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let parent = path.parent().ok_or("missing parent directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(
        ".perro-write-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
