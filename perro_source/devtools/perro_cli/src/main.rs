use std::env;
use std::path::{Path, PathBuf};

mod bench;
mod doctor;
mod gltf_animation;
mod install;
mod profiling;
mod project;
mod scaffold;
mod script_tests;
mod vscode;

use bench::bench_command;
use doctor::doctor_command;
use gltf_animation::gltf_to_panim_command;
use install::install_command;
use profiling::{flamegraph_command, mem_profile_command};
use project::{
    clean_command, clippy_command, dev_command, dlc_command, format_command, project_command,
    scripts_command,
};
use scaffold::{
    new_animation_command, new_command, new_dlc_command, new_panimtree_command, new_scene_command,
    new_script_command,
};
use script_tests::test_command;

const DEFAULT_PROJECT_NAME: &str = "Perro Project";
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_BLUE: &str = "\x1b[94m";
const COLOR_GREEN: &str = "\x1b[92m";
const COLOR_YELLOW: &str = "\x1b[93m";

fn log_step(label: &str) {
    println!("{COLOR_BLUE}🔧 {label}...{COLOR_RESET}");
}

fn log_done(label: &str) {
    println!("{COLOR_GREEN}✅ {label}{COLOR_RESET}");
}

fn log_note(label: &str) {
    println!("{COLOR_YELLOW}🚀 {label}{COLOR_RESET}");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let Some(command) = args.get(1).map(String::as_str) else {
        print_usage();
        std::process::exit(2);
    };

    let result = if command == "--help"
        || command == "-h"
        || command == "help"
        || command_help_requested(&args)
    {
        print_usage();
        Ok(())
    } else if let Err(err) = validate_command_args(command, &args) {
        Err(err)
    } else {
        match command {
            "new" => new_command(&args, &cwd),
            "new_dlc" => new_dlc_command(&args, &cwd),
            "new_script" => new_script_command(&args, &cwd),
            "new_scene" => new_scene_command(&args, &cwd),
            "new_animation" => new_animation_command(&args, &cwd),
            "new_panimtree" => new_panimtree_command(&args, &cwd),
            "import_anim" | "gltf_to_panim" | "glb_to_panim" => gltf_to_panim_command(&args, &cwd),
            "clean" => clean_command(&args, &cwd),
            "install" => install_command(&args),
            "check" => scripts_command(&args, &cwd),
            "test" => test_command(&args, &cwd),
            "build" => project_command(&args, &cwd),
            "dlc" => dlc_command(&args, &cwd),
            "dev" => dev_command(&args, &cwd),
            "bench" => bench_command(&args, &cwd),
            "doctor" => doctor_command(&args, &cwd),
            "mem-profile" => mem_profile_command(&args, &cwd),
            "flamegraph" => flamegraph_command(&args, &cwd),
            "format" => format_command(&args, &cwd),
            "clippy" => clippy_command(&args, &cwd),
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

#[derive(Clone, Copy)]
enum FlagArity {
    Switch,
    Value,
    OptionalValue,
}

#[derive(Clone, Copy)]
struct FlagSpec {
    name: &'static str,
    arity: FlagArity,
}

const fn switch(name: &'static str) -> FlagSpec {
    FlagSpec {
        name,
        arity: FlagArity::Switch,
    }
}

const fn value(name: &'static str) -> FlagSpec {
    FlagSpec {
        name,
        arity: FlagArity::Value,
    }
}

const fn optional_value(name: &'static str) -> FlagSpec {
    FlagSpec {
        name,
        arity: FlagArity::OptionalValue,
    }
}

const PATH: &[FlagSpec] = &[value("--path")];
const NEW: &[FlagSpec] = &[value("--path"), value("--name")];
const NEW_DLC: &[FlagSpec] = &[value("--path"), value("--name"), switch("--no-open")];
const NEW_SCRIPT: &[FlagSpec] = &[
    value("--path"),
    value("--name"),
    value("--res"),
    value("--dlc"),
    switch("--no-open"),
];
const NEW_SCENE: &[FlagSpec] = &[
    value("--path"),
    value("--name"),
    value("--res"),
    value("--dlc"),
    value("--template"),
    switch("--no-open"),
];
const IMPORT_ANIM: &[FlagSpec] = &[
    value("--input"),
    value("--in"),
    value("--output"),
    value("--out"),
    value("--fps"),
    value("--clip"),
    value("--skeleton"),
    value("--retarget-map"),
    value("--retarget"),
    value("--target-rig"),
];
const INSTALL: &[FlagSpec] = &[value("--profile")];
const BUILD: &[FlagSpec] = &[
    value("--path"),
    value("--target"),
    switch("--profile"),
    switch("--console"),
    switch("--headless"),
    switch("--fresh"),
];
const DLC: &[FlagSpec] = &[value("--name"), value("--path")];
const DEV: &[FlagSpec] = &[
    value("--path"),
    value("--target"),
    switch("--timings"),
    switch("--profile"),
    switch("--ui-profile"),
    switch("--release"),
    optional_value("--csv-profile"),
    value("--host"),
    value("--port"),
    switch("--headless"),
];
const BENCH: &[FlagSpec] = &[
    value("--path"),
    value("--script"),
    value("--method"),
    value("--var"),
];
const MEM_PROFILE: &[FlagSpec] = &[
    value("--path"),
    switch("--release"),
    optional_value("--csv"),
];
const FLAMEGRAPH: &[FlagSpec] = &[value("--path"), switch("--profile"), switch("--root")];
const FORMAT: &[FlagSpec] = &[value("--path"), switch("--dedup")];

fn command_schema(command: &str) -> Option<&'static [FlagSpec]> {
    match command {
        "new" => Some(NEW),
        "new_dlc" => Some(NEW_DLC),
        "new_script" | "new_animation" | "new_panimtree" => Some(NEW_SCRIPT),
        "new_scene" => Some(NEW_SCENE),
        "import_anim" | "gltf_to_panim" | "glb_to_panim" => Some(IMPORT_ANIM),
        "clean" | "check" | "test" | "doctor" | "clippy" => Some(PATH),
        "install" => Some(INSTALL),
        "build" => Some(BUILD),
        "dlc" => Some(DLC),
        "dev" => Some(DEV),
        "bench" => Some(BENCH),
        "mem-profile" => Some(MEM_PROFILE),
        "flamegraph" => Some(FLAMEGRAPH),
        "format" => Some(FORMAT),
        _ => None,
    }
}

fn command_help_requested(args: &[String]) -> bool {
    args.iter()
        .skip(2)
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--help" || arg == "-h")
}

fn validate_command_args(command: &str, args: &[String]) -> Result<(), String> {
    let Some(schema) = command_schema(command) else {
        return Ok(());
    };
    let mut index = 2;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            break;
        }
        if !arg.starts_with('-') {
            index += 1;
            continue;
        }
        let Some(spec) = schema.iter().find(|spec| spec.name == arg) else {
            let valid = schema
                .iter()
                .map(|spec| spec.name)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "unknown flag `{arg}` for `{command}`; valid flags: {valid}"
            ));
        };
        match spec.arity {
            FlagArity::Switch => index += 1,
            FlagArity::Value => {
                let next = args.get(index + 1);
                if next.is_none_or(|value| value == "--" || value.starts_with('-')) {
                    return Err(format!(
                        "missing value for flag `{}` in `{command}`",
                        spec.name
                    ));
                }
                index += 2;
            }
            FlagArity::OptionalValue => {
                index += 1;
                if args
                    .get(index)
                    .is_some_and(|value| value != "--" && !value.starts_with('-'))
                {
                    index += 1;
                }
            }
        }
    }
    Ok(())
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!(
        "  perro_cli check [--path <project_dir>]    # scripts-only compile (.perro/scripts)"
    );
    eprintln!(
        "  perro_cli test [--path <project_dir>] [-- <cargo_test_args>]    # sync scripts + run cargo test for .perro/scripts"
    );
    eprintln!(
        "  perro_cli build [--path <project_dir>] [--target native|web|android] [--profile] [--console] [--headless] [--fresh]    # full static project bundle + build (--fresh drops pipeline caches)"
    );
    eprintln!(
        "  perro_cli dlc --name <dlc_name> [--path <project_dir>] # build one runtime-loadable DLC package"
    );
    eprintln!(
        "  perro_cli dev [--path <project_dir>] [--target native|web|android] [--headless] [--timings] [--profile] [--ui-profile] [--release] [--csv-profile [csv_name]] [--host <addr>] [--port <num>]      # build scripts + run dev runner, web server, or android app"
    );
    eprintln!(
        "  perro_cli bench [--path <project_dir>] [--script <hash>] [--method <name>] [--var <name>] [-- <criterion_args>]    # criterion bench scripts"
    );
    eprintln!(
        "  perro_cli mem-profile [--path <project_dir>] [--release] [--csv [csv_name]]    # run dev runner + process memory samples"
    );
    eprintln!(
        "  perro_cli flamegraph [--path <project_dir>] [--profile] [--root]    # run cargo flamegraph for dev runner (auto-installs tool if missing)"
    );
    eprintln!(
        "  perro_cli doctor [--path <project_dir>]   # scene/resource/script reference checks"
    );
    eprintln!(
        "  perro_cli format [--path <project_dir>] [--dedup]   # format .rs, .scn, .fur, .pmat, .ppart, .uistyle under project res"
    );
    eprintln!(
        "  perro_cli clippy [--path <project_dir>]   # cargo clippy for .rs under project res"
    );
    eprintln!("  perro_cli clean [--path <project_dir>]    # remove project target/");
    eprintln!(
        "  perro_cli install                          # add `perro` source-mode command in shell profile"
    );
    eprintln!("  perro_cli new [--path <parent_dir>] [--name <project_name>]");
    eprintln!("  perro_cli new_dlc --name <dlc_name> [--path <project_dir>]");
    eprintln!(
        "  perro_cli new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>]"
    );
    eprintln!(
        "  perro_cli new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--template 2D|3D]"
    );
    eprintln!(
        "  perro_cli new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>]"
    );
    eprintln!(
        "  perro_cli new_panimtree --name <tree_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>]"
    );
    eprintln!(
        "  perro_cli import_anim <model.glb|model.gltf> --output <clip.panim> [--clip <name|index>] [--fps <fps>] [--skeleton <object_name>] [--retarget-map <map.pretarget>] [--target-rig <rig.glb|rig.gltf>]"
    );
}

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1)
        .filter(|value| value.as_str() != "--" && !value.starts_with('-'))
        .cloned()
}

fn parse_optional_flag_value(args: &[String], flag: &str) -> Option<Option<String>> {
    let idx = args.iter().position(|a| a == flag)?;
    let next = args.get(idx + 1);
    if let Some(val) = next
        && val != "--"
        && !val.starts_with('-')
    {
        return Some(Some(val.clone()));
    }
    Some(None)
}

fn resolve_local_path(input: &str, local_root: &Path) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("local://") {
        let rel = stripped.trim_start_matches('/');
        if rel.is_empty() {
            return local_root.to_path_buf();
        }
        return local_root.join(rel);
    }
    #[cfg(not(target_os = "windows"))]
    if input.starts_with('/') {
        return PathBuf::from(input);
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

fn workspace_root() -> PathBuf {
    let raw = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");
    raw.canonicalize().unwrap_or(raw)
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        if ancestor.join("project.toml").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

#[cfg(test)]
mod cli_arg_tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn required_flag_value_does_not_consume_next_flag() {
        let args = args(&["perro", "dev", "--path", "--release"]);

        let err = validate_command_args("dev", &args).unwrap_err();

        assert_eq!(err, "missing value for flag `--path` in `dev`");
        assert_eq!(parse_flag_value(&args, "--path"), None);
    }

    #[test]
    fn unknown_flag_reports_command_and_valid_schema() {
        let args = args(&["perro", "clean", "--pth", "project"]);

        let err = validate_command_args("clean", &args).unwrap_err();

        assert!(err.contains("unknown flag `--pth` for `clean`"));
        assert!(err.contains("valid flags: --path"));
    }

    #[test]
    fn optional_value_does_not_consume_switch() {
        let args = args(&["perro", "dev", "--csv-profile", "--release"]);

        assert_eq!(validate_command_args("dev", &args), Ok(()));
        assert_eq!(
            parse_optional_flag_value(&args, "--csv-profile"),
            Some(None)
        );
    }

    #[test]
    fn passthrough_flags_skip_command_validation() {
        let args = args(&["perro", "test", "--path", "game", "--", "--nocapture"]);

        assert_eq!(validate_command_args("test", &args), Ok(()));
    }
}
