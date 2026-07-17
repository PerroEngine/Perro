mod nodes {
    use super::*;

    #[test]
    fn node_collection_typed_nodes_fill_defaults_and_keep_meta() {
        let collection = node_collection![{
            name = "title",
            tags = tags!["ui"],
            node = UiLabel {
                text: {"Paused".into()},
                font_size: 32.0,
            },
            script = res_path!("res://scripts/title.rs"),
        }];

        assert_eq!(collection.specs.len(), 1);
        let spec = &collection.specs[0];
        assert_eq!(spec.name.as_deref(), Some("title"));
        assert_eq!(spec.tags.len(), 1);
        assert_eq!(
            spec.script.as_ref().map(|script| script.path.as_ref()),
            Some("res://scripts/title.rs")
        );
        match &spec.data {
            SceneNodeData::UiLabel(label) => {
                assert_eq!(label.text.as_ref(), "Paused");
                assert_eq!(label.font_size, 32.0);
            }
            other => panic!("expected UiLabel, got {other:?}"),
        }
    }

}
