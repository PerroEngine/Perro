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
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("RayLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            Some(NodeField::RayLight3D(RayLight3DField::Visible)) => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_ambient_light_3d_fields(node: &mut AmbientLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("AmbientLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_point_light_3d_fields(node: &mut PointLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PointLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::PointLight3D(PointLight3DField::Range)) => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_spot_light_3d_fields(node: &mut SpotLight3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("SpotLight3D", name) {
            Some(NodeField::Light3D(Light3DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            Some(NodeField::Light3D(Light3DField::Intensity)) => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            Some(NodeField::SpotLight3D(SpotLight3DField::Range)) => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            Some(NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians)) => {
                if let Some(v) = as_f32(value) {
                    node.inner_angle_radians = v;
                }
            }
            Some(NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians)) => {
                if let Some(v) = as_f32(value) {
                    node.outer_angle_radians = v;
                }
            }
            Some(NodeField::Light3D(Light3DField::Active)) => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    });
}
