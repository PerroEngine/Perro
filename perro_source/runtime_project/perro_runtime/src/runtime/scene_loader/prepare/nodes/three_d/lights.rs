fn build_ray_light_3d(data: &SceneDefNodeData) -> RayLight3D {
    let mut node = RayLight3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_ray_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_ambient_light_3d(data: &SceneDefNodeData) -> AmbientLight3D {
    let mut node = AmbientLight3D::new();
    apply_ambient_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_point_light_3d(data: &SceneDefNodeData) -> PointLight3D {
    let mut node = PointLight3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_point_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_spot_light_3d(data: &SceneDefNodeData) -> SpotLight3D {
    let mut node = SpotLight3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_spot_light_3d_fields(&mut node, &data.fields);
    node
}

fn apply_ray_light_3d_fields(node: &mut RayLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        });
}

fn apply_ambient_light_3d_fields(node: &mut AmbientLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        });
}

fn apply_point_light_3d_fields(node: &mut PointLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        });
}

fn apply_spot_light_3d_fields(node: &mut SpotLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            "inner_angle_radians" => {
                if let Some(v) = as_f32(value) {
                    node.inner_angle_radians = v;
                }
            }
            "outer_angle_radians" => {
                if let Some(v) = as_f32(value) {
                    node.outer_angle_radians = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        });
}
