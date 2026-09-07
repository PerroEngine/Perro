use crate::scripts::editor::main::{EditorState, cached_scene_doc_shared};
use perro_api::scene::{SceneDoc, SceneKey};
use std::collections::HashSet;

pub fn keys(state: &EditorState) -> Vec<u32> {
    let Some(primary) = state.selected_key else {
        return Vec::new();
    };
    // Legacy single-node actions can still replace the primary directly.
    if !state.selected_keys.contains(&primary) {
        return vec![primary];
    }
    state.selected_keys.clone()
}

pub fn replace(state: &mut EditorState, keys: Vec<u32>) {
    state.selected_key = keys.last().copied();
    state.selected_keys = keys;
}

pub fn click(state: &mut EditorState, key: u32, ctrl: bool, shift: bool, visible: &[u32]) {
    let mut next = keys(state);
    if shift {
        if let Some((a, b)) = state
            .selection_anchor
            .and_then(|anchor| visible.iter().position(|k| *k == anchor))
            .zip(visible.iter().position(|k| *k == key))
        {
            next = visible[a.min(b)..=a.max(b)].to_vec();
            next.retain(|k| *k != key);
            next.push(key);
        } else {
            next = vec![key];
        }
    } else if ctrl {
        if next.contains(&key) {
            next.retain(|k| *k != key);
        } else {
            next.push(key);
        }
        state.selection_anchor = Some(key);
    } else {
        next = vec![key];
        state.selection_anchor = Some(key);
    }
    replace(state, next);
}

pub fn prune(state: &mut EditorState) {
    let doc = cached_scene_doc_shared(&state.doc_text);
    let valid: HashSet<_> = doc.scene.nodes.iter().map(|n| n.key.as_u32()).collect();
    let mut next = keys(state);
    next.retain(|k| valid.contains(k));
    replace(state, next);
}

/// Exclude selected descendants so a subtree is edited exactly once.
pub fn roots(doc: &SceneDoc, keys: &[u32]) -> Vec<u32> {
    let parents: std::collections::HashMap<_, _> = doc
        .scene
        .nodes
        .iter()
        .map(|n| (n.key.as_u32(), n.parent.map(SceneKey::as_u32)))
        .collect();
    let selected: HashSet<_> = keys.iter().copied().collect();
    let mut seen = HashSet::new();
    keys.iter()
        .copied()
        .filter(|key| {
            if !seen.insert(*key) || !parents.contains_key(key) {
                return false;
            }
            let mut parent = parents[key];
            let mut visited = HashSet::new();
            while let Some(p) = parent {
                if selected.contains(&p) || !visited.insert(p) {
                    return false;
                }
                parent = parents.get(&p).copied().flatten();
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggle_range_and_primary() {
        let mut state = EditorState::default();
        click(&mut state, 1, false, false, &[1, 2, 3]);
        click(&mut state, 3, false, true, &[1, 2, 3]);
        assert_eq!(keys(&state), [1, 2, 3]);
        click(&mut state, 3, true, false, &[1, 2, 3]);
        assert_eq!(keys(&state), [1, 2]);
        assert_eq!(state.selected_key, Some(2));
    }
    #[test]
    fn parent_selection_absorbs_child() {
        let mut doc = SceneDoc::parse(
            "$root = @r\n[r]\n[Node2D]\n[/Node2D]\n[/r]\n[child]\nparent = @r\n[Node2D]\n[/Node2D]\n[/child]\n",
        );
        assert_eq!(doc.scene.nodes.len(), 2);
        let a = doc.scene.nodes[0].key;
        let b = doc.scene.nodes[1].key;
        doc.scene.nodes.to_mut()[1].parent = Some(a);
        assert_eq!(roots(&doc, &[a.as_u32(), b.as_u32()]), [a.as_u32()]);
    }
}
