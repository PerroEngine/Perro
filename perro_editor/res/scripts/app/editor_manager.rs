use std::path::{Path, PathBuf};

pub fn project_name(text: &str) -> Option<String> {
    let cfg = text.parse::<toml::Table>().ok()?;
    let name = cfg.get("project")?.get("name")?.as_str()?.trim();
    (!name.is_empty()).then_some(name.to_string())
}

pub fn project_main_scene(text: &str) -> Option<String> {
    let cfg = text.parse::<toml::Table>().ok()?;
    let scene = cfg.get("project")?.get("main_scene")?.as_str()?.trim();
    if scene.is_empty() {
        return None;
    }
    if scene.starts_with("res://") {
        Some(scene.to_string())
    } else {
        Some(format!("res://{}", scene.trim_start_matches('/')))
    }
}

pub fn canonical_project_root(root: &Path) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|err| format!("project folder unreadable: {err}"))?;
    validate_project_root(&root)?;
    Ok(root)
}

pub fn validate_project_root(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err("project folder missing".to_string());
    }
    if !root.join("project.toml").is_file() {
        return Err("missing project.toml".to_string());
    }
    Ok(())
}

pub fn project_switch_blocked(current_root: &str, dirty: bool) -> bool {
    dirty && !current_root.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_main_scene_parse_toml() {
        assert_eq!(
            project_main_scene("[project]\nmain_scene = 'levels/start.scn'\n"),
            Some("res://levels/start.scn".to_string())
        );
    }

    #[test]
    fn project_main_scene_ignore_other_tables() {
        assert_eq!(
            project_main_scene("[other]\nmain_scene = 'wrong.scn'\n"),
            None
        );
    }

    #[test]
    fn project_name_parse_comments_and_quotes() {
        assert_eq!(
            project_name("[project]\nname = 'Demo' # label\n"),
            Some("Demo".to_string())
        );
    }

    #[test]
    fn dirty_project_open_stays_blocked() {
        assert!(project_switch_blocked("C:/one", true));
        assert!(!project_switch_blocked("C:/one", false));
        assert!(!project_switch_blocked("", true));
    }
}
