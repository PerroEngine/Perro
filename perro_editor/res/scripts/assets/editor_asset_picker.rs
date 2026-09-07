use crate::scripts::editor::main::EditorState;
use crate::scripts::ui::editor_ui::refresh_selection_panels;
use perro_api::prelude::*;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;

struct Job {
    root: String,
    task: JoinHandle<Vec<String>>,
}
static JOB: OnceLock<Mutex<Option<Job>>> = OnceLock::new();

/// Metadata only: no buffer/texture decode, import, or generated asset files.
pub fn subrefs(path: &str, doc: &gltf::Document) -> Vec<String> {
    [
        ("mesh", doc.meshes().count()),
        ("mat", doc.materials().count()),
        ("tex", doc.textures().count()),
        ("rig", doc.skins().count()),
    ]
    .into_iter()
    .flat_map(|(kind, count)| (0..count).map(move |i| format!("{path}:{kind}[{i}]")))
    .collect()
}

pub fn start<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some((root, paths)) = with_state_mut!(ctx.run, EditorState, ctx.id, |s| {
        if !matches!(s.inspector_picker_kind.as_str(), "asset" | "value_asset") {
            return None;
        }
        s.inspector_asset_subrefs.clear();
        Some((
            s.project_root.clone(),
            s.file_paths
                .iter()
                .filter(|p| p.ends_with(".glb") || p.ends_with(".gltf"))
                .cloned()
                .collect::<Vec<_>>(),
        ))
    })
    .flatten() else {
        return;
    };
    let Ok(mut job) = JOB.get_or_init(|| Mutex::new(None)).lock() else {
        return;
    };
    let worker_root = root.clone();
    let task = std::thread::spawn(move || {
        let mut refs = Vec::new();
        for path in paths {
            let Some(rel) = path.strip_prefix("res://") else {
                continue;
            };
            let abs = Path::new(&worker_root).join("res").join(rel);
            if let Ok(gltf) = gltf::Gltf::open(abs) {
                refs.extend(subrefs(&path, &gltf.document));
            }
        }
        refs
    });
    *job = Some(Job { root, task });
}

pub fn tick<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(job) = JOB.get() else {
        return;
    };
    let Ok(mut job) = job.lock() else {
        return;
    };
    if !job.as_ref().is_some_and(|j| j.task.is_finished()) {
        return;
    }
    let Some(done) = job.take() else {
        return;
    };
    drop(job);
    let Ok(refs) = done.task.join() else {
        return;
    };
    let visible = with_state_mut!(ctx.run, EditorState, ctx.id, |s| {
        if s.project_root != done.root {
            return false;
        }
        s.inspector_asset_subrefs = refs;
        s.inspector_picker_open
    })
    .unwrap_or(false);
    if visible {
        refresh_selection_panels(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enumerate_materials_without_loading_buffers() {
        let doc =
            gltf::Gltf::from_slice(br#"{"asset":{"version":"2.0"},"materials":[{},{}]}"#).unwrap();
        assert_eq!(
            subrefs("res://model.glb", &doc.document),
            ["res://model.glb:mat[0]", "res://model.glb:mat[1]"]
        );
    }
}
