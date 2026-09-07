use crate::scripts::editor::main::{EditorState, cached_scene_doc, set_state_scene_doc};
use crate::scripts::scene::editor_nodes::collect_scene_subtree_keys;
use crate::scripts::scene::editor_selection::{self as selection};
use crate::scripts::ui::editor_ui::unique_node_name;
use perro_api::scene::{SceneDoc, SceneKey};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

pub fn commit(state: &mut EditorState, doc: &SceneDoc, selected: Vec<u32>) -> bool {
    if !set_state_scene_doc(state, doc) {
        return false;
    }
    selection::replace(state, selected);
    state.dirty = true;
    if let Some(path) = state.open_paths.get(state.active_open)
        && !state.dirty_scene_paths.contains(path)
    {
        state.dirty_scene_paths.push(path.clone());
    }
    true
}

pub fn delete(state: &mut EditorState) -> Result<bool, String> {
    let mut doc = cached_scene_doc(&state.doc_text);
    let roots = selection::roots(&doc, &selection::keys(state));
    if roots.is_empty() {
        return Err("select nodes".into());
    }
    if doc.scene.root.is_some_and(|r| roots.contains(&r.as_u32())) {
        return Err("cannot delete scene root".into());
    }
    let remove: HashSet<_> = roots
        .iter()
        .flat_map(|k| collect_scene_subtree_keys(&doc, *k))
        .collect();
    let parent = doc
        .scene
        .nodes
        .iter()
        .find(|n| n.key.as_u32() == roots[0])
        .and_then(|n| n.parent);
    doc.scene
        .nodes
        .to_mut()
        .retain(|n| !remove.contains(&n.key.as_u32()));
    doc.normalize_links();
    state.collapsed_scene_keys.retain(|k| !remove.contains(k));
    Ok(commit(
        state,
        &doc,
        parent.into_iter().map(SceneKey::as_u32).collect(),
    ))
}

pub fn copy(state: &mut EditorState) -> Result<(), String> {
    let doc = cached_scene_doc(&state.doc_text);
    let roots = selection::roots(&doc, &selection::keys(state));
    if roots.is_empty() {
        return Err("select nodes".into());
    }
    state.copied_scene_text = doc.to_text();
    state.copied_scene_keys = roots;
    Ok(())
}

pub fn clone_nodes(
    target: &mut SceneDoc,
    source: &SceneDoc,
    roots: &[u32],
    parent: Option<SceneKey>,
    paste: bool,
) -> Vec<u32> {
    let mut map = HashMap::new();
    let mut clones = Vec::new();
    for key in roots
        .iter()
        .flat_map(|k| collect_scene_subtree_keys(source, *k))
    {
        if map.contains_key(&key) {
            continue;
        }
        let Some(node) = source.scene.nodes.iter().find(|n| n.key.as_u32() == key) else {
            continue;
        };
        let next = target.scene.key_names.len() as u32;
        let name = unique_node_name(
            target,
            &format!("{}_copy", source.scene.key_name_or_id(node.key)),
        );
        target.scene.key_names.to_mut().push(Cow::Owned(name));
        map.insert(key, next);
        clones.push(node.clone());
    }
    let names = map
        .iter()
        .map(|(old, new)| {
            (
                source.scene.key_name_or_id(SceneKey::new(*old)).to_string(),
                target.scene.key_name_or_id(SceneKey::new(*new)).to_string(),
            )
        })
        .collect::<Vec<_>>();
    for mut node in clones {
        let old = node.key.as_u32();
        node.key = SceneKey::new(map[&old]);
        node.name = None;
        node.parent = if roots.contains(&old) && paste {
            parent
        } else {
            node.parent.map(|p| {
                map.get(&p.as_u32())
                    .copied()
                    .map(SceneKey::new)
                    .unwrap_or(p)
            })
        };
        // A duplicate of the scene root becomes its child, not a second root.
        if node.parent.is_none() {
            node.parent = target.scene.root;
        }
        // Rewrite only references within this copied forest; leave external refs intact.
        remap_value_refs_in_data(&mut node.data, &names);
        for (_, value) in node.script_vars.to_mut() {
            remap_value_refs(value, &names);
        }
        node.children = Cow::Owned(Vec::new());
        target.scene.nodes.to_mut().push(node);
    }
    target.normalize_links();
    roots.iter().filter_map(|k| map.get(k).copied()).collect()
}

fn remap_value_refs_in_data(
    data: &mut perro_api::scene::SceneNodeData,
    names: &[(String, String)],
) {
    for (_, value) in data.fields.to_mut() {
        remap_value_refs(value, names);
    }
    if let Some(perro_api::scene::SceneNodeDataBase::Owned(base)) = data.base.as_mut() {
        remap_value_refs_in_data(base, names);
    }
}
fn remap_value_refs(value: &mut perro_api::scene::SceneValue, names: &[(String, String)]) {
    use perro_api::scene::{SceneValue, SceneValueKey};
    match value {
        SceneValue::Key(key) => {
            let raw: &str = key.as_ref();
            let prefix = raw.starts_with('@');
            let bare = raw.strip_prefix('@').unwrap_or(raw);
            if let Some((_, next)) = names.iter().find(|(old, _)| old == bare) {
                *key = SceneValueKey::from(format!("{}{next}", if prefix { "@" } else { "" }));
            }
        }
        SceneValue::Array(values) => {
            for value in values.to_mut() {
                remap_value_refs(value, names);
            }
        }
        SceneValue::Object(fields) => {
            for (_, value) in fields.to_mut() {
                remap_value_refs(value, names);
            }
        }
        _ => {}
    }
}

pub fn duplicate(state: &mut EditorState, paste: bool) -> Result<bool, String> {
    let mut doc = cached_scene_doc(&state.doc_text);
    if doc.scene.root.is_none() {
        return Err("open scene first".into());
    }
    let source = if paste {
        cached_scene_doc(&state.copied_scene_text)
    } else {
        doc.clone()
    };
    let roots = if paste {
        state.copied_scene_keys.clone()
    } else {
        selection::roots(&source, &selection::keys(state))
    };
    if roots.is_empty() {
        return Err("copy/select nodes first".into());
    }
    let parent = state.selected_key.map(SceneKey::new).or(doc.scene.root);
    let selected = clone_nodes(&mut doc, &source, &roots, parent, paste);
    Ok(commit(state, &doc, selected))
}

pub fn reparent(state: &mut EditorState, dir: isize) -> Result<bool, String> {
    let mut doc = cached_scene_doc(&state.doc_text);
    let selected = selection::keys(state);
    let roots = selection::roots(&doc, &selected);
    let Some(first) = roots.first() else {
        return Err("select nodes".into());
    };
    if doc.scene.root.is_some_and(|r| roots.contains(&r.as_u32())) {
        return Err("cannot reparent scene root".into());
    }
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|n| n.key.as_u32() == *first)
        .ok_or("missing node")?;
    let parent = node.parent;
    if roots.iter().any(|k| {
        doc.scene
            .nodes
            .iter()
            .any(|n| n.key.as_u32() == *k && n.parent != parent)
    }) {
        return Err("select siblings for reparent in/out".into());
    }
    let next = if dir < 0 {
        doc.scene
            .nodes
            .iter()
            .find(|n| Some(n.key) == parent)
            .and_then(|n| n.parent)
    } else {
        doc.scene
            .nodes
            .iter()
            .take_while(|n| n.key.as_u32() != *first)
            .filter(|n| n.parent == parent && !roots.contains(&n.key.as_u32()))
            .last()
            .map(|n| n.key)
    }
    .ok_or("no target parent")?;
    if roots
        .iter()
        .any(|k| collect_scene_subtree_keys(&doc, *k).contains(&next.as_u32()))
    {
        return Err("reparent cycle".into());
    }
    for node in doc.scene.nodes.to_mut() {
        if roots.contains(&node.key.as_u32()) {
            node.parent = Some(next);
        }
    }
    doc.normalize_links();
    Ok(commit(state, &doc, selected))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> EditorState {
        let doc = SceneDoc::parse(
            "$root = @r\n[r]\n[Node2D]\n[/Node2D]\n[/r]\n[a]\nparent = @r\n[Node2D]\n[/Node2D]\n[/a]\n[b]\nparent = @a\n[Node2D]\n[/Node2D]\n[/b]\n",
        );
        assert_eq!(doc.scene.nodes.len(), 3);
        EditorState {
            doc_text: doc.to_text(),
            selected_key: Some(2),
            selected_keys: vec![1, 2],
            ..Default::default()
        }
    }
    #[test]
    fn delete_subtree_once_and_undo_selection() {
        let mut s = state();
        assert!(delete(&mut s).unwrap());
        assert_eq!(cached_scene_doc(&s.doc_text).scene.nodes.len(), 1);
        assert_eq!(s.scene_undo_stack.len(), 1);
        assert!(crate::scripts::editor::main::undo_scene_doc(&mut s));
        assert_eq!(selection::keys(&s), [1, 2]);
    }
    #[test]
    fn clipboard_survives_source_delete() {
        let mut s = state();
        copy(&mut s).unwrap();
        delete(&mut s).unwrap();
        duplicate(&mut s, true).unwrap();
        assert_eq!(cached_scene_doc(&s.doc_text).scene.nodes.len(), 3);
    }
    #[test]
    fn root_delete_fails_without_edit() {
        let mut s = state();
        selection::replace(&mut s, vec![0]);
        let before = s.doc_text.clone();
        assert!(delete(&mut s).is_err());
        assert_eq!(s.doc_text, before);
    }
    #[test]
    fn clone_remaps_internal_refs_and_keeps_external_refs() {
        use perro_api::scene::{SceneFieldName, SceneValue, SceneValueKey};
        let s = state();
        let mut source = cached_scene_doc(&s.doc_text);
        source.scene.nodes.to_mut()[1].script_vars.to_mut().push((
            SceneFieldName::from_name("refs".to_string()),
            SceneValue::Array(Cow::Owned(vec![
                SceneValue::Key(SceneValueKey::from("b")),
                SceneValue::Key(SceneValueKey::from("r")),
            ])),
        ));
        let mut target = source.clone();
        let selected = clone_nodes(&mut target, &source, &[1], None, false);
        let node = target
            .scene
            .nodes
            .iter()
            .find(|n| n.key.as_u32() == selected[0])
            .unwrap();
        let SceneValue::Array(refs) = &node.script_vars[0].1 else {
            panic!("refs")
        };
        assert!(matches!(&refs[0],SceneValue::Key(k) if k.as_ref()=="b_copy_1"));
        assert!(matches!(&refs[1],SceneValue::Key(k) if k.as_ref()=="r"));
        assert_eq!(target.scene.nodes.len(), 5);
    }
}
