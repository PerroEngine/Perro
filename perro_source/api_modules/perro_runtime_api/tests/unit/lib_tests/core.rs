mod core {
    use super::*;

    #[test]
    fn runtime_prelude_exports_world_label_and_sprite_nodes() {
        fn _uses_node_types(
            _label_2d: Option<Label2D>,
            _label_3d: Option<Label3D>,
            _sprite_3d: Option<Sprite3D>,
        ) {
        }

        let _node_type = NodeType::Label3D;
        let _data = SceneNodeData::Label3D(Label3D::new());
    }

}
