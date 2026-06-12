use perro_api::scene::SceneDoc;
use std::path::Path;

#[derive(Default)]
pub struct SceneDeps {
    pub paths: Vec<String>,
    pub error: Option<String>,
}

pub fn collect_scene_deps(root: &Path, entry_path: &str, entry_text: &str) -> SceneDeps {
    let mut out = SceneDeps::default();
    let mut stack = Vec::new();
    collect_inner(root, entry_path, Some(entry_text), &mut stack, &mut out);
    out.paths.sort();
    out.paths.dedup();
    out
}

fn collect_inner(
    root: &Path,
    path: &str,
    override_text: Option<&str>,
    stack: &mut Vec<String>,
    out: &mut SceneDeps,
) {
    if out.error.is_some() {
        return;
    }
    if stack.iter().any(|item| item == path) {
        out.error = Some(format!("root_of cycle: {}", stack.join(" -> ")));
        return;
    }
    if !out.paths.iter().any(|item| item == path) {
        out.paths.push(path.to_string());
    }

    stack.push(path.to_string());
    let text = match override_text {
        Some(text) => text.to_string(),
        None => match std::fs::read_to_string(res_to_abs(root, path)) {
            Ok(text) => text,
            Err(err) => {
                out.error = Some(format!("{path}: {err}"));
                stack.pop();
                return;
            }
        },
    };
    let doc = SceneDoc::parse(&text);
    for node in doc.scene.nodes.iter() {
        let Some(root_of) = node.root_of.as_deref() else {
            continue;
        };
        if root_of.starts_with("res://") && root_of.ends_with(".scn") {
            collect_inner(root, root_of, None, stack, out);
        }
    }
    stack.pop();
}

fn res_to_abs(root: &Path, res_path: &str) -> std::path::PathBuf {
    root.join("res").join(
        res_path
            .trim_start_matches("res://")
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    )
}
