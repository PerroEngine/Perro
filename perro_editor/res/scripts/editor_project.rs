use std::path::Path;

pub fn create_project(parent_dir: &str, project_name: &str) -> Result<String, String> {
    let parent = Path::new(parent_dir);
    if !parent.is_dir() {
        return Err("project parent dir missing".to_string());
    }

    let project_name = project_name.trim();
    if project_name.is_empty() {
        return Err("project name empty".to_string());
    }

    let parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
    let project_root = parent.join(sanitize_project_dir_name(project_name));
    perro_project::create_new_project(&project_root, project_name)
        .map_err(|err| format!("failed to scaffold project: {err}"))?;
    Ok(project_root.to_string_lossy().to_string())
}

fn sanitize_project_dir_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "perro_project".to_string();
    }

    let mut out = String::with_capacity(trimmed.len());
    for c in trimmed.chars() {
        let invalid = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if invalid {
            out.push('_');
        } else {
            out.push(c);
        }
    }

    let collapsed = out.trim_matches('.');
    if collapsed.is_empty() {
        "perro_project".to_string()
    } else {
        collapsed.to_string()
    }
}
