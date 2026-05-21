use std::env;
use std::path::{Path, PathBuf};

mod bench;
mod doctor;
mod gltf_animation;
mod install;
mod profiling;
mod project;
mod scaffold;
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

    let result = if command == "--help" || command == "-h" || command == "help" {
        print_usage();
        Ok(())
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

fn print_usage() {
    eprintln!("Usage:");
    eprintln!(
        "  perro_cli check [--path <project_dir>]    # scripts-only compile (.perro/scripts)"
    );
    eprintln!(
        "  perro_cli build [--path <project_dir>] [--target native|web|android] [--profile] [--console]    # full static project bundle + build"
    );
    eprintln!(
        "  perro_cli dlc --name <dlc_name> [--path <project_dir>] # build one runtime-loadable DLC package"
    );
    eprintln!(
        "  perro_cli dev [--path <project_dir>] [--target native|web|android] [--profile] [--ui-profile] [--release] [--csv-profile [csv_name]] [--host <addr>] [--port <num>]      # build scripts + run dev runner, web server, or android app"
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
        "  perro_cli import_anim <model.glb|model.gltf> --output <clip.panim> [--clip <name|index>] [--fps <fps>] [--skeleton <object_name>]"
    );
}

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

fn parse_optional_flag_value(args: &[String], flag: &str) -> Option<Option<String>> {
    let idx = args.iter().position(|a| a == flag)?;
    let next = args.get(idx + 1);
    if let Some(val) = next
        && !val.starts_with("--")
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
