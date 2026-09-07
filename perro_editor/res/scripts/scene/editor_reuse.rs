use crate::scripts::assets::editor_assets::{open_scene_path, refresh_project_assets};
use crate::scripts::editor::main::{EditorState, cached_scene_doc_shared};
use crate::scripts::scene::editor_nodes::collect_scene_subtree_keys;
use crate::scripts::scene::editor_selection;
use crate::scripts::ui::editor_ui::set_log;
use perro_api::prelude::*;
use perro_api::scene::{SceneDoc, SceneKey};
use std::path::Path;

pub fn branch(doc: &SceneDoc, root: u32) -> Result<SceneDoc, String> {
    let keep = collect_scene_subtree_keys(doc, root);
    if keep.is_empty() {
        return Err("missing branch".into());
    }
    let mut out = doc.clone();
    out.scene
        .nodes
        .to_mut()
        .retain(|n| keep.contains(&n.key.as_u32()));
    out.scene.root = Some(SceneKey::new(root));
    for node in out.scene.nodes.to_mut() {
        if node.key.as_u32() == root {
            node.parent = None;
        }
    }
    out.normalize_links();
    Ok(out)
}

pub fn save_branch<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |s| {
        let keys = editor_selection::keys(s);
        if keys.len() != 1 {
            return Err("select one branch".to_string());
        }
        let doc = cached_scene_doc_shared(&s.doc_text);
        let out = branch(&doc, keys[0])?;
        Ok((
            s.project_root.clone(),
            doc.scene.key_name_or_id(SceneKey::new(keys[0])).to_string(),
            out,
        ))
    })
    .unwrap_or_else(|| Err("missing editor".into()));
    let (root, name, doc) = match request {
        Ok(r) => r,
        Err(e) => {
            set_log(ctx, &e);
            return;
        }
    };
    let Some(folder) = FileMod::pick_folder("Save Branch: choose folder under res") else {
        return;
    };
    let result = (|| -> Result<String, String> {
        let res = Path::new(&root)
            .join("res")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let folder = Path::new(&folder)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !folder.starts_with(&res) {
            return Err("choose folder under project res".into());
        }
        let name = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        for suffix in 0..10_000 {
            let stem = if suffix == 0 {
                name.clone()
            } else {
                format!("{name}_{suffix}")
            };
            let target = folder.join(format!("{stem}.scn"));
            if target.exists() {
                continue;
            }
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)
                .map_err(|e| e.to_string())?;
            use std::io::Write;
            if let Err(err) = file
                .write_all(doc.to_text().as_bytes())
                .and_then(|_| file.sync_all())
            {
                drop(file);
                let _ = std::fs::remove_file(&target);
                return Err(err.to_string());
            }
            return Ok(format!("save branch\n{}", target.display()));
        }
        Err("no free branch name".into())
    })();
    refresh_project_assets(ctx);
    set_log(
        ctx,
        &result.unwrap_or_else(|e| format!("save branch fail\n{e}")),
    );
}

pub fn open_source<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let path = with_state!(ctx.run, EditorState, ctx.id, |s| {
        let key = s.selected_key?;
        cached_scene_doc_shared(&s.doc_text)
            .scene
            .nodes
            .iter()
            .find(|n| n.key.as_u32() == key)?
            .root_of
            .as_ref()
            .map(|s| s.to_string())
    })
    .unwrap_or_default();
    if let Some(path) = path {
        open_scene_path(ctx, &path);
    } else {
        set_log(ctx, "select scene instance");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn branch_round_trip_retains_source() {
        let doc = perro_api::scene::Parser::new("$root = @r\n[r]\n[Node2D]\n[/Node2D]\n[/r]\n[c]\nparent = @r\n[Node2D]\n[/Node2D]\n[/c]\n").try_parse_scene_doc().unwrap();
        let text = doc.to_text();
        let out = branch(&doc, doc.scene.nodes[1].key.as_u32()).unwrap();
        let reload = perro_api::scene::Parser::new(&out.to_text())
            .try_parse_scene_doc()
            .unwrap();
        assert_eq!(reload.scene.nodes.len(), 1);
        assert!(reload.scene.nodes[0].parent.is_none());
        assert_eq!(doc.to_text(), text);
    }
}
