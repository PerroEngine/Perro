fn build_node_2d(data: &SceneDefNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data(&mut node, data);
    node
}

fn build_sprite_2d(data: &SceneDefNodeData) -> Sprite2D {
    let mut node = Sprite2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_sprite_2d_fields(&mut node, &data.fields);
    node
}

fn apply_node_2d_data(target: &mut Node2D, data: &SceneDefNodeData) {
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(target, base);
    }
    apply_node_2d_fields(target, &data.fields);
}

fn apply_node_2d_fields(node: &mut Node2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "position" => {
                if let Some((x, y)) = value.as_vec2() {
                    node.transform.position = Vector2 { x, y };
                }
            }
            "scale" => {
                if let Some((x, y)) = value.as_vec2() {
                    node.transform.scale = Vector2 { x, y };
                }
            }
            "rotation" => {
                if let Some(v) = value.as_f32() {
                    node.transform.rotation = v;
                }
            }
            "z_index" => {
                if let Some(v) = value.as_i32() {
                    node.z_index = v;
                }
            }
            "visible" => {
                if let Some(v) = value.as_bool() {
                    node.visible = v;
                }
            }
            _ => {}
        });
}

fn apply_sprite_2d_fields(_node: &mut Sprite2D, _fields: &[SceneObjectField]) {}
