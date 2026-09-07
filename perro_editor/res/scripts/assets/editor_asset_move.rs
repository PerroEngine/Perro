use crate::scripts::assets::editor_animation_tool::write_atomic;
use crate::scripts::assets::editor_assets::{renamed_asset_ref, rewrite_asset_refs_in_doc};
use crate::scripts::editor::main::EditorState;
use perro_api::prelude::*;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

struct Job {
    root: String,
    source: String,
    target: String,
    task: std::thread::JoinHandle<Result<MovePlan, String>>,
}
static JOB: OnceLock<Mutex<Option<Job>>> = OnceLock::new();

pub fn start(
    root: String,
    source: String,
    target: String,
    dirty: Vec<String>,
) -> Result<(), String> {
    let mut slot = JOB
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "asset move lock")?;
    if slot.is_some() {
        return Err("asset move already active".into());
    }
    let (r, s, t) = (root.clone(), source.clone(), target.clone());
    let task = std::thread::spawn(move || prepare(Path::new(&r), &s, &t, &dirty));
    *slot = Some(Job {
        root,
        source,
        target,
        task,
    });
    Ok(())
}

pub fn tick<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(slot) = JOB.get() else {
        return;
    };
    let Ok(mut slot) = slot.lock() else {
        return;
    };
    if !slot.as_ref().is_some_and(|j| j.task.is_finished()) {
        return;
    }
    let Some(job) = slot.take() else {
        return;
    };
    drop(slot);
    let same =
        with_state!(ctx.run, EditorState, ctx.id, |s| s.project_root == job.root).unwrap_or(false);
    if !same {
        return;
    }
    let ready = with_state_mut!(ctx.run, EditorState, ctx.id, |s| {
        crate::scripts::assets::editor_assets::asset_move_ready(
            s,
            &job.root,
            &job.source,
            &job.target,
        )
    })
    .unwrap_or(false);
    let result = if ready {
        job.task
            .join()
            .unwrap_or_else(|_| Err("asset scan failed".into()))
            .and_then(execute)
    } else {
        Err("asset move cancelled: save affected scene first".into())
    };
    crate::scripts::assets::editor_assets::finish_asset_move(ctx, job.source, job.target, result);
}

struct Edit {
    path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
}
pub struct MovePlan {
    from: PathBuf,
    to: PathBuf,
    edits: Vec<Edit>,
    source: String,
    target: String,
}

fn local(root: &Path, path: &str) -> Result<PathBuf, String> {
    let tail = path
        .strip_prefix("res://")
        .ok_or("asset path must start with res://")?;
    if tail.is_empty()
        || Path::new(tail)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("invalid asset path".into());
    }
    let base = root.join("res").canonicalize().map_err(|e| e.to_string())?;
    let target = base.join(tail);
    let existing = target
        .ancestors()
        .find(|p| p.exists())
        .ok_or("missing asset parent")?;
    if !existing
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&base)
    {
        return Err("asset path leaves res".into());
    }
    Ok(target)
}

fn walk(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_symlink() {
        return Err(format!("move blocked: linked path {}", path.display()));
    }
    if path.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            walk(&entry.map_err(|e| e.to_string())?.path(), files)?;
        }
    } else {
        files.push(path.to_path_buf());
    }
    Ok(())
}

fn rewrite_toml(value: &mut toml::Value, source: &str, target: &str) -> bool {
    match value {
        toml::Value::String(s) => {
            let next = renamed_asset_ref(s, source, target).or_else(|| {
                s.strip_prefix("res/")
                    .and_then(|tail| renamed_asset_ref(&format!("res://{tail}"), source, target))
                    .map(|s| s.replacen("res://", "res/", 1))
            });
            if let Some(next) = next {
                *s = next;
                true
            } else {
                false
            }
        }
        toml::Value::Array(items) => items.iter_mut().fold(false, |changed, item| {
            rewrite_toml(item, source, target) | changed
        }),
        toml::Value::Table(items) => items.iter_mut().fold(false, |changed, (_, item)| {
            rewrite_toml(item, source, target) | changed
        }),
        _ => false,
    }
}

pub fn prepare(
    root: &Path,
    source: &str,
    target: &str,
    dirty: &[String],
) -> Result<MovePlan, String> {
    let root_path = root.canonicalize().map_err(|e| e.to_string())?;
    let root = root_path.as_path();
    let from = local(root, source)?;
    let to = local(root, target)?;
    if !from.exists() {
        return Err("source missing".into());
    }
    if to.exists() {
        return Err("target already exists".into());
    }
    if to.starts_with(&from) {
        return Err("cannot move folder into itself".into());
    }
    if from.is_dir() != source.ends_with('/') || source.ends_with('/') != target.ends_with('/') {
        return Err("folder paths require trailing slash".into());
    }
    let mut paths = Vec::new();
    walk(&root.join("res"), &mut paths)?;
    walk(&root.join("editor_tools"), &mut paths)?;
    paths.push(root.join("project.toml"));
    let mut edits = Vec::new();
    let mut warnings = Vec::new();
    for path in paths {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if matches!(ext, "gltf" | "glb") {
            let model = gltf::Gltf::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let uris = model
                .document
                .buffers()
                .filter_map(|b| match b.source() {
                    gltf::buffer::Source::Uri(uri) => Some(uri),
                    _ => None,
                })
                .chain(model.document.images().filter_map(|i| match i.source() {
                    gltf::image::Source::Uri { uri, .. } => Some(uri),
                    _ => None,
                }));
            for uri in uris.filter(|uri| !uri.starts_with("data:")) {
                let owner_moves = path.starts_with(&from);
                // Encoded/external URIs need a format-aware manual move.
                let dependency = path.parent().unwrap_or(root).join(uri).canonicalize();
                if uri.contains('%')
                    || uri.contains("://")
                    || dependency.as_ref().is_ok_and(|p| p.starts_with(&from)) != owner_moves
                {
                    warnings.push(format!("{}: relative dependency {uri}", path.display()));
                }
            }
        }
        if ext != "scn" && ext != "toml" {
            if matches!(ext, "rs" | "pmat" | "panim" | "uistyle" | "gltf")
                && let Ok(text) = std::fs::read_to_string(&path)
                && (text.contains(source) || text.contains(&source.replacen("res://", "res/", 1)))
            {
                warnings.push(path.display().to_string());
            }
            continue;
        }
        let before = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = std::str::from_utf8(&before).map_err(|e| e.to_string())?;
        let (changed, after) = if ext == "scn" {
            let mut doc = perro_api::scene::Parser::new(text)
                .try_parse_scene_doc()
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let changed = rewrite_asset_refs_in_doc(&mut doc, source, target);
            (
                changed,
                if changed {
                    doc.to_text().into_bytes()
                } else {
                    Vec::new()
                },
            )
        } else {
            let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
            let mut value = toml::Value::Table(table);
            let changed = rewrite_toml(&mut value, source, target);
            (
                changed,
                if changed {
                    toml::to_string_pretty(&value)
                        .map_err(|e| e.to_string())?
                        .into_bytes()
                } else {
                    Vec::new()
                },
            )
        };
        let res_path = path
            .strip_prefix(root.join("res"))
            .ok()
            .map(|p| format!("res://{}", p.to_string_lossy().replace('\\', "/")));
        if res_path.as_ref().is_some_and(|p| dirty.contains(p))
            && (changed || path.starts_with(&from))
        {
            return Err(format!("save affected scene first: {}", path.display()));
        }
        if changed {
            edits.push(Edit {
                path,
                before,
                after,
            });
        }
    }
    if !warnings.is_empty() {
        return Err(format!("manual refs block move:\n{}", warnings.join("\n")));
    }
    Ok(MovePlan {
        from,
        to,
        edits,
        source: source.to_string(),
        target: target.to_string(),
    })
}

pub fn execute(plan: MovePlan) -> Result<String, String> {
    let MovePlan {
        from,
        to,
        edits,
        source,
        target,
    } = plan;
    if !from.exists() || to.exists() {
        return Err("asset path changed during scan".into());
    }
    for edit in &edits {
        if std::fs::read(&edit.path).map_err(|e| e.to_string())? != edit.before {
            return Err(format!("external change: {}", edit.path.display()));
        }
    }
    let mut applied = 0;
    let result = (|| -> Result<(), String> {
        for edit in &edits {
            write_atomic(&edit.path, &edit.after)?;
            applied += 1;
        }
        std::fs::create_dir_all(to.parent().ok_or("missing target parent")?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&from, &to).map_err(|e| e.to_string())
    })();
    if let Err(mut error) = result {
        for edit in edits[..applied].iter().rev() {
            if let Err(rollback) = write_atomic(&edit.path, &edit.before) {
                error.push_str(&format!("\nrollback {}: {rollback}", edit.path.display()));
            }
        }
        return Err(error);
    }
    Ok(format!(
        "move asset\n{source} -> {target}\nupdate {} reference files",
        edits.len()
    ))
}

pub fn move_asset(
    root: &Path,
    source: &str,
    target: &str,
    dirty: &[String],
) -> Result<String, String> {
    execute(prepare(root, source, target, dirty)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "perro-move-test-{}-{}",
                std::process::id(),
                ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            std::fs::create_dir(root.join("res")).unwrap();
            std::fs::write(root.join("res/a.png"), b"asset").unwrap();
            std::fs::write(root.join("project.toml"), "[project]\nicon='res://a.png'\n").unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn move_publishes_asset_and_config_refs() {
        let f = Fixture::new();
        move_asset(&f.0, "res://a.png", "res://b.png", &[]).unwrap();
        assert!(!f.0.join("res/a.png").exists());
        assert_eq!(std::fs::read(f.0.join("res/b.png")).unwrap(), b"asset");
        assert!(
            std::fs::read_to_string(f.0.join("project.toml"))
                .unwrap()
                .contains("res://b.png")
        );
    }
    #[test]
    fn failed_move_restores_original_reference_bytes() {
        let f = Fixture::new();
        std::fs::write(f.0.join("res/blocked"), b"not a folder").unwrap();
        let before = std::fs::read(f.0.join("project.toml")).unwrap();
        assert!(move_asset(&f.0, "res://a.png", "res://blocked/b.png", &[]).is_err());
        assert_eq!(std::fs::read(f.0.join("project.toml")).unwrap(), before);
        assert!(f.0.join("res/a.png").is_file());
    }
    #[test]
    fn all_nested_scene_values_rewrite() {
        use perro_api::scene::SceneValue;
        use std::borrow::Cow;
        let mut refs = SceneValue::Array(Cow::Owned(vec![
            SceneValue::Str(Cow::Borrowed("res://a.png")),
            SceneValue::Str(Cow::Borrowed("res://a.png:tex[1]")),
        ]));
        assert!(
            crate::scripts::assets::editor_assets::rewrite_asset_refs_in_value(
                &mut refs,
                "res://a.png",
                "res://b.png"
            )
        );
        let SceneValue::Array(refs) = refs else {
            panic!("array")
        };
        assert!(matches!(&refs[0],SceneValue::Str(s) if s=="res://b.png"));
        assert!(matches!(&refs[1],SceneValue::Str(s) if s=="res://b.png:tex[1]"));
    }
    #[test]
    fn nested_refs_and_folder_boundaries() {
        let mut value = toml::Value::Table(
            "refs=['res://old/a.png','res://old/b.png','res://older/c.png']"
                .parse()
                .unwrap(),
        );
        assert!(rewrite_toml(&mut value, "res://old/", "res://new/"));
        assert_eq!(value["refs"][0].as_str(), Some("res://new/a.png"));
        assert_eq!(value["refs"][1].as_str(), Some("res://new/b.png"));
        assert_eq!(value["refs"][2].as_str(), Some("res://older/c.png"));
    }
    #[test]
    fn relative_gltf_dependency_blocks_move() {
        let f = Fixture::new();
        std::fs::write(
            f.0.join("res/model.gltf"),
            br#"{"asset":{"version":"2.0"},"images":[{"uri":"a.png"}]}"#,
        )
        .unwrap();
        let result = move_asset(&f.0, "res://a.png", "res://b.png", &[]);
        assert!(result.unwrap_err().contains("relative dependency"));
        assert!(f.0.join("res/a.png").exists());
    }
    #[test]
    fn changed_reference_file_blocks_staged_commit() {
        let f = Fixture::new();
        let plan = prepare(&f.0, "res://a.png", "res://b.png", &[]).unwrap();
        std::fs::write(f.0.join("project.toml"), "# external edit\n").unwrap();
        assert!(execute(plan).unwrap_err().contains("external change"));
        assert_eq!(
            std::fs::read_to_string(f.0.join("project.toml")).unwrap(),
            "# external edit\n"
        );
        assert!(f.0.join("res/a.png").exists());
    }
}
