use crate::{log_done, log_note, log_step, parse_flag_value, workspace_root};
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
