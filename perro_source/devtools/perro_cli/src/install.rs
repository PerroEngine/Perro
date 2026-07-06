use crate::{parse_flag_value, workspace_root};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const PROFILE_SNIPPET_BEGIN: &str = "# >>> perro_cli source-mode >>>";
const PROFILE_SNIPPET_END: &str = "# <<< perro_cli source-mode <<<";

pub(crate) fn install_command(args: &[String]) -> Result<(), String> {
    let explicit_profile = parse_flag_value(args, "--profile").map(PathBuf::from);

    if cfg!(target_os = "windows") {
        let profile_paths = if let Some(path) = explicit_profile {
            vec![path]
        } else {
            default_powershell_profile_paths()
        };
        let workspace_manifest =
            normalize_powershell_path(&workspace_root().join("Cargo.toml")).replace('\\', "\\\\");
        let snippet = format!(
            "{PROFILE_SNIPPET_BEGIN}\n\
function perro {{\n\
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)\n\
    $manifest = \"{workspace_manifest}\"\n\
    cargo build --manifest-path $manifest -p perro_cli\n\
    if ($LASTEXITCODE -ne 0) {{ return }}\n\
    $root = Split-Path -Parent $manifest\n\
    $targetDir = if ($env:CARGO_TARGET_DIR) {{ $env:CARGO_TARGET_DIR }} else {{ Join-Path $root \"target\" }}\n\
    $cliExe = Join-Path $targetDir \"debug\\perro_cli.exe\"\n\
    $runRoot = Join-Path ([System.IO.Path]::GetTempPath()) \"perro_cli_runs\"\n\
    New-Item -ItemType Directory -Force -Path $runRoot | Out-Null\n\
    Get-ChildItem -Path $runRoot -Directory -ErrorAction SilentlyContinue | Where-Object {{ $_.LastWriteTimeUtc -lt (Get-Date).ToUniversalTime().AddDays(-1) }} | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue\n\
    $runDir = Join-Path $runRoot (\"$PID-\" + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())\n\
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null\n\
    $runExe = Join-Path $runDir \"perro_cli.exe\"\n\
    Copy-Item -LiteralPath $cliExe -Destination $runExe -Force\n\
    & $runExe @Args\n\
}}\n\
{PROFILE_SNIPPET_END}\n"
        );
        install_snippet_into_profiles(&profile_paths, &snippet)?;
        if let Some(primary) = profile_paths.first() {
            println!("restart PowerShell or run: . \"{}\"", primary.display());
        }
        return Ok(());
    }

    if cfg!(target_os = "linux") {
        let profile_paths = if let Some(path) = explicit_profile {
            vec![path]
        } else {
            default_posix_profile_paths()
        };
        let workspace_manifest = shell_single_quote_path(&workspace_root().join("Cargo.toml"));
        let snippet = format!(
            "{PROFILE_SNIPPET_BEGIN}\n\
perro() {{\n\
    manifest={workspace_manifest}\n\
    cargo build --manifest-path \"$manifest\" -p perro_cli || return\n\
    root=$(dirname \"$manifest\")\n\
    target_dir=${{CARGO_TARGET_DIR:-$root/target}}\n\
    run_root=${{TMPDIR:-/tmp}}/perro_cli_runs\n\
    mkdir -p \"$run_root\" || return\n\
    find \"$run_root\" -mindepth 1 -maxdepth 1 -type d -mtime +1 -exec rm -rf {{}} + 2>/dev/null\n\
    run_dir=\"$run_root/$$-$(date +%s%3N 2>/dev/null || date +%s)\"\n\
    mkdir -p \"$run_dir\" || return\n\
    cp \"$target_dir/debug/perro_cli\" \"$run_dir/perro_cli\" || return\n\
    \"$run_dir/perro_cli\" \"$@\"\n\
}}\n\
{PROFILE_SNIPPET_END}\n"
        );
        install_snippet_into_profiles(&profile_paths, &snippet)?;
        if let Some(primary) = profile_paths.first() {
            println!("restart shell or run: . \"{}\"", primary.display());
        }
        return Ok(());
    }

    Err(
        "install currently supports Windows PowerShell + Linux POSIX shells only. Use docs snippet manually for this platform."
            .to_string(),
    )
}

fn install_snippet_into_profiles(profile_paths: &[PathBuf], snippet: &str) -> Result<(), String> {
    for profile_path in profile_paths {
        let parent = profile_path.parent().ok_or_else(|| {
            format!(
                "invalid profile path (no parent directory): {}",
                profile_path.display()
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;

        let existing = if profile_path.exists() {
            fs::read_to_string(profile_path)
                .map_err(|err| format!("failed to read {}: {err}", profile_path.display()))?
        } else {
            String::new()
        };

        let updated = replace_or_append_snippet(&existing, snippet)?;
        fs::write(profile_path, updated)
            .map_err(|err| format!("failed to write {}: {err}", profile_path.display()))?;
        println!(
            "installed source-mode command `perro` into {}",
            profile_path.display()
        );
    }
    Ok(())
}

fn default_posix_profile_paths() -> Vec<PathBuf> {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let home = PathBuf::from(home);
    let mut paths = vec![
        home.join(".profile"),
        home.join(".bashrc"),
        home.join(".zshrc"),
    ];
    paths.sort();
    paths.dedup();
    paths
}

fn shell_single_quote_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let escaped = raw.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn default_powershell_profile_paths() -> Vec<PathBuf> {
    let user_profile = env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let docs = PathBuf::from(user_profile).join("Documents");
    let ps7_dir = docs.join("PowerShell");
    let ps5_dir = docs.join("WindowsPowerShell");

    // Install for current host + all hosts in both pwsh (7+) and Windows PowerShell (5.1).
    let mut paths = vec![
        ps7_dir.join("Microsoft.PowerShell_profile.ps1"),
        ps7_dir.join("profile.ps1"),
        ps5_dir.join("Microsoft.PowerShell_profile.ps1"),
        ps5_dir.join("profile.ps1"),
    ];
    paths.sort();
    paths.dedup();
    paths
}

pub(crate) fn normalize_powershell_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(stripped) = raw.strip_prefix("\\\\?\\") {
        stripped.to_string()
    } else {
        raw.to_string()
    }
}

fn replace_or_append_snippet(existing: &str, snippet: &str) -> Result<String, String> {
    let start = existing.find(PROFILE_SNIPPET_BEGIN);
    let end = existing.find(PROFILE_SNIPPET_END);
    match (start, end) {
        (Some(s), Some(e)) if e >= s => {
            let after = e + PROFILE_SNIPPET_END.len();
            let mut out = String::new();
            out.push_str(&existing[..s]);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(snippet);
            let tail = &existing[after..];
            if !tail.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(tail.trim_start_matches('\n'));
            }
            Ok(out)
        }
        (None, None) => {
            let mut out = existing.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(snippet);
            Ok(out)
        }
        _ => Err(
            "profile contains a partial perro_cli snippet; remove it and re-run install"
                .to_string(),
        ),
    }
}
