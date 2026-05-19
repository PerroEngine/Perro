use crate::{
    find_project_root, log_done, log_note, log_step, parse_flag_value, resolve_local_path,
    workspace_root,
};
use perro_compiler::sync_scripts;
use perro_project::{ensure_source_overrides, load_project_toml};
use std::fs;
use std::path::Path;
use std::process::Command;

const BENCH_NAME: &str = "perro_script_bench";

pub(crate) fn bench_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let project_dir = parse_flag_value(args, "--path")
        .map(|p| resolve_local_path(&p, cwd))
        .or_else(|| find_project_root(cwd))
        .unwrap_or_else(|| cwd.to_path_buf());
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    if !project_dir.join("project.toml").exists() {
        return Err(format!(
            "invalid --path `{}` for bench. Use project root (directory containing project.toml).",
            project_dir.display()
        ));
    }

    let filters = parse_multi_flag(args, "--script");
    let methods = parse_multi_flag(args, "--method");
    let vars = parse_multi_flag(args, "--var");
    let bench_args = passthrough_args(args);

    log_step("Syncing Bench Scripts");
    ensure_source_overrides(&project_dir)
        .map_err(|err| format!("failed to refresh source overrides: {err}"))?;
    sync_scripts(&project_dir).map_err(|err| format!("failed to sync scripts: {err}"))?;
    ensure_scripts_bench_manifest(&project_dir)?;
    write_script_bench_harness(&project_dir, &filters, &methods, &vars)?;
    log_done("Bench Scripts Synced");

    log_note("Running Criterion Script Bench");
    let project_cfg = load_project_toml(&project_dir)
        .map_err(|err| format!("failed to load project.toml: {err}"))?;
    let scripts_crate = project_dir.join(".perro").join("scripts");
    let target_dir = project_dir.join("target");
    let mut cmd = Command::new("cargo");
    cmd.arg("bench")
        .arg("--bench")
        .arg(BENCH_NAME)
        .env("CARGO_TARGET_DIR", &target_dir)
        .current_dir(&scripts_crate);
    if project_cfg.steam.enabled {
        cmd.arg("--features").arg("steamworks");
    }
    if bench_args.is_empty() {
        cmd.arg("--").arg("--sample-size").arg("10");
    } else {
        cmd.arg("--");
        cmd.args(bench_args);
    }
    let status = cmd.status().map_err(|err| {
        format!(
            "failed to run cargo bench from {}: {err}",
            scripts_crate.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "cargo bench failed with exit code {:?}",
            status.code()
        ));
    }
    log_done("Script Bench Finished");
    Ok(())
}

fn parse_multi_flag(args: &[String], flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < args.len() {
        if args[i] == flag {
            out.push(args[i + 1].clone());
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

fn passthrough_args(args: &[String]) -> Vec<String> {
    let Some(idx) = args.iter().position(|arg| arg == "--") else {
        return Vec::new();
    };
    args[idx + 1..].to_vec()
}

fn ensure_scripts_bench_manifest(project_dir: &Path) -> Result<(), String> {
    let manifest = project_dir
        .join(".perro")
        .join("scripts")
        .join("Cargo.toml");
    let src = fs::read_to_string(&manifest)
        .map_err(|err| format!("failed to read {}: {err}", manifest.display()))?;
    let mut out = src.clone();
    out = ensure_line_in_table(&out, "dev-dependencies", "criterion", "criterion = \"0.5\"");
    out = ensure_line_in_table(
        &out,
        "dev-dependencies",
        "perro_runtime",
        "perro_runtime = { version = \"0.1.0\", features = [\"bench\"] }",
    );
    if !src.contains(&format!("name = \"{BENCH_NAME}\"")) {
        out.push_str(&format!(
            "\n[[bench]]\nname = \"{BENCH_NAME}\"\nharness = false\n"
        ));
    }
    if out != src {
        fs::write(&manifest, out)
            .map_err(|err| format!("failed to write {}: {err}", manifest.display()))?;
    }
    Ok(())
}

fn ensure_line_in_table(src: &str, table: &str, key: &str, line: &str) -> String {
    let header = format!("[{table}]");
    let lines = src.lines().collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|l| l.trim() == header) else {
        let mut out = src.trim_end().to_string();
        out.push_str(&format!("\n\n{header}\n{line}\n"));
        return out;
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(idx, l)| {
            let t = l.trim();
            (t.starts_with('[') && t.ends_with(']')).then_some(idx)
        })
        .unwrap_or(lines.len());
    if lines[start + 1..end]
        .iter()
        .any(|l| l.trim_start().starts_with(&format!("{key} ")))
    {
        return src.to_string();
    }

    let mut out = String::new();
    for (idx, item) in lines.iter().enumerate() {
        if idx == end {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(item);
        out.push('\n');
    }
    if end == lines.len() {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn write_script_bench_harness(
    project_dir: &Path,
    filters: &[String],
    methods: &[String],
    vars: &[String],
) -> Result<(), String> {
    let bench_dir = project_dir.join(".perro").join("scripts").join("benches");
    fs::create_dir_all(&bench_dir)
        .map_err(|err| format!("failed to create {}: {err}", bench_dir.display()))?;
    let path = bench_dir.join(format!("{BENCH_NAME}.rs"));
    let content = bench_harness_source(filters, methods, vars);
    fs::write(&path, content).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn bench_harness_source(filters: &[String], methods: &[String], vars: &[String]) -> String {
    let filters = rust_str_array(filters);
    let methods = rust_str_array(methods);
    let vars = rust_str_array(vars);
    let workspace = normalize_toml_path(&workspace_root());
    format!(
        r#"use criterion::{{black_box, criterion_group, criterion_main, Criterion}};
use perro_api::prelude::*;
use perro_runtime::Runtime;
use perro_api::runtime_api::sub_apis::NodeAPI;
use perro_api::scripting::ScriptBehavior;
use perro_api::variant::Variant;

const SCRIPT_FILTERS: &[&str] = {filters};
const METHODS: &[&str] = {methods};
const VARS: &[&str] = {vars};

fn keep_script(hash: u64) -> bool {{
    SCRIPT_FILTERS.is_empty()
        || SCRIPT_FILTERS.iter().any(|filter| {{
            filter == &hash.to_string() || filter == &format!("0x{{hash:x}}")
        }})
}}

fn behavior_from_ctor(
    ctor: perro_api::scripting::ScriptConstructor<perro_runtime::RuntimeScriptApi>,
) -> Box<dyn ScriptBehavior<perro_runtime::RuntimeScriptApi>> {{
    let raw = ctor();
    assert!(!raw.is_null(), "script constructor returned null");
    unsafe {{ Box::from_raw(raw) }}
}}

fn bench_script_ctor_state(c: &mut Criterion) {{
    for (hash, ctor) in scripts::SCRIPT_REGISTRY.iter().copied().filter(|(hash, _)| keep_script(*hash)) {{
        c.bench_function(&format!("script/{{hash}}/ctor_state"), |b| {{
            b.iter(|| {{
                let behavior = behavior_from_ctor(ctor);
                let state = behavior.create_state();
                black_box(state);
            }});
        }});
    }}
}}

fn bench_script_lifecycle(c: &mut Criterion) {{
    for (hash, ctor) in scripts::SCRIPT_REGISTRY.iter().copied().filter(|(hash, _)| keep_script(*hash)) {{
        c.bench_function(&format!("script/{{hash}}/lifecycle/update"), |b| {{
            let behavior = behavior_from_ctor(ctor);
            let mut runtime = Runtime::new();
            let id = NodeAPI::create::<Node3D>(&mut runtime);
            b.iter(|| runtime.bench_with_script_context(id, |ctx| behavior.on_update(ctx)));
        }});
        c.bench_function(&format!("script/{{hash}}/lifecycle/fixed_update"), |b| {{
            let behavior = behavior_from_ctor(ctor);
            let mut runtime = Runtime::new();
            let id = NodeAPI::create::<Node3D>(&mut runtime);
            b.iter(|| runtime.bench_with_script_context(id, |ctx| behavior.on_fixed_update(ctx)));
        }});
    }}
}}

fn bench_script_methods(c: &mut Criterion) {{
    for method in METHODS {{
        let method_id = func!(method);
        for (hash, ctor) in scripts::SCRIPT_REGISTRY.iter().copied().filter(|(hash, _)| keep_script(*hash)) {{
            c.bench_function(&format!("script/{{hash}}/method/{{method}}"), |b| {{
                let behavior = behavior_from_ctor(ctor);
                let mut runtime = Runtime::new();
                let id = NodeAPI::create::<Node3D>(&mut runtime);
                b.iter(|| {{
                    runtime.bench_with_script_context(id, |ctx| {{
                        black_box(behavior.call_method(method_id, ctx, &[]));
                    }});
                }});
            }});
        }}
    }}
}}

fn bench_script_vars(c: &mut Criterion) {{
    for var in VARS {{
        let var_id = var!(var);
        for (hash, ctor) in scripts::SCRIPT_REGISTRY.iter().copied().filter(|(hash, _)| keep_script(*hash)) {{
            c.bench_function(&format!("script/{{hash}}/state/get_var/{{var}}"), |b| {{
                let behavior = behavior_from_ctor(ctor);
                let state = behavior.create_state();
                b.iter(|| black_box(behavior.get_var(state.as_ref(), var_id)));
            }});
            c.bench_function(&format!("script/{{hash}}/state/set_var/{{var}}"), |b| {{
                let behavior = behavior_from_ctor(ctor);
                let mut state = behavior.create_state();
                b.iter(|| behavior.set_var(state.as_mut(), var_id, Variant::Null));
            }});
        }}
    }}
}}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets = bench_script_ctor_state, bench_script_lifecycle, bench_script_methods, bench_script_vars
);
criterion_main!(benches);

// Engine workspace: {workspace}
"#
    )
}

fn rust_str_array(values: &[String]) -> String {
    let mut out = String::from("&[");
    for value in values {
        out.push('"');
        for ch in value.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c => out.push(c),
            }
        }
        out.push_str("\",");
    }
    out.push(']');
    out
}

fn normalize_toml_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw.strip_prefix("\\\\?\\").unwrap_or(raw.as_ref());
    stripped.replace('\\', "/")
}
