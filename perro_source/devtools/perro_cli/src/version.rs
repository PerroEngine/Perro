use crate::{log_done, log_note, log_step, parse_flag_value, workspace_root};
use std::env;
use std::fs;
use std::path::Path;

/// Prints the engine version, or sets it with `--set X.Y.Z`.
///
/// The version lives in exactly two shapes in the engine manifest: the literal
/// in `[workspace.package] version` (which every `perro_*` crate inherits via
/// `version.workspace = true`) and the requirement on each `perro_*` entry under
/// `[workspace.dependencies]`. Cargo cannot inherit the latter from the former,
/// so this keeps them in step.
pub(crate) fn version_command(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let manifest = root.join("Cargo.toml");
    let src = fs::read_to_string(&manifest)
        .map_err(|err| format!("failed to read {}: {err}", manifest.display()))?;
    let current = workspace_package_version(&src).ok_or_else(|| {
        format!(
            "could not find `[workspace.package] version` in {}",
            manifest.display()
        )
    })?;

    let Some(requested) = parse_flag_value(args, "--set") else {
        println!("{current}");
        return Ok(());
    };
    let next = validate_semver(&requested)?;
    if next == current {
        log_note(&format!("Engine version already {next}"));
        return Ok(());
    }

    log_step(&format!("Setting Engine Version {current} -> {next}"));
    let (updated, dep_count) = rewrite_workspace_versions(&src, &next);
    write_if_changed(&manifest, &updated)?;
    // A member may pin a sibling directly instead of inheriting (perro_macros
    // does, to keep `default-features = false`). Those requirements are a second
    // place the version lives, so bump them too or resolution breaks.
    let member_count = rewrite_member_manifests(&root.join("perro_source"), &next)?;
    log_done(&format!(
        "Engine Version {next} ({dep_count} workspace + {member_count} member requirement(s) updated)"
    ));
    log_note("Generated project manifests pick this up on the next `perro dev`.");
    Ok(())
}

/// Rewrites direct `perro_* = { version = "..." }` pins inside engine crates.
fn rewrite_member_manifests(engine_src: &Path, next: &str) -> Result<usize, String> {
    let mut manifests = Vec::new();
    collect_manifests(engine_src, &mut manifests);
    let mut count = 0usize;
    for manifest in manifests {
        let Ok(src) = fs::read_to_string(&manifest) else {
            continue;
        };
        let mut out = String::with_capacity(src.len());
        let mut changed = 0usize;
        for line in src.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("perro_")
                && let Some(replaced) = replace_version_requirement(line, next)
            {
                changed += 1;
                out.push_str(&replaced);
                out.push('\n');
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        if changed > 0 && out != src {
            write_if_changed(&manifest, &out)?;
            count += changed;
        }
    }
    Ok(count)
}

fn collect_manifests(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_manifests(&path, out);
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            out.push(path);
        }
    }
}

/// Engine version this CLI was built from. `perro_cli` inherits
/// `[workspace.package] version`, so it is the engine version by construction.
pub(crate) const CLI_ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Escape hatch for a deliberate mismatch.
const VERSION_MISMATCH_OVERRIDE: &str = "PERRO_ALLOW_VERSION_MISMATCH";

/// Fails when the engine the project will link differs from the engine this CLI
/// was built from.
///
/// The CLI generates the script glue; the project links the engine. If those two
/// come from different engine versions the glue can target an API the runtime no
/// longer has, and the failure surfaces as a crash inside the loaded dylib
/// rather than a build error.
///
/// Checks the resolved version in `.perro/Cargo.lock`, not the requirement in
/// the manifest: `version = "0.1.0"` is a caret range, so the manifest cannot
/// say what actually gets linked. No lock yet (first run) means nothing to
/// check.
pub(crate) fn ensure_engine_version_match(project_dir: &Path) -> Result<(), String> {
    if env::var_os(VERSION_MISMATCH_OVERRIDE).is_some() {
        return Ok(());
    }
    let lock = project_dir.join(".perro").join("Cargo.lock");
    let Ok(text) = fs::read_to_string(&lock) else {
        return Ok(());
    };
    let Some(linked) = locked_package_version(&text, "perro_runtime") else {
        return Ok(());
    };
    if linked == CLI_ENGINE_VERSION {
        return Ok(());
    }
    Err(format!(
        "engine version mismatch: this `perro` CLI was built from engine {CLI_ENGINE_VERSION}, \
         but {} resolves perro_runtime {linked}.\n\
         The CLI generates the script glue and the project links the engine, so mismatched \
         versions can crash inside the scripts dylib instead of failing to build.\n\
         Rebuild/reinstall `perro` from engine {linked}, or align the project's engine version \
         with `perro version --set {CLI_ENGINE_VERSION}`.\n\
         Set {VERSION_MISMATCH_OVERRIDE}=1 to proceed anyway.",
        lock.display()
    ))
}

/// Reads `version` for a `[[package]]` entry from a Cargo lockfile.
fn locked_package_version(lock: &str, package: &str) -> Option<String> {
    let needle = format!("name = \"{package}\"");
    let mut in_entry = false;
    for line in lock.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            in_entry = false;
            continue;
        }
        if trimmed == needle {
            in_entry = true;
            continue;
        }
        if in_entry && let Some(rest) = trimmed.strip_prefix("version = ") {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn validate_semver(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let invalid = || format!("invalid version `{trimmed}`. Use MAJOR.MINOR.PATCH, e.g. 1.0.0");

    // Split the pre-release / build suffix off first: `1.0.0-rc.1` carries dots
    // of its own and must not be counted as extra core components.
    let core = trimmed
        .split_once(['-', '+'])
        .map(|(core, _)| core)
        .unwrap_or(trimmed);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 {
        return Err(invalid());
    }
    if parts
        .iter()
        .any(|part| part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(invalid());
    }
    Ok(trimmed.to_string())
}

/// Reads the literal under `[workspace.package]`.
fn workspace_package_version(src: &str) -> Option<String> {
    let mut in_section = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == "[workspace.package]";
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version") {
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                continue;
            };
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Rewrites `[workspace.package] version` and every `perro_*` requirement under
/// `[workspace.dependencies]`. Returns the new text and how many deps changed.
fn rewrite_workspace_versions(src: &str, next: &str) -> (String, usize) {
    let mut out = String::with_capacity(src.len());
    let mut section = String::new();
    let mut dep_count = 0usize;

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed.to_string();
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if section == "[workspace.package]" && trimmed.starts_with("version") {
            let indent = &line[..line.len() - line.trim_start().len()];
            out.push_str(indent);
            out.push_str(&format!("version = \"{next}\""));
            out.push('\n');
            continue;
        }

        if section == "[workspace.dependencies]" && trimmed.starts_with("perro_") {
            if let Some(replaced) = replace_version_requirement(line, next) {
                dep_count += 1;
                out.push_str(&replaced);
                out.push('\n');
                continue;
            }
        }

        out.push_str(line);
        out.push('\n');
    }
    (out, dep_count)
}

/// Swaps the `version = "..."` value inside a single inline dependency spec.
fn replace_version_requirement(line: &str, next: &str) -> Option<String> {
    let key_at = line.find("version")?;
    let after_key = line[key_at + "version".len()..].trim_start();
    if !after_key.starts_with('=') {
        return None;
    }
    let eq_at = key_at + line[key_at..].find('=')?;
    let rest = &line[eq_at + 1..];
    let open_rel = rest.find('"')?;
    let open = eq_at + 1 + open_rel;
    let close_rel = line[open + 1..].find('"')?;
    let close = open + 1 + close_rel;
    Some(format!("{}\"{next}\"{}", &line[..open], &line[close + 1..]))
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    if fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[workspace]
members = ["a"]

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
perro_api = { version = "0.1.0", path = "perro_source/api_modules/perro_api" }
perro_ids = { version = "0.1.0", path = "perro_source/core/perro_ids" }
serde = { version = "1.0.228", features = ["derive"] }
"#;


    const LOCK: &str = r#"version = 4

[[package]]
name = "ahash"
version = "0.8.12"

[[package]]
name = "perro_runtime"
version = "0.1.0"
dependencies = [
 "ahash",
]

[[package]]
name = "perro_ui"
version = "0.1.0"
"#;

    #[test]
    fn reads_locked_version_for_the_right_package() {
        assert_eq!(
            locked_package_version(LOCK, "perro_runtime").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(locked_package_version(LOCK, "ahash").as_deref(), Some("0.8.12"));
        assert_eq!(locked_package_version(LOCK, "not_present"), None);
    }

    #[test]
    fn no_lock_is_not_a_mismatch() {
        let dir = std::env::temp_dir().join("perro_version_guard_nolock");
        std::fs::create_dir_all(&dir).expect("dir");
        assert!(ensure_engine_version_match(&dir).is_ok());
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn mismatched_lock_is_rejected() {
        let dir = std::env::temp_dir().join("perro_version_guard_mismatch");
        let perro = dir.join(".perro");
        std::fs::create_dir_all(&perro).expect("dir");
        std::fs::write(
            perro.join("Cargo.lock"),
            "[[package]]
name = \"perro_runtime\"
version = \"9.9.9\"
",
        )
        .expect("lock");
        let err = ensure_engine_version_match(&dir).expect_err("mismatch must fail");
        assert!(err.contains("engine version mismatch"));
        assert!(err.contains("9.9.9"));
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn matching_lock_passes() {
        let dir = std::env::temp_dir().join("perro_version_guard_match");
        let perro = dir.join(".perro");
        std::fs::create_dir_all(&perro).expect("dir");
        std::fs::write(
            perro.join("Cargo.lock"),
            format!("[[package]]
name = \"perro_runtime\"
version = \"{CLI_ENGINE_VERSION}\"
"),
        )
        .expect("lock");
        assert!(ensure_engine_version_match(&dir).is_ok());
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
    #[test]
    fn reads_workspace_package_version() {
        assert_eq!(workspace_package_version(SAMPLE).as_deref(), Some("0.1.0"));
    }

    #[test]
    fn bumps_package_and_perro_deps_only() {
        let (out, count) = rewrite_workspace_versions(SAMPLE, "1.0.0");
        assert_eq!(count, 2);
        assert!(out.contains("version = \"1.0.0\"\nedition"));
        assert!(out.contains(
            "perro_api = { version = \"1.0.0\", path = \"perro_source/api_modules/perro_api\" }"
        ));
        assert!(out.contains("perro_ids = { version = \"1.0.0\""));
        // Third-party requirements are untouched.
        assert!(out.contains("serde = { version = \"1.0.228\""));
    }

    #[test]
    fn round_trips_when_version_unchanged() {
        let (out, _) = rewrite_workspace_versions(SAMPLE, "0.1.0");
        assert_eq!(out, SAMPLE);
    }

    #[test]
    fn rejects_bad_versions() {
        assert!(validate_semver("1.0").is_err());
        assert!(validate_semver("x.y.z").is_err());
        assert!(validate_semver("1.0.0").is_ok());
        assert!(validate_semver("1.0.0-rc.1").is_ok());
    }
}
