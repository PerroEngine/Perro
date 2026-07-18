define_scene_node_builder! {
    fn build_camera_2d -> Camera2D = Camera2D::default();
    base node_2d;
    apply [apply_camera_2d_fields];
    custom |node, fields| { apply_audio_listener_options_data(&mut node.audio_options, fields); }
}

fn apply_camera_2d_fields(node: &mut Camera2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each_field(|field, value| {
        match field {
            SceneFieldName::Zoom => {
                if let Some(v) = value.as_f32() {
                    node.zoom = v;
                }
            }
            SceneFieldName::Active => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            _ => match resolve_scene_node_field("Camera2D", field) {
                Some(NodeField::Camera2D(Camera2DField::RenderMask)) => {
                    if let Some(v) = as_bitmask(value) {
                        node.render_mask = v;
                    }
                }
                Some(NodeField::Camera2D(Camera2DField::PostProcessing)) => {
                    if let Some(v) = as_post_processing(value) {
                        node.post_processing = v;
                    }
                }
                _ => {}
            },
        }
    });
}
