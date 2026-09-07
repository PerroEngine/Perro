use std::path::Path;

/// Expand portable tool options before CLI flags (the parser uses the first match).
pub(crate) fn expand(args: &[String], cwd: &Path) -> Result<Vec<String>, String> {
    let Some(path) = crate::parse_flag_value(args, "--options") else {
        return Ok(args.to_vec());
    };
    let text = std::fs::read_to_string(crate::resolve_local_path(&path, cwd))
        .map_err(|e| format!("read animation options: {e}"))?;
    expand_text(args, &text)
}

fn expand_text(args: &[String], text: &str) -> Result<Vec<String>, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("animation options: {e}"))?;
    if table.get("version").and_then(toml::Value::as_integer) != Some(1) {
        return Err("animation options need version = 1".into());
    }
    let mut out = args.to_vec();
    for (key, value) in table {
        if key == "version" {
            continue;
        }
        let (flag, aliases): (&str, &[&str]) = match key.as_str() {
            "input" => ("--input", &["--input", "--in"]),
            "output" => ("--output", &["--output", "--out"]),
            "clip" => ("--clip", &["--clip"]),
            "fps" => ("--fps", &["--fps"]),
            "skeleton" => ("--skeleton", &["--skeleton"]),
            "retarget_map" => ("--retarget-map", &["--retarget-map", "--retarget"]),
            "target_rig" => ("--target-rig", &["--target-rig"]),
            _ => return Err(format!("unknown animation option `{key}`")),
        };
        let value = match value {
            toml::Value::String(s) => s,
            toml::Value::Integer(n) if key == "clip" || key == "fps" => n.to_string(),
            toml::Value::Float(n) if key == "fps" => n.to_string(),
            _ => return Err(format!("invalid animation option `{key}`")),
        };
        if aliases
            .iter()
            .any(|alias| crate::parse_flag_value(args, alias).is_some())
            || (key == "input" && args.get(2).is_some_and(|s| !s.starts_with("--")))
        {
            continue;
        }
        out.extend([flag.to_string(), value]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_flags_and_positional_input_win() {
        let args = [
            "perro",
            "import_anim",
            "hero.glb",
            "--fps",
            "30",
            "--out",
            "new.panim",
        ]
        .map(str::to_string);
        let out = expand_text(
            &args,
            "version=1\ninput='old.glb'\noutput='old.panim'\nfps=60\nclip='Walk'",
        )
        .expect("valid animation options");
        assert_eq!(
            crate::parse_flag_value(&out, "--fps").as_deref(),
            Some("30")
        );
        assert!(crate::parse_flag_value(&out, "--input").is_none());
        assert!(crate::parse_flag_value(&out, "--output").is_none());
        assert_eq!(
            crate::parse_flag_value(&out, "--clip").as_deref(),
            Some("Walk")
        );
    }
    #[test]
    fn reject_unknown_version_key_and_type() {
        for text in ["version=2", "version=1\nwat=1", "version=1\nfps=[]"] {
            assert!(expand_text(&[], text).is_err());
        }
    }
}
