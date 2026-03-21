fn build_camera_2d(data: &SceneDefNodeData) -> Camera2D {
    let mut node = Camera2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_camera_2d_fields(&mut node, &data.fields);
    node
}

fn apply_camera_2d_fields(node: &mut Camera2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "zoom" => {
                if let Some(v) = value.as_f32() {
                    node.zoom = v;
                }
            }
            "post_processing" => {
                if let Some(v) = as_post_processing(value) {
                    node.post_processing = v;
                }
            }
            "active" => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            _ => {}
        });
}
