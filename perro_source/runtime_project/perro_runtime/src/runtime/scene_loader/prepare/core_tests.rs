#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::SignalID;
    use perro_nodes::SceneNodeData;
    use perro_scene::Parser;
    use perro_structs::{BitMask, Color, CustomPostParamValue, UVector2, Vector2, Vector3};

    #[test]
    fn runtime_scene_specs_cover_every_node_type() {
        for &node_type in NodeType::ALL {
            let source = SceneDefNodeData::new(node_type, Cow::Owned(Vec::new()), None);
            let data = scene_node_data_from(&source, None)
                .unwrap_or_else(|error| panic!("{node_type}: {error}"));
            assert_eq!(SceneNode::new(data).node_type(), node_type);
        }
    }





















































    include!("core_tests/water_lighting.rs");
    include!("core_tests/assets.rs");
    include!("core_tests/ui.rs");
    include!("core_tests/scene_data.rs");

}
