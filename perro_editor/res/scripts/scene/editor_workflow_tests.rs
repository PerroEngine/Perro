#[cfg(test)]
mod tests {
    use crate::scripts::editor::main::{EditorState, cached_scene_doc, undo_scene_doc};
    use crate::scripts::ui::editor_inspector_values::apply_shared_inspector_value;
    use crate::scripts::ui::editor_ui::inspector_visible_rows_for_node;
    use perro_api::scene::{SceneDoc, SceneFieldName, SceneValue};

    fn state() -> EditorState {
        let mut doc = SceneDoc::parse(
            "$root = @r\n[r]\n[Node2D]\n[/Node2D]\n[/r]\n[a]\nparent = @r\n[Node2D]\n[/Node2D]\n[/a]\n[b]\nparent = @r\n[Node2D]\n[/Node2D]\n[/b]\n",
        );
        for (i, node) in doc.scene.nodes.to_mut().iter_mut().enumerate().skip(1) {
            node.data.fields.to_mut().push((
                SceneFieldName::from_name("position".to_string()),
                SceneValue::Vec2 {
                    x: i as f32,
                    y: 0.0,
                },
            ));
        }
        EditorState {
            doc_text: doc.to_text(),
            selected_key: Some(2),
            selected_keys: vec![1, 2],
            ..Default::default()
        }
    }
    #[test]
    fn mixed_value_edit_commits_all_nodes_once_and_undo_restores() {
        let mut s = state();
        let before = s.doc_text.clone();
        let doc = cached_scene_doc(&s.doc_text);
        let rows = inspector_visible_rows_for_node(&s, &doc.scene.nodes[2]);
        let row = rows
            .iter()
            .find(|r| r.source == "scene" && r.name.trim() == "position")
            .expect("shared position");
        assert_eq!(row.value, "<mixed>");
        assert!(apply_shared_inspector_value(
            &mut s,
            row,
            &SceneValue::Vec2 { x: 9.0, y: 3.0 }
        ));
        assert_eq!(s.scene_undo_stack.len(), 1);
        let next = cached_scene_doc(&s.doc_text);
        for node in &next.scene.nodes[1..] {
            assert!(
                node.data
                    .fields
                    .iter()
                    .any(|(k, v)| k.as_ref() == "position"
                        && matches!(v,SceneValue::Vec2{x,y} if *x==9.0&&*y==3.0))
            );
        }
        assert!(!apply_shared_inspector_value(
            &mut s,
            row,
            &SceneValue::Vec2 { x: 9.0, y: 3.0 }
        ));
        assert_eq!(s.scene_undo_stack.len(), 1);
        assert!(undo_scene_doc(&mut s));
        assert_eq!(s.doc_text, before);
        assert_eq!(s.selected_keys, [1, 2]);
    }
}
