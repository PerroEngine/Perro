pub fn project_main_scene(text: &str) -> Option<String> {
    let mut in_project = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[project]" {
            in_project = true;
            continue;
        }
        if in_project && trimmed.starts_with('[') {
            return None;
        }
        if in_project && trimmed.starts_with("main_scene") {
            let (_, value) = trimmed.split_once('=')?;
            let value = value.trim().trim_matches('"').to_string();
            return (!value.is_empty()).then_some(value);
        }
    }
    None
}
